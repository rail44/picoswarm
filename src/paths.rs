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
        .map_or_else(|| dirs.home_dir().join(".local").join("run"), PathBuf::from);
    Ok(base.join("picoswarm").join("sock"))
}

/// Directory holding the daemon's tracing logs (rotated daily) plus
/// the small append-only crash log that captures stdout/stderr from
/// the daemonized process.
pub fn log_dir() -> Result<PathBuf> {
    let dirs = base_dirs()?;
    let base = std::env::var_os("XDG_STATE_HOME").map_or_else(
        || dirs.home_dir().join(".local").join("state"),
        PathBuf::from,
    );
    Ok(base.join("picoswarm"))
}

/// Append-only file the daemon's stdout/stderr is redirected to via
/// `daemonize`. Captures panics and any direct stderr writes from
/// libraries that bypass tracing. Tracing's own output goes to the
/// rolling files in [`log_dir`], not here.
pub fn crash_log_path() -> Result<PathBuf> {
    Ok(log_dir()?.join("daemon.crash.log"))
}

/// Directory holding per-agent inbox files (one append-only `.jsonl`
/// per agent, plus a `.cursor` file with the reader's last-read line
/// number). File-direct: senders and readers touch these files
/// without going through the daemon's protocol.
pub fn inbox_dir() -> Result<PathBuf> {
    Ok(log_dir()?.join("inbox"))
}

/// Append-only JSON Lines file holding messages addressed to `name`.
/// Senders open with `O_APPEND | O_CREAT`; readers stream from the
/// cursor onward.
pub fn inbox_message_path(name: &str) -> Result<PathBuf> {
    Ok(inbox_dir()?.join(format!("{name}.jsonl")))
}

/// Single-line file holding the reader's progress (line number, 1-based)
/// in the corresponding `<name>.jsonl`. Lazy-created by the reader on
/// first emit.
pub fn inbox_cursor_path(name: &str) -> Result<PathBuf> {
    Ok(inbox_dir()?.join(format!("{name}.cursor")))
}
