//! Session spawning: open a PTY, run the requested command, hand the
//! output stream over to a per-agent session task, and assemble an
//! `AgentEntry` that the registry can store.

use anyhow::{anyhow, Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::daemon::output_session::{self, ChunkSender};
use crate::daemon::registry::AgentEntry;
use crate::protocol::RunRequest;

pub fn spawn_session(req: RunRequest) -> Result<AgentEntry> {
    validate_name(&req.name)?;

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: req.initial_size.rows,
            cols: req.initial_size.cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .context("openpty failed")?;

    // Default to `claude` when the client sends an empty argv.
    let argv: Vec<String> = if req.cmd.is_empty() {
        vec!["claude".to_string()]
    } else {
        req.cmd.clone()
    };
    let (program, rest) = argv
        .split_first()
        .ok_or_else(|| anyhow!("empty cmd after defaulting"))?;

    let mut cmd = CommandBuilder::new(program);
    for arg in rest {
        cmd.arg(arg);
    }
    if let Some(cwd) = &req.cwd {
        cmd.cwd(cwd);
    }

    // Inherit the daemon's env, then layer on PSWARM_DAEMON=1, then any
    // explicit overrides from the client.
    for (k, v) in std::env::vars() {
        cmd.env(k, v);
    }
    cmd.env("PSWARM_DAEMON", "1");
    for (k, v) in &req.env {
        cmd.env(k, v);
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .context("failed to spawn the agent process")?;
    debug!("spawned {} (pid {:?})", req.name, child.process_id());
    drop(pair.slave);

    let reader = pair
        .master
        .try_clone_reader()
        .context("try_clone_reader failed")?;
    let writer = pair
        .master
        .take_writer()
        .context("take_writer failed")?;

    let master = Arc::new(Mutex::new(pair.master));
    let child = Arc::new(Mutex::new(child));
    let writer = Arc::new(Mutex::new(writer));
    let attached = Arc::new(AtomicBool::new(false));
    let dead = Arc::new(AtomicBool::new(false));

    // The session task gets an `on_eof` closure that captures the child
    // handle so it can report the exit code and flip the `dead` flag.
    let child_for_eof = Arc::clone(&child);
    let dead_for_eof = Arc::clone(&dead);
    let on_eof = move || {
        let exit = match child_for_eof.lock() {
            Ok(mut c) => c.try_wait().ok().flatten().map(|s| s.exit_code() as i32),
            Err(_) => None,
        };
        dead_for_eof.store(true, Ordering::Relaxed);
        exit
    };

    let (chunk_tx, inbox) = output_session::spawn(on_eof);

    // Drain the master into the session task on a dedicated OS thread.
    let agent_name = req.name.clone();
    thread::Builder::new()
        .name(format!("pty-reader/{}", agent_name))
        .spawn(move || drain_into_session(&agent_name, reader, chunk_tx))
        .context("failed to start the PTY reader thread")?;

    Ok(AgentEntry {
        id: Uuid::new_v4(),
        name: req.name,
        cwd: req.cwd,
        created_at: now_unix(),
        master,
        child,
        writer,
        inbox,
        attached,
        dead,
    })
}

fn drain_into_session(agent_name: &str, mut reader: Box<dyn Read + Send>, sink: ChunkSender) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                debug!("PTY reader for {} reached EOF", agent_name);
                return;
            }
            Ok(n) => {
                if sink.send(buf[..n].to_vec()).is_err() {
                    debug!("session task closed; PTY reader for {} exiting", agent_name);
                    return;
                }
            }
            Err(e) => {
                warn!("PTY reader for {} exiting on error: {}", agent_name, e);
                return;
            }
        }
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn validate_name(name: &str) -> Result<()> {
    let len = name.chars().count();
    if !(1..=64).contains(&len) {
        return Err(anyhow!("agent name must be 1-64 characters: {}", name));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(anyhow!(
            "agent name may only contain a-z, A-Z, 0-9, '.', '_', '-': {}",
            name
        ));
    }
    Ok(())
}
