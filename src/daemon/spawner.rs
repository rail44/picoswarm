//! Dedicated long-lived OS thread that performs every agent fork.
//!
//! `PR_SET_PDEATHSIG(SIGTERM)` (installed in `session::spawn_session`'s
//! `pre_exec`) fires when the *thread* that created the child dies.
//! Running the fork from `tokio::task::spawn_blocking` would tie the
//! signal to whatever blocking-pool worker happened to take the job —
//! and tokio reaps idle blocking workers after ~10 s, which would kill
//! every agent ~10 s after spawn. This module provides a single OS
//! thread that lives for the daemon's entire lifetime, so the signal
//! fires only when the daemon process itself goes away.
//!
//! The thread holds a `tokio::runtime::Handle` and enters it before
//! each spawn so that `session::spawn_session`'s call into
//! `output_session::spawn` (which uses `tokio::spawn`) succeeds.

use anyhow::{Result, anyhow};
use std::sync::mpsc as std_mpsc;
use std::thread;
use tokio::runtime::Handle;
use tokio::sync::oneshot;

use crate::daemon::registry::AgentEntry;
use crate::daemon::session;
use crate::protocol::RunRequest;

type Reply = oneshot::Sender<Result<AgentEntry>>;

#[derive(Clone)]
pub struct Spawner {
    tx: std_mpsc::Sender<(RunRequest, Reply)>,
}

impl Spawner {
    pub fn new(handle: Handle) -> Result<Self> {
        let (tx, rx) = std_mpsc::channel::<(RunRequest, Reply)>();
        thread::Builder::new()
            .name("pswarm-spawner".into())
            .spawn(move || run_loop(&rx, &handle))
            .map_err(|e| anyhow!("failed to start pswarm-spawner thread: {e}"))?;
        Ok(Self { tx })
    }

    pub async fn spawn(&self, req: RunRequest) -> Result<AgentEntry> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send((req, reply_tx))
            .map_err(|_| anyhow!("spawner thread is gone"))?;
        reply_rx
            .await
            .map_err(|_| anyhow!("spawner reply dropped"))?
    }
}

fn run_loop(rx: &std_mpsc::Receiver<(RunRequest, Reply)>, handle: &Handle) {
    while let Ok((req, reply)) = rx.recv() {
        // Enter the runtime so `tokio::spawn` calls inside
        // `session::spawn_session` (notably `output_session::spawn`)
        // can find a reactor.
        let _guard = handle.enter();
        let result = session::spawn_session(req);
        // The waiting task may have been cancelled; nothing to do if so.
        #[allow(clippy::let_underscore_must_use)]
        let _ = reply.send(result);
    }
}
