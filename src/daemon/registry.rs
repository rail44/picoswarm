//! In-memory agent registry held inside the daemon process.
//!
//! The registry owns every live agent's PTY master and Child handle, plus
//! a per-agent ring buffer of recent output. There is no on-disk
//! persistence: when the daemon dies its child processes die with it, so
//! reviving registry rows would describe nothing real.

use anyhow::{anyhow, Result};
use portable_pty::{Child, MasterPty};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::protocol::{AgentStatus, AgentSummary};

pub const RING_BUFFER_BYTES: usize = 64 * 1024;

pub struct AgentEntry {
    pub id: Uuid,
    pub name: String,
    pub cwd: Option<PathBuf>,
    pub created_at: i64,
    pub master: Box<dyn MasterPty + Send>,
    pub child: Box<dyn Child + Send + Sync>,
    pub ring_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Set once the child has been observed to have exited.
    pub dead: bool,
}

impl AgentEntry {
    pub fn summary(&self) -> AgentSummary {
        AgentSummary {
            id: self.id,
            name: self.name.clone(),
            status: if self.dead {
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

    /// Reconcile each entry's `dead` flag and return summaries.
    pub fn refresh_and_list(&self) -> Vec<AgentSummary> {
        let mut inner = self.inner.lock().unwrap();
        for entry in inner.by_id.values_mut() {
            if entry.dead {
                continue;
            }
            if let Ok(Some(_)) = entry.child.try_wait() {
                entry.dead = true;
            }
        }
        inner.by_id.values().map(AgentEntry::summary).collect()
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
        let mut entry = inner.by_id.remove(&id)?;
        let _ = entry.child.kill();
        Some(entry)
    }
}
