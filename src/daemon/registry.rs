//! In-memory agent registry held inside the daemon process.
//!
//! Per-agent state is shared via `Arc<Mutex<>>` (or atomic flags) so the
//! attach handler, the session task, and the registry's own bookkeeping
//! can each operate without holding the global registry lock for long.
//! Nothing is persisted to disk: when the daemon dies its child processes
//! die with it, so reviving registry rows would describe nothing real.

use anyhow::{anyhow, Result};
use portable_pty::{Child, MasterPty};
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::daemon::output_session::SessionInbox;
use crate::protocol::{AgentStatus, AgentSummary};

/// Cloneable handle to a single agent. All fields are `Arc`/`Clone` so the
/// attach handler can hold a snapshot independently of the registry lock.
#[derive(Clone)]
pub struct AgentEntry {
    pub id: Uuid,
    pub name: String,
    pub cwd: Option<PathBuf>,
    pub created_at: i64,
    pub master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    pub child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
    pub writer: Arc<Mutex<Box<dyn Write + Send>>>,
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

    /// Insert a fresh entry. Returns Err if the name is already taken.
    pub fn insert(&self, entry: AgentEntry) -> Result<()> {
        let mut inner = self.inner.lock().unwrap();
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
        let inner = self.inner.lock().unwrap();
        let id = inner.by_name.get(name)?;
        inner.by_id.get(id).cloned()
    }

    /// Return summaries of every registered agent. Reads each entry's
    /// atomic `dead` flag (kept up to date by its session task) and also
    /// double-checks via `try_wait` in case the session task hasn't yet
    /// observed the EOF.
    pub fn list(&self) -> Vec<AgentSummary> {
        let inner = self.inner.lock().unwrap();
        let mut summaries: Vec<AgentSummary> = Vec::with_capacity(inner.by_id.len());
        for entry in inner.by_id.values() {
            if !entry.dead.load(Ordering::Relaxed) {
                if let Ok(mut child) = entry.child.lock() {
                    if let Ok(Some(_)) = child.try_wait() {
                        entry.dead.store(true, Ordering::Relaxed);
                    }
                }
            }
            summaries.push(entry.summary());
        }
        summaries
    }

    /// Remove the agent with `name`, killing the child if alive. Returns
    /// the removed entry on success, or None if not found.
    ///
    /// `force` is currently a no-op; portable-pty's `kill` already sends
    /// SIGKILL on Unix. Once a graceful SIGTERM-then-SIGKILL path lands,
    /// `force = true` will skip the SIGTERM step.
    pub fn remove(&self, name: &str, _force: bool) -> Option<AgentEntry> {
        let mut inner = self.inner.lock().unwrap();
        let id = inner.by_name.remove(name)?;
        let entry = inner.by_id.remove(&id)?;
        if let Ok(mut child) = entry.child.lock() {
            let _ = child.kill();
        }
        Some(entry)
    }

    /// Kill every registered agent and clear the registry. Used during
    /// daemon shutdown so live agent processes do not become orphans of
    /// init when the daemon exits.
    pub fn shutdown_all(&self) {
        let mut inner = self.inner.lock().unwrap();
        for (_, entry) in inner.by_id.drain() {
            if let Ok(mut child) = entry.child.lock() {
                let _ = child.kill();
            }
        }
        inner.by_name.clear();
    }
}
