//! Daemonize the current process, then bring up the tokio runtime and
//! hand control to the server loop.

use anyhow::{Context, Result};
use daemonize::Daemonize;
use std::fs::{self, OpenOptions};
use tracing::info;

use crate::paths;

pub fn start() -> Result<()> {
    let socket_path = paths::socket_path()?;
    let log_path = paths::log_path()?;

    // Ensure parent directories exist while we still have a real stderr to
    // surface failures on.
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    // Open the log file. After daemonize redirects, tracing output (which
    // defaults to stderr) lands here.
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .with_context(|| format!("failed to open {}", log_path.display()))?;
    let stderr = stdout.try_clone()?;

    Daemonize::new()
        .working_directory("/")
        .stdout(stdout)
        .stderr(stderr)
        .start()
        .context("failed to daemonize")?;

    // From this point on we are the daemon process.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("picoswarm daemon starting");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;

    runtime.block_on(super::server::run(socket_path))?;

    info!("picoswarm daemon stopping");
    Ok(())
}
