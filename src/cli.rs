use std::fs::{self, OpenOptions};
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command as ProcessCommand, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde_json::json;

use crate::app;
use crate::config::AppConfig;
use crate::paths::AppPaths;
use crate::platform::probe_wayland;

#[derive(Debug, Parser)]
#[command(
    name = "moonlight-clock",
    version,
    about = "Moonlight Clock for COSMIC"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Clone, Debug, Subcommand)]
enum Command {
    /// Run the widget in the foreground.
    Run,
    /// Start the widget in the background.
    Start,
    /// Stop the running widget.
    Stop,
    /// Restart the widget.
    Restart,
    /// Show process state.
    Status,
    /// Refresh weather data.
    Refresh,
    /// Check runtime requirements.
    Doctor,
    /// Create the default configuration.
    InitConfig,
    /// Open config.toml with xdg-open.
    OpenConfig,
}

pub fn main() -> ExitCode {
    match execute(Cli::parse().command.unwrap_or(Command::Run)) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command) -> Result<u8> {
    let paths = AppPaths::from_environment()?;
    match command {
        Command::Run => run(paths),
        Command::Start => start(&paths),
        Command::Stop => stop(&paths, false),
        Command::Restart => {
            stop(&paths, true)?;
            start(&paths)
        }
        Command::Status => status(&paths),
        Command::Refresh => refresh(&paths),
        Command::Doctor => doctor(&paths),
        Command::InitConfig => {
            let created = paths.ensure_config()?;
            println!(
                "{}{}",
                if created { "Created " } else { "Exists  " },
                paths.config_file.display()
            );
            Ok(0)
        }
        Command::OpenConfig => open_config(&paths),
    }
}

fn run(paths: AppPaths) -> Result<u8> {
    paths.ensure_config()?;
    if let Some(pid) = running_pid(&paths.pid_file)
        && pid != std::process::id()
    {
        bail!("Moonlight Clock is already running (PID {pid})");
    }
    fs::write(&paths.pid_file, std::process::id().to_string())
        .with_context(|| format!("cannot write {}", paths.pid_file.display()))?;
    let _guard = PidGuard(paths.pid_file.clone());
    app::run(paths)?;
    Ok(0)
}

fn start(paths: &AppPaths) -> Result<u8> {
    paths.ensure_config()?;
    if let Some(pid) = running_pid(&paths.pid_file) {
        println!("Moonlight Clock is already running (PID {pid})");
        return Ok(0);
    }
    let executable = std::env::current_exe().context("cannot find the Moonlight Clock binary")?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&paths.log_file)
        .with_context(|| format!("cannot open {}", paths.log_file.display()))?;
    let error_log = log.try_clone()?;
    let mut command = ProcessCommand::new(executable);
    command
        .arg("run")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(error_log));
    // A detached session keeps the widget alive after the invoking terminal closes.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().context("cannot start Moonlight Clock")?;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if let Some(status) = child.try_wait()? {
            bail!(
                "Moonlight Clock exited with {status}; see {}",
                paths.log_file.display()
            );
        }
        if running_pid(&paths.pid_file) == Some(child.id()) {
            println!("Moonlight Clock started (PID {})", child.id());
            return Ok(0);
        }
    }
    bail!(
        "Moonlight Clock did not start; see {}",
        paths.log_file.display()
    )
}

fn stop(paths: &AppPaths, quiet: bool) -> Result<u8> {
    let Some(pid) = running_pid(&paths.pid_file) else {
        let _ = fs::remove_file(&paths.pid_file);
        if !quiet {
            println!("Moonlight Clock is not running");
        }
        return Ok(0);
    };
    send_signal(pid, libc::SIGTERM)?;
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(50));
        if !pid_exists(pid) {
            let _ = fs::remove_file(&paths.pid_file);
            if !quiet {
                println!("Moonlight Clock stopped");
            }
            return Ok(0);
        }
    }
    bail!("Moonlight Clock did not stop cleanly (PID {pid})")
}

fn status(paths: &AppPaths) -> Result<u8> {
    let pid = running_pid(&paths.pid_file);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "running": pid.is_some(),
            "pid": pid,
            "config": paths.config_file,
            "backend": "libcosmic-wayland"
        }))?
    );
    Ok(if pid.is_some() { 0 } else { 1 })
}

fn refresh(paths: &AppPaths) -> Result<u8> {
    if running_pid(&paths.pid_file).is_none() {
        bail!("Moonlight Clock is not running");
    }
    let marker = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();
    fs::write(&paths.refresh_file, marker)
        .with_context(|| format!("cannot write {}", paths.refresh_file.display()))?;
    Ok(0)
}

fn doctor(paths: &AppPaths) -> Result<u8> {
    paths.ensure_config()?;
    let config_valid = AppConfig::load(&paths.config_file).is_ok();
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").is_some();
    let capabilities = probe_wayland();
    let wayland = capabilities.is_ok();
    let layer_shell = capabilities
        .as_ref()
        .is_ok_and(|capabilities| capabilities.layer_shell);
    let checks = [
        ("configuration", config_valid),
        ("XDG runtime directory", runtime),
        ("Wayland compositor", wayland),
        ("wlr layer shell", layer_shell),
    ];
    for (label, passed) in checks {
        println!("{}  {label}", if passed { "OK" } else { "MISSING" });
    }
    if let Err(error) = capabilities {
        eprintln!("Wayland probe: {error:#}");
    }
    Ok(if checks.iter().all(|(_, passed)| *passed) {
        0
    } else {
        1
    })
}

fn open_config(paths: &AppPaths) -> Result<u8> {
    paths.ensure_config()?;
    let status = ProcessCommand::new("xdg-open")
        .arg(&paths.config_file)
        .spawn()
        .context("xdg-open is not available")?;
    drop(status);
    Ok(0)
}

fn read_pid(path: &Path) -> Option<u32> {
    fs::read_to_string(path)
        .ok()?
        .trim()
        .parse()
        .ok()
        .filter(|pid| *pid > 0)
}

fn running_pid(path: &Path) -> Option<u32> {
    read_pid(path).filter(|pid| pid_exists(*pid))
}

fn pid_exists(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || io::Error::last_os_error().kind() == io::ErrorKind::PermissionDenied
}

fn send_signal(pid: u32, signal: i32) -> Result<()> {
    if unsafe { libc::kill(pid as libc::pid_t, signal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error()).context("cannot signal Moonlight Clock")
    }
}

struct PidGuard(std::path::PathBuf);

impl Drop for PidGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_valid_positive_pids() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pid");
        fs::write(&path, std::process::id().to_string()).unwrap();
        assert_eq!(read_pid(&path), Some(std::process::id()));
        fs::write(&path, "not-a-pid").unwrap();
        assert_eq!(read_pid(&path), None);
    }
}
