//! Session spawning: open a PTY, run the requested command, hand the
//! output stream over to a per-agent session task, and assemble an
//! `AgentEntry` that the registry can store.

use anyhow::{Context, Result, anyhow};
use nix::sys::prctl;
use nix::sys::signal::Signal;
use pty_process::Size;
use pty_process::blocking::{Command, Pty, open};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::SystemTime;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::daemon::output_session::{self, ChunkSender};
use crate::daemon::registry::{AgentEntry, AgentKind};
use crate::protocol::RunRequest;

pub fn spawn_session(req: RunRequest) -> Result<AgentEntry> {
    validate_agent_name(&req.name)?;

    let (pty, pts) = open().context("openpty failed")?;
    pty.resize(Size::new(req.initial_size.rows, req.initial_size.cols))
        .context("initial resize failed")?;

    let argv: Vec<String> = if req.cmd.is_empty() {
        vec!["claude".to_string()]
    } else {
        req.cmd.clone()
    };
    let (program, rest) = argv
        .split_first()
        .ok_or_else(|| anyhow!("empty cmd after defaulting"))?;

    let mut cmd = Command::new(program);
    for arg in rest {
        cmd = cmd.arg(arg);
    }
    if let Some(cwd) = &req.cwd {
        cmd = cmd.current_dir(cwd);
    }

    // The agent's id needs to land in the spawned env, so generate it
    // here and reuse the same value for the AgentEntry below.
    let agent_id = Uuid::new_v4();

    // Env in three layers, last write wins:
    //   daemon env -> request env -> daemon-controlled identity
    for (k, v) in std::env::vars() {
        cmd = cmd.env(k, v);
    }
    for (k, v) in &req.env {
        cmd = cmd.env(k, v);
    }
    cmd = cmd
        .env("PSWARM_DAEMON", "1")
        .env("PSWARM_AGENT_NAME", &req.name)
        .env("PSWARM_AGENT_ID", agent_id.to_string());

    // PR_SET_PDEATHSIG: kernel SIGTERMs the child if the daemon dies
    // hard, preventing orphan agents reparented to init (issue #21).
    unsafe {
        cmd = cmd.pre_exec(|| {
            prctl::set_pdeathsig(Signal::SIGTERM)
                .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
        });
    }

    let child = cmd
        .spawn(pts)
        .context("failed to spawn the agent process")?;
    debug!("spawned {} (pid {})", req.name, child.id());

    let pty = Arc::new(pty);
    let child = Arc::new(Mutex::new(child));
    let write_lock = Arc::new(Mutex::new(()));
    let attached = Arc::new(AtomicBool::new(false));
    let dead = Arc::new(AtomicBool::new(false));

    let child_for_eof = Arc::clone(&child);
    let dead_for_eof = Arc::clone(&dead);
    let on_eof = move || {
        let exit = child_for_eof.lock().ok().and_then(|mut c| {
            c.try_wait()
                .ok()
                .flatten()
                .and_then(|s| s.code())
                .or(Some(0))
        });
        dead_for_eof.store(true, Ordering::Relaxed);
        exit
    };

    let (chunk_tx, inbox) = output_session::spawn(on_eof);

    let pty_for_read = Arc::clone(&pty);
    let agent_name = req.name.clone();
    thread::Builder::new()
        .name(format!("pty-reader/{agent_name}"))
        .spawn(move || drain_into_session(&agent_name, &pty_for_read, &chunk_tx))
        .context("failed to start the PTY reader thread")?;

    Ok(AgentEntry {
        id: agent_id,
        name: req.name,
        created_at: now_unix(),
        kind: AgentKind::Spawned {
            pty,
            child,
            write_lock,
            inbox,
            cwd: req.cwd,
        },
        last_event: Arc::new(Mutex::new(None)),
        attached,
        dead,
    })
}

fn drain_into_session(agent_name: &str, pty: &Arc<Pty>, sink: &ChunkSender) {
    let mut buf = [0u8; 8192];
    loop {
        // `&Pty` impls Read, so the reader thread can read from the
        // shared Arc<Pty> without taking the write lock.
        match (&**pty).read(&mut buf) {
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
        .map_or(0, |d| d.as_secs().cast_signed())
}

/// Shared agent-name validation. Used both by `spawn_session` (PTY-backed
/// `pswarm run`) and by the daemon server's `Register` handler. Names
/// must be 1–64 chars, ASCII alphanumeric plus `.`, `_`, `-`, and not
/// the reserved literal `self`.
pub fn validate_agent_name(name: &str) -> Result<()> {
    let len = name.chars().count();
    if !(1..=64).contains(&len) {
        return Err(anyhow!("agent name must be 1-64 characters: {name}"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(anyhow!(
            "agent name may only contain a-z, A-Z, 0-9, '.', '_', '-': {name}"
        ));
    }
    if name == "self" {
        return Err(anyhow!(
            "the name `self` is reserved for `pswarm <verb> self` env-resolution"
        ));
    }
    Ok(())
}
