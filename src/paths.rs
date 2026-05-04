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

/// The append-only log file the daemon writes its tracing output to.
pub fn log_path() -> Result<PathBuf> {
    let dirs = base_dirs()?;
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs.home_dir().join(".local").join("state"));
    Ok(base.join("picoswarm").join("daemon.log"))
}
