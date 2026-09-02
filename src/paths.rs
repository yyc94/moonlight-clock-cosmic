use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};

pub const DEFAULT_CONFIG: &str = include_str!("../config.example.toml");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub state_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub config_file: PathBuf,
    pub cache_file: PathBuf,
    pub pid_file: PathBuf,
    pub refresh_file: PathBuf,
    pub log_file: PathBuf,
}

impl AppPaths {
    pub fn from_environment() -> Result<Self> {
        let home = dirs::home_dir().context("HOME is not available")?;
        let config_root = env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let state_root = env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/state"));
        let config_dir = config_root.join("moonlight-clock");
        let state_dir = state_root.join("moonlight-clock");
        let runtime_dir = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| state_dir.join("runtime"))
            .join("moonlight-clock");

        Ok(Self {
            config_file: config_dir.join("config.toml"),
            cache_file: state_dir.join("weather.json"),
            pid_file: runtime_dir.join("moonlight-clock.pid"),
            refresh_file: runtime_dir.join("refresh"),
            log_file: state_dir.join("moonlight-clock.log"),
            config_dir,
            state_dir,
            runtime_dir,
        })
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for path in [&self.config_dir, &self.state_dir, &self.runtime_dir] {
            fs::create_dir_all(path)
                .with_context(|| format!("cannot create {}", path.display()))?;
        }
        Ok(())
    }

    pub fn ensure_config(&self) -> Result<bool> {
        self.ensure_dirs()?;
        if self.config_file.exists() {
            return Ok(false);
        }
        fs::write(&self.config_file, DEFAULT_CONFIG)
            .with_context(|| format!("cannot write {}", self.config_file.display()))?;
        set_private_permissions(&self.config_file)?;
        Ok(true)
    }
}

#[cfg(unix)]
fn set_private_permissions(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("cannot set permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_private_permissions(_path: &std::path::Path) -> Result<()> {
    Ok(())
}
