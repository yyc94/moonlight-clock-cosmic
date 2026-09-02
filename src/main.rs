mod app;
mod astronomy;
mod cli;
mod config;
mod content;
mod layer;
mod paths;
mod platform;
mod render;
mod weather;

fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "moonlight_clock=info".into()),
        )
        .with_target(false)
        .init();
    cli::main()
}
