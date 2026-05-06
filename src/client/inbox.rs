//! `pswarm inbox`: a file-backed message channel between agents.
//!
//! The inbox is a per-agent append-only JSON Lines file at
//! `$XDG_STATE_HOME/picoswarm/inbox/<name>.jsonl`. Senders post a
//! single line per message; the recipient agent (or any other reader)
//! tails the file and emits unread lines, advancing a sidecar
//! `<name>.cursor` file (1-based line number).
//!
//! No daemon mediation: senders and readers touch the files directly.
//! This keeps the daemon out of message-passing state, makes debug
//! trivial (`cat $XDG_STATE_HOME/picoswarm/inbox/X.jsonl`), and
//! survives daemon restarts. See `docs/decision-log.md` item 19.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::paths;

const ENV_AGENT_NAME: &str = "PSWARM_AGENT_NAME";

/// Hard cap on a single encoded message line (including the trailing
/// newline). Below `PIPE_BUF` (4096 on Linux) so a single
/// `O_APPEND` write is atomic and concurrent posters can't
/// interleave.
const MAX_LINE_BYTES: usize = 4000;

/// Polling interval for `--follow` (file `tail -F`-style). Local-disk
/// poll, so 100 ms is comfortably below human noticeability and
/// avoids hammering the kernel.
const FOLLOW_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Serialize, Deserialize)]
struct Message<'a> {
    ts: i64,
    from: &'a str,
    body: &'a str,
}

pub fn post(to: &str, body: Option<String>, from: Option<String>) -> Result<()> {
    let body = match body {
        Some(b) => b,
        None => read_stdin_or_bail()?,
    };
    let from = match from.or_else(|| std::env::var(ENV_AGENT_NAME).ok()) {
        Some(f) if !f.is_empty() => f,
        _ => bail!(
            "missing sender identity: pass `--from <label>` or run from inside a pswarm-spawned agent (where `{ENV_AGENT_NAME}` is set automatically)"
        ),
    };

    let msg = Message {
        ts: now_unix(),
        from: &from,
        body: &body,
    };
    let mut line = serde_json::to_string(&msg).context("serialise inbox message")?;
    line.push('\n');

    if line.len() >= MAX_LINE_BYTES {
        bail!(
            "encoded message is {} bytes; the per-line limit is {} (POSIX `O_APPEND` atomicity boundary). Split the message or reference a file path in the body.",
            line.len(),
            MAX_LINE_BYTES
        );
    }

    let path = paths::inbox_message_path(to)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create inbox dir at {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .with_context(|| format!("open inbox at {}", path.display()))?;
    file.write_all(line.as_bytes())
        .with_context(|| format!("append to inbox at {}", path.display()))?;
    Ok(())
}

pub fn read(follow: bool) -> Result<()> {
    let Ok(name) = std::env::var(ENV_AGENT_NAME) else {
        // Outside a pswarm-spawned context: silent no-op so wrappers
        // (Claude Code's Monitor on `pswarm inbox read --follow`) do
        // not error when the user runs them outside an agent.
        return Ok(());
    };
    if name.is_empty() {
        return Ok(());
    }

    let inbox_path = paths::inbox_message_path(&name)?;
    let cursor_path = paths::inbox_cursor_path(&name)?;
    let mut cursor = read_cursor(&cursor_path);

    drain(&inbox_path, &cursor_path, &mut cursor)?;

    if follow {
        loop {
            std::thread::sleep(FOLLOW_POLL_INTERVAL);
            drain(&inbox_path, &cursor_path, &mut cursor)?;
        }
    }
    Ok(())
}

fn drain(inbox_path: &Path, cursor_path: &Path, cursor: &mut usize) -> Result<()> {
    let lines: Vec<String> = match File::open(inbox_path) {
        Ok(f) => BufReader::new(f)
            .lines()
            .collect::<std::io::Result<Vec<_>>>()
            .context("read inbox file")?,
        Err(_) => return Ok(()), // Inbox not created yet — nothing to drain.
    };

    if lines.len() < *cursor {
        // File shrank below the cursor. The agent likely respawned
        // with a clean inbox; reset and re-emit.
        *cursor = 0;
    }

    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        if line_no <= *cursor {
            continue;
        }
        if serde_json::from_str::<serde_json::Value>(line).is_err() {
            bail!(
                "corrupted inbox line at {line_no} of {}: not valid JSON. The atomic-append invariant has been violated; refusing to advance the cursor.",
                inbox_path.display()
            );
        }
        println!("{line}");
        *cursor = line_no;
        write_cursor(cursor_path, *cursor)?;
    }
    Ok(())
}

fn read_cursor(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

fn write_cursor(path: &Path, value: usize) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create inbox dir at {}", parent.display()))?;
    }
    std::fs::write(path, value.to_string())
        .with_context(|| format!("write cursor at {}", path.display()))?;
    Ok(())
}

fn read_stdin_or_bail() -> Result<String> {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        bail!("no body given and stdin is a terminal; pass body as an argument or pipe it in");
    }
    let mut buf = String::new();
    stdin.lock().read_to_string(&mut buf)?;
    Ok(buf)
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}
