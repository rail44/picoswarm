//! Wire protocol shared between the picoswarm client and daemon.
//!
//! Frames are length-prefixed `postcard` payloads over a Unix socket.
//! See `docs/protocol.md` for the full specification.

use anyhow::{Result, anyhow};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 9;

/// Hard cap on a single frame's payload size, to keep a malformed length
/// prefix from triggering an arbitrarily large allocation.
const MAX_FRAME_LEN: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TermSize {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRequest {
    pub name: String,
    pub cmd: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub initial_size: TermSize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    Running,
    Dead,
    /// PTY-less entry created by `pswarm register`. Has an inbox and
    /// can record lifecycle events but does not own a child process.
    Registered,
}

/// Lifecycle events an agent (or its plugin glue) can report back to
/// the daemon via `pswarm event`. The set is intentionally small — a
/// new variant requires a `PROTOCOL_VERSION` bump.
///
/// `ValueEnum` is derived so the same type drives clap's CLI parser
/// in `pswarm event <event>`; clap is already a workspace dep.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, clap::ValueEnum)]
pub enum Event {
    /// Agent finished its current turn and is waiting for the next
    /// user input. Wired from Claude Code's `Stop` hook, Codex's
    /// `agent-turn-complete`, Gemini's `AfterAgent`, etc.
    Idle,
    /// Agent needs human input that isn't a normal turn — e.g. a
    /// permission prompt. Wired from Claude Code's `Notification`
    /// hook.
    Attention,
    /// Agent's session is ending. Wired from Claude Code's
    /// `SessionEnd` hook. Distinct from process death (which the
    /// daemon detects via PTY EOF).
    Exit,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Idle => "idle",
            Self::Attention => "attention",
            Self::Exit => "exit",
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub cwd: Option<PathBuf>,
    pub created_at: i64,
    /// Most recent lifecycle event reported by the agent (via the
    /// `pswarm event` subcommand), with the unix timestamp at which
    /// the daemon recorded it. `None` when no event has been seen.
    pub last_event: Option<(Event, i64)>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    NotFound,
    NameTaken,
    AlreadyAttached,
    SpawnFailed,
    ProtocolMismatch,
    /// Returned by `Shutdown` when one or more agents have an attached
    /// client and the request did not set `force`.
    ActiveAttachments,
    /// Returned when a request that needs a PTY (`Send`, `View`,
    /// `Attach`, `GetCwd`) targets an entry created by `Register`.
    /// Such entries have no child process, only an inbox and event slot.
    NoPty,
    /// Returned when a request supplies an agent name that fails the
    /// shared name validation (length, allowed chars, reserved
    /// literals). Distinguished from `Internal` so clients can tell a
    /// user typo from a daemon bug.
    InvalidName,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientToDaemon {
    Hello {
        protocol_version: u32,
    },
    Run(RunRequest),
    Ls,
    Attach {
        name: String,
        initial_size: TermSize,
    },
    Detach,
    Resize(TermSize),
    Stdin(Vec<u8>),
    Rm {
        name: String,
        force: bool,
    },
    Ping,
    /// Ask the daemon to terminate gracefully: stop accepting new
    /// connections, kill all live agents, remove the socket, and exit.
    /// If `force` is false and any agent is currently attached, the
    /// daemon refuses with `Error { ActiveAttachments, ... }`.
    Shutdown {
        force: bool,
    },
    /// Ask the daemon for runtime stats (used by `pswarm doctor`).
    Status,
    /// Sweep dead agents from the registry (used by `pswarm clean`).
    Clean,
    /// Ask the daemon for the current working directory of a running
    /// agent's process (read from `/proc/<pid>/cwd`).
    GetCwd {
        name: String,
    },
    /// Write `payload` to the named agent's PTY without attaching. The
    /// daemon does not interpret the bytes — they are forwarded verbatim
    /// to the writer side of the agent's PTY. Sending while a client is
    /// attached is allowed; the bytes will interleave with the attached
    /// client's stdin.
    Send {
        name: String,
        payload: Vec<u8>,
    },
    /// One-shot read of the named agent's recent PTY output (the ring
    /// buffer's current contents). Read-only, no side effects on the
    /// agent. Daemon responds with a single `Stdout(backlog)` followed
    /// by closing the connection.
    View {
        name: String,
    },
    /// Record a lifecycle event reported by the agent itself. Sent
    /// from `pswarm event <name> <event>` and from the bundled Claude
    /// Code plugin's hook scripts. The daemon responds with `Ok` on
    /// success or `Error { NotFound, ... }` if the agent is gone.
    Event {
        name: String,
        event: Event,
    },
    /// Register a virtual (PTY-less) agent. Used by drivers — Claude
    /// sessions that orchestrate spawned agents — to take a unique
    /// identity in the registry, get an inbox, and record lifecycle
    /// events, without the daemon spawning a child process for them.
    /// The entry shows in `pswarm ls` with status `Registered`; the
    /// PTY-bound subcommands (`send`, `view`, `attach`, `cwd`) reject
    /// it with `ErrorCode::NoPty`. Daemon responds with `Ok` or
    /// `Error { NameTaken, ... }`.
    Register {
        name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonToClient {
    Hello {
        protocol_version: u32,
    },
    Ok,
    Error {
        code: ErrorCode,
        message: String,
    },
    RunResult {
        id: Uuid,
        name: String,
    },
    AgentList(Vec<AgentSummary>),
    Stdout(Vec<u8>),
    SessionEnded {
        exit_code: Option<i32>,
    },
    Pong,
    /// Response to `ClientToDaemon::Status`.
    Status {
        /// Daemon process uptime in seconds.
        uptime_seconds: u64,
        /// Number of agents currently in the registry.
        agent_count: u32,
        /// Daemon binary version (CARGO_PKG_VERSION at build time).
        version: String,
    },
    /// Response to `ClientToDaemon::Clean`. Lists the names that were
    /// removed from the registry.
    Cleaned {
        removed: Vec<String>,
    },
    /// Response to `ClientToDaemon::GetCwd`. `path` is `None` when the
    /// agent has no PID, the `/proc/<pid>/cwd` symlink can't be read, or
    /// the daemon does not support cwd discovery on this platform.
    AgentCwd {
        path: Option<PathBuf>,
    },
}

/// Read one length-prefixed `postcard`-encoded message from `reader`.
pub async fn read_msg<T, R>(reader: &mut R) -> Result<T>
where
    T: DeserializeOwned,
    R: AsyncReadExt + Unpin,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_FRAME_LEN {
        return Err(anyhow!(
            "frame size {len} exceeds the {MAX_FRAME_LEN}-byte limit"
        ));
    }
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf).await?;
    Ok(postcard::from_bytes(&buf)?)
}

/// Write one length-prefixed `postcard`-encoded message to `writer`.
///
/// `T: Serialize` and `W: AsyncWriteExt` aren't constrained `Send`, so
/// the returned future isn't `Send` either. Every caller awaits in the
/// same task that owns `writer`, so cross-task scheduling never
/// happens — we accept a non-Send future on purpose.
#[allow(clippy::future_not_send)]
pub async fn write_msg<T, W>(writer: &mut W, msg: &T) -> Result<()>
where
    T: Serialize,
    W: AsyncWriteExt + Unpin,
{
    let bytes = postcard::to_allocvec(msg)?;
    let len: u32 = bytes
        .len()
        .try_into()
        .map_err(|_| anyhow!("encoded message exceeds u32 length prefix"))?;
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&bytes).await?;
    writer.flush().await?;
    Ok(())
}
