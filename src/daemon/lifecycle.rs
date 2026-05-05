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
use tracing_appender::rolling::{Builder as RollingBuilder, Rotation};

use crate::paths;

/// How many days of rotated tracing logs to keep before pruning.
const LOG_RETENTION_DAYS: usize = 7;

pub fn start() -> Result<()> {
    let socket_path = paths::socket_path()?;
    let foreground = std::env::var_os("PSWARM_DAEMON_FOREGROUND").is_some();

    // The socket directory is needed in either mode.
    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    if !foreground {
        // Stdout/stderr from the daemonized process get redirected to a
        // dedicated crash log. Tracing's own output goes through the
        // rolling appender below; this file only catches things that
        // bypass tracing (panics, direct stderr writes from libraries).
        let crash_path = paths::crash_log_path()?;
        if let Some(parent) = crash_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let crash = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
            .with_context(|| format!("failed to open {}", crash_path.display()))?;
        let crash_dup = crash.try_clone()?;

        Daemonize::new()
            .working_directory("/")
            .stdout(crash)
            .stderr(crash_dup)
            .start()
            .context("failed to daemonize")?;
    }

    init_tracing(foreground)?;

    info!(foreground, "picoswarm daemon starting");

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;

    // Spawner must be created post-daemonize (so the OS thread lives in
    // the surviving child process) and outside the tokio runtime (so
    // its lifetime is bound to this function, not to a worker thread
    // tokio might reap). The Spawner's thread becomes the parent of
    // every agent fork, which is what makes `PR_SET_PDEATHSIG` actually
    // fire on daemon shutdown rather than on a random worker timeout.
    // The runtime handle is passed so the spawner thread can `enter`
    // the runtime before each fork (some spawn-time bookkeeping calls
    // `tokio::spawn`).
    let spawner = super::spawner::Spawner::new(runtime.handle().clone())?;

    runtime.block_on(super::server::run(socket_path, spawner))?;

    info!("picoswarm daemon stopping");
    Ok(())
}

fn init_tracing(foreground: bool) -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if foreground {
        // Tests / interactive use: write to the inherited stderr so the
        // operator (or test harness) sees logs in real time.
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_writer(std::io::stderr)
            .init();
        return Ok(());
    }

    // Background daemon: rotate daily, keep the last LOG_RETENTION_DAYS
    // files. Filenames look like `daemon.YYYY-MM-DD.log` in `log_dir`.
    let dir = paths::log_dir()?;
    fs::create_dir_all(&dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let appender = RollingBuilder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("daemon")
        .filename_suffix("log")
        .max_log_files(LOG_RETENTION_DAYS)
        .build(&dir)
        .with_context(|| format!("failed to build rolling log appender in {}", dir.display()))?;
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_writer(appender)
        .with_ansi(false)
        .init();
    Ok(())
}
