//! Daemonize the current process, then bring up the tokio runtime and
//! hand control to the server loop.
//!
//! Setting `PSWARM_DAEMON_FOREGROUND=1` skips the daemonize dance: the
//! process keeps its stdout/stderr and stays attached to its parent. This
//! is the mode integration tests use so they can spawn a daemon, hold its
//! `Child` handle, and kill it cleanly on teardown.

use anyhow::{Context, Result};
use daemonize::Daemonize;
use std::fs::{self, OpenOptions};
use tracing::info;

use crate::paths;

pub fn start() -> Result<()> {
    let socket_path = paths::socket_path()?;
    let foreground = std::env::var_os("PSWARM_DAEMON_FOREGROUND").is_some();

    // The socket directory is needed in either mode.
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if !foreground {
        let log_path = paths::log_path()?;
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

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
    }

    // Tracing init runs in either mode. After daemonize, stderr points at
    // the log file; in foreground mode it is the original terminal/pipe.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!(foreground, "picoswarm daemon starting");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;

    runtime.block_on(super::server::run(socket_path))?;

    info!("picoswarm daemon stopping");
    Ok(())
}
