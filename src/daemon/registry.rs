//! In-memory agent registry held inside the daemon process.
//!
//! Per-agent state is shared via `Arc<Mutex<>>` (or atomic flags) so the
//! attach handler, the session task, and the registry's own bookkeeping
//! can each operate without holding the global registry lock for long.
//! Nothing is persisted to disk: when the daemon dies its child processes
//! die with it, so reviving registry rows would describe nothing real.

use anyhow::{Result, anyhow};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use pty_process::blocking::Pty;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use uuid::Uuid;

use crate::daemon::output_session::SessionInbox;
use crate::protocol::{AgentStatus, AgentSummary};

const GRACEFUL_TERMINATE_GRACE: Duration = Duration::from_secs(1);
const GRACEFUL_TERMINATE_POLL: Duration = Duration::from_millis(100);

#[derive(Clone)]
pub struct AgentEntry {
    pub id: Uuid,
    pub name: String,
    pub cwd: Option<PathBuf>,
    pub created_at: i64,
    pub pty: Arc<Pty>,
    pub child: Arc<Mutex<Child>>,
    /// Serialises concurrent writers (attach stdin, send) so multi-byte
    /// payloads don't interleave at the syscall level.
    pub write_lock: Arc<Mutex<()>>,
    pub inbox: SessionInbox,
    pub attached: Arc<AtomicBool>,
    pub dead: Arc<AtomicBool>,
}

impl AgentEntry {
    pub fn summary(&self) -> AgentSummary {
        AgentSummary {
            id: self.id,
            name: self.name.clone(),
            status: if self.dead.load(Ordering::Relaxed) {
                AgentStatus::Dead
            } else {
                AgentStatus::Running
            },
            cwd: self.cwd.clone(),
            created_at: self.created_at,
        }
    }
}

#[derive(Default)]
struct Inner {
    by_id: HashMap<Uuid, AgentEntry>,
    by_name: HashMap<String, Uuid>,
}

