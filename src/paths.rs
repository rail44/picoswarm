//! XDG-style path resolution for picoswarm's runtime files.

use anyhow::{Context, Result};
use directories::BaseDirs;
use std::path::PathBuf;

fn base_dirs() -> Result<BaseDirs> {
    BaseDirs::new().context("could not determine the user's home directory")
}

/// The Unix socket the daemon binds to and clients connect to.
pub fn socket_path() -> Result<PathBuf> {
    let dirs = base_dirs()?;
    let base = dirs
        .runtime_dir()
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs.home_dir().join(".local").join("run"));
    Ok(base.join("picoswarm").join("sock"))
}

/// Directory holding the daemon's tracing logs (rotated daily) plus
/// the small append-only crash log that captures stdout/stderr from
/// the daemonized process.
pub fn log_dir() -> Result<PathBuf> {
    let dirs = base_dirs()?;
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs.home_dir().join(".local").join("state"));
    Ok(base.join("picoswarm"))
}

/// Append-only file the daemon's stdout/stderr is redirected to via
/// `daemonize`. Captures panics and any direct stderr writes from
/// libraries that bypass tracing. Tracing's own output goes to the
/// rolling files in [`log_dir`], not here.
pub fn crash_log_path() -> Result<PathBuf> {
    Ok(log_dir()?.join("daemon.crash.log"))
}
