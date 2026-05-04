//! Wire protocol shared between the picoswarm client and daemon.
//!
//! Frames are length-prefixed `postcard` payloads over a Unix socket.
//! See `docs/protocol.md` for the full specification.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;

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
    Idle,
    Dead,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub cwd: Option<PathBuf>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ErrorCode {
    NotFound,
    NameTaken,
    AlreadyAttached,
    SpawnFailed,
    ProtocolMismatch,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientToDaemon {
    Hello { protocol_version: u32 },
    Run(RunRequest),
    Ls,
    Attach { name: String, initial_size: TermSize },
    Detach,
    Resize(TermSize),
    Stdin(Vec<u8>),
    Rm { name: String, force: bool },
    Ping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonToClient {
    Hello { protocol_version: u32 },
    Ok,
    Error { code: ErrorCode, message: String },
    RunResult { id: Uuid, name: String },
    AgentList(Vec<AgentSummary>),
    Stdout(Vec<u8>),
    SessionEnded { exit_code: Option<i32> },
    Pong,
}
