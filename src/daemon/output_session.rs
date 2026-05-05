//! Per-agent output session task.
//!
//! Each agent owns a long-lived tokio task that:
//!   - receives PTY output chunks from a dedicated OS reader thread,
//!   - keeps a 64 KiB rolling backlog so reattach can replay recent output,
//!   - serves new subscribers an atomic snapshot + an ongoing event stream,
//!   - emits a `SessionEnded` event to all subscribers when the PTY EOFs.
//!
//! The session task is the single point of state mutation for the
//! agent's output, so subscribers cannot race the producer or each other.

use std::collections::VecDeque;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

const RING_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub enum SessionEvent {
    Output(Vec<u8>),
    Ended { exit_code: Option<i32> },
}

pub struct SubscribeReply {
    pub backlog: Vec<u8>,
    pub events: mpsc::UnboundedReceiver<SessionEvent>,
}

enum SessionRequest {
    Subscribe(oneshot::Sender<SubscribeReply>),
}

#[derive(Clone)]
pub struct SessionInbox {
    requests: mpsc::UnboundedSender<SessionRequest>,
}

impl SessionInbox {
    pub async fn subscribe(&self) -> Option<SubscribeReply> {
        let (tx, rx) = oneshot::channel();
        self.requests.send(SessionRequest::Subscribe(tx)).ok()?;
        rx.await.ok()
    }
}

/// Channel the OS reader thread uses to push PTY output into the session task.
/// `None` (channel closed) means the PTY hit EOF.
pub type ChunkSender = mpsc::UnboundedSender<Vec<u8>>;
type ChunkReceiver = mpsc::UnboundedReceiver<Vec<u8>>;

/// Spawn a new session task. The returned `ChunkSender` is wired into the
/// PTY reader thread; the `SessionInbox` is stored on the agent entry so
/// attach handlers can subscribe.
///
/// `on_eof` is invoked when the PTY chunk channel closes; it should return
/// the exit code of the agent process if known.
pub fn spawn<F>(on_eof: F) -> (ChunkSender, SessionInbox)
where
    F: FnOnce() -> Option<i32> + Send + 'static,
{
    let (chunk_tx, chunk_rx) = mpsc::unbounded_channel();
    let (req_tx, req_rx) = mpsc::unbounded_channel();
    tokio::spawn(run(chunk_rx, req_rx, on_eof));
    (chunk_tx, SessionInbox { requests: req_tx })
}

async fn run<F>(
    mut chunks: ChunkReceiver,
    mut requests: mpsc::UnboundedReceiver<SessionRequest>,
    on_eof: F,
) where
    F: FnOnce() -> Option<i32> + Send + 'static,
{
    let mut ring: VecDeque<u8> = VecDeque::with_capacity(RING_BUFFER_BYTES);
    let mut subscribers: Vec<mpsc::UnboundedSender<SessionEvent>> = Vec::new();
    let mut on_eof = Some(on_eof);

    loop {
        tokio::select! {
            chunk = chunks.recv() => match chunk {
                Some(bytes) => {
                    push_into_ring(&mut ring, &bytes);
                    subscribers.retain(|s| {
                        s.send(SessionEvent::Output(bytes.clone())).is_ok()
                    });
                }
                None => {
                    // PTY closed: the agent process is gone.
                    let exit_code = on_eof.take().and_then(|f| f());
                    debug!("session task ending, exit_code={:?}", exit_code);
                    for s in &subscribers {
                        // Receiver may have already detached; we're shutting down anyway.
                        #[allow(clippy::let_underscore_must_use)]
                        let _ = s.send(SessionEvent::Ended { exit_code });
                    }
                    return;
                }
            },
            req = requests.recv() => match req {
                Some(SessionRequest::Subscribe(reply)) => {
                    let (event_tx, event_rx) = mpsc::unbounded_channel();
                    let backlog: Vec<u8> = ring.iter().copied().collect();
                    if reply.send(SubscribeReply { backlog, events: event_rx }).is_ok() {
                        subscribers.push(event_tx);
                    }
                }
                None => {
                    // No more inboxes — the AgentEntry was dropped, but the
                    // PTY may still be alive. Keep draining chunks so the
                    // kernel buffer doesn't fill, but stop accepting subscribers.
                    // Convert into a drain-only loop.
                    while chunks.recv().await.is_some() {
                        // discard
                    }
                    return;
                }
            },
        }
    }
}

fn push_into_ring(ring: &mut VecDeque<u8>, bytes: &[u8]) {
    for &b in bytes {
        if ring.len() == RING_BUFFER_BYTES {
            ring.pop_front();
        }
        ring.push_back(b);
    }
}