#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<Inner>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// A poisoned registry mutex means another thread panicked while
    /// holding our own internal state — there's nothing meaningful to
    /// recover, and continuing risks corrupted bookkeeping. Centralised
    /// here so the `unwrap` lint is explicitly silenced once.
    #[allow(clippy::unwrap_used)]
    fn lock_inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// Insert a fresh entry. Returns Err if the name is already taken.
    pub fn insert(&self, entry: AgentEntry) -> Result<()> {
        let mut inner = self.lock_inner();
        if inner.by_name.contains_key(&entry.name) {
            return Err(anyhow!("name already in use: {}", entry.name));
        }
        let id = entry.id;
        inner.by_name.insert(entry.name.clone(), id);
        inner.by_id.insert(id, entry);
        Ok(())
    }

    /// Return a clone of the named agent's entry, or `None` if missing.
    pub fn lookup(&self, name: &str) -> Option<AgentEntry> {
        let inner = self.lock_inner();
        let id = inner.by_name.get(name)?;
        inner.by_id.get(id).cloned()
    }

    /// Return summaries of every registered agent. Reads each entry's
    /// atomic `dead` flag (kept up to date by its session task) and also
    /// double-checks via `try_wait` in case the session task hasn't yet
    /// observed the EOF.
    pub fn list(&self) -> Vec<AgentSummary> {
        let inner = self.lock_inner();
        let mut summaries: Vec<AgentSummary> = Vec::with_capacity(inner.by_id.len());
        for entry in inner.by_id.values() {
            if !entry.dead.load(Ordering::Relaxed)
                && let Ok(mut child) = entry.child.lock()
                && let Ok(Some(_)) = child.try_wait()
            {
                entry.dead.store(true, Ordering::Relaxed);
            }
            summaries.push(entry.summary());
        }
        summaries
    }

    /// Remove the agent with `name`, terminating the child if alive.
    /// Returns the removed entry on success, or None if not found.
    ///
    /// With `force = false` (default), sends SIGTERM, polls for up to
    /// `GRACEFUL_TERMINATE_GRACE`, then falls back to SIGKILL if the
    /// child is still running. With `force = true`, skips the SIGTERM
    /// step and goes straight to SIGKILL. Blocks for up to the grace
    /// period; callers in async contexts should wrap in `spawn_blocking`.
    pub fn remove(&self, name: &str, force: bool) -> Option<AgentEntry> {
        // Drop the registry lock before the (potentially second-long)
        // termination so other registry operations stay responsive.
        let entry = {
            let mut inner = self.lock_inner();
            let id = inner.by_name.remove(name)?;
            inner.by_id.remove(&id)?
        };
        terminate_entry(&entry, force);
        Some(entry)
    }

    /// Kill every registered agent and clear the registry. Used during
    /// daemon shutdown so live agent processes do not become orphans of
    /// init when the daemon exits.
    pub fn shutdown_all(&self) {
        let mut inner = self.lock_inner();
        for (_, entry) in inner.by_id.drain() {
            if let Ok(mut child) = entry.child.lock()
                && let Err(e) = child.kill()
            {
                debug!("kill on shutdown failed for pid {}: {e}", child.id());
            }
        }
        inner.by_name.clear();
    }

    /// Drop every registered agent whose process has exited. Returns the
    /// names that were removed.
    pub fn prune_dead(&self) -> Vec<String> {
        let mut inner = self.lock_inner();

        // First sweep: refresh `dead` flags for entries we haven't yet
        // observed exit on.
        for entry in inner.by_id.values() {
            if !entry.dead.load(Ordering::Relaxed)
                && let Ok(mut child) = entry.child.lock()
                && let Ok(Some(_)) = child.try_wait()
            {
                entry.dead.store(true, Ordering::Relaxed);
            }
        }

        // Second sweep: collect dead ids, then remove.
        let dead_ids: Vec<Uuid> = inner
            .by_id
            .iter()
            .filter(|(_, entry)| entry.dead.load(Ordering::Relaxed))
            .map(|(id, _)| *id)
            .collect();

        let mut removed = Vec::with_capacity(dead_ids.len());
        for id in dead_ids {
            if let Some(entry) = inner.by_id.remove(&id) {
                inner.by_name.remove(&entry.name);
                removed.push(entry.name);
            }
        }
        removed
    }

    /// Return the running PID of the agent named `name`, or None if the
    /// agent doesn't exist or has no PID (e.g. the child handle reports
    /// nothing on this platform).
    /// Names of agents that currently have a client attached. Used by
    /// the Shutdown handler to refuse non-forced shutdowns while users
    /// are mid-session.
    pub fn attached_names(&self) -> Vec<String> {
        let inner = self.lock_inner();
        inner
            .by_id
            .values()
            .filter(|e| e.attached.load(Ordering::Relaxed))
            .map(|e| e.name.clone())
            .collect()
    }

    pub fn pid_of(&self, name: &str) -> Option<u32> {
        let inner = self.lock_inner();
        let id = inner.by_name.get(name)?;
        let entry = inner.by_id.get(id)?;
        let child = entry.child.lock().ok()?;
        Some(child.id())
    }
}

fn terminate_entry(entry: &AgentEntry, force: bool) {
    let Some(pid) = entry.child.lock().ok().map(|c| c.id()) else {
        return;
    };

    if force {
        send_sigkill(entry);
        return;
    }

    let nix_pid = Pid::from_raw(pid.cast_signed());
    if let Err(e) = kill(nix_pid, Signal::SIGTERM) {
        if e == nix::errno::Errno::ESRCH {
            return;
        }
        warn!("SIGTERM to pid {pid} failed: {e}; falling back to SIGKILL");
    }

    let deadline = Instant::now() + GRACEFUL_TERMINATE_GRACE;
    while Instant::now() < deadline {
        thread::sleep(GRACEFUL_TERMINATE_POLL);
        if let Ok(mut child) = entry.child.lock()
            && let Ok(Some(_)) = child.try_wait()
        {
            debug!("pid {pid} exited gracefully after SIGTERM");
            return;
        }
    }

    debug!("pid {pid} did not exit within grace period; sending SIGKILL");
    send_sigkill(entry);
}

fn send_sigkill(entry: &AgentEntry) {
    if let Ok(mut child) = entry.child.lock()
        && let Err(e) = child.kill()
    {
        debug!("SIGKILL on pid {} failed: {e}", child.id());
    }
}
