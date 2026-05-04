//! Session spawning: open a PTY, run the requested command, and start
//! draining its output into the per-session ring buffer.

use anyhow::{anyhow, Context, Result};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::daemon::registry::{AgentEntry, RING_BUFFER_BYTES};
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

    // The slave handle is owned by the child once spawned.
    drop(pair.slave);

    let ring_buffer: Arc<Mutex<VecDeque<u8>>> =
        Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_BYTES)));

    // Drain the master into the ring buffer on a dedicated OS thread so
    // the kernel PTY buffer never fills up even when no client is attached.
    let reader = pair
        .master
        .try_clone_reader()
        .context("try_clone_reader failed")?;
    let buffer_clone = Arc::clone(&ring_buffer);
    let agent_name = req.name.clone();
    thread::Builder::new()
        .name(format!("pty-reader/{}", agent_name))
        .spawn(move || drain_into_ring(&agent_name, reader, buffer_clone))
        .context("failed to start the PTY reader thread")?;

    Ok(AgentEntry {
        id: Uuid::new_v4(),
        name: req.name,
        cwd: req.cwd,
        created_at: now_unix(),
        master: pair.master,
        child,
        ring_buffer,
        dead: false,
    })
}

fn drain_into_ring(
    agent_name: &str,
    mut reader: Box<dyn Read + Send>,
    buffer: Arc<Mutex<VecDeque<u8>>>,
) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => {
                debug!("PTY reader for {} reached EOF", agent_name);
                return;
            }
            Ok(n) => {
                let Ok(mut ring) = buffer.lock() else { return };
                for &byte in &buf[..n] {
                    if ring.len() == RING_BUFFER_BYTES {
                        ring.pop_front();
                    }
                    ring.push_back(byte);
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
