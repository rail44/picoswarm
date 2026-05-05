# Client-Daemon Protocol

This document specifies the wire protocol between the picoswarm CLI client and the picoswarm daemon. Protocol version: **5**.

## Transport

- **Socket path**: `$XDG_RUNTIME_DIR/picoswarm/sock`. If `$XDG_RUNTIME_DIR` is not set, fall back to `$HOME/.local/run/picoswarm/sock`. The parent directory is created with mode `0700` if missing; the socket itself is mode `0600` (only the owning user can connect).
- **Single-instance**: only one daemon may bind the socket. The daemon detects collision by attempting to bind; if the socket exists and a connection succeeds, it exits silently. If the socket exists but no daemon answers, it removes the stale socket and re-binds.
- **Auto-start**: when a client connects and gets `ECONNREFUSED` or `ENOENT`, it forks a daemon (`pswarm daemon` invocation) and retries with backoff up to 2 seconds.

## Framing

Each message is a length-prefixed frame:

```
[ u32 length, little-endian ] [ payload bytes ]
```

The payload is a [`postcard`](https://docs.rs/postcard) serialization of one envelope variant (see "Messages" below).

## Versioning handshake

Immediately after a TCP-style `accept`, both sides exchange a `Hello`:

1. The client sends `ClientToDaemon::Hello { protocol_version }` first.
2. The daemon responds with `DaemonToClient::Hello { protocol_version }` if the version matches, or `DaemonToClient::Error { code: ProtocolMismatch, message }` and closes if not.

The version is bumped any time the wire format changes incompatibly (new variants count: removing or reordering a variant changes its postcard discriminant, so any addition that's not a strict append is breaking — and `postcard` deserialization rejects unknown discriminants regardless). On mismatch, the daemon returns `Error { ProtocolMismatch }` and closes; the client surfaces a message suggesting `pswarm daemon restart` to reload the binary.

## Messages

```rust
// shared types

pub struct TermSize { pub rows: u16, pub cols: u16 }

pub struct RunRequest {
    pub name: String,
    pub cmd: Vec<String>,         // argv; if empty, daemon defaults to ["claude"]
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub initial_size: TermSize,
}

pub struct AgentSummary {
    pub id: Uuid,
    pub name: String,
    pub status: AgentStatus,
    pub cwd: Option<PathBuf>,
    pub created_at: i64,          // unix seconds
}

pub enum AgentStatus { Running, Dead }

pub enum ErrorCode {
    NotFound,
    NameTaken,
    AlreadyAttached,
    SpawnFailed,
    ProtocolMismatch,
    Internal,
}

// envelopes

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
    Shutdown,                       // ask the daemon to exit gracefully
    Status,                         // daemon stats (used by `pswarm doctor`)
    Clean,                          // sweep dead agents from the registry
    GetCwd { name: String },        // read /proc/<pid>/cwd for a running agent
    Send { name: String, payload: Vec<u8> }, // write bytes to an agent's PTY without attaching
}

pub enum DaemonToClient {
    Hello { protocol_version: u32 },
    Ok,
    Error { code: ErrorCode, message: String },
    RunResult { id: Uuid, name: String },
    AgentList(Vec<AgentSummary>),
    Stdout(Vec<u8>),
    SessionEnded { exit_code: Option<i32> },
    Pong,
    Status {                        // response to ClientToDaemon::Status
        uptime_seconds: u64,
        agent_count: u32,
        version: String,
    },
    Cleaned { removed: Vec<String> },  // response to ClientToDaemon::Clean
    AgentCwd { path: Option<PathBuf> }, // response to ClientToDaemon::GetCwd
}
```

Note: the message names align with the user-facing CLI verbs (`Run`, `Rm`, …) rather than introducing a separate technical vocabulary. If a future operation needs to spawn a process without exposing its PTY (e.g. background hooks), it will be added as a distinct message at that point.

## Per-command flows

`H` denotes the Hello exchange (omitted from each diagram for brevity).

### `pswarm run <NAME> [-- CMD...]`

```
C → D : Run(RunRequest{ name, cmd, cwd, env, initial_size })
D → C : RunResult { id, name }     |  Error { NameTaken | SpawnFailed | ... }
< close >
```

The daemon spawns the process via `portable-pty`, registers the agent, and returns the assigned id.

### `pswarm ls [--json]`

```
C → D : Ls
D → C : AgentList([...])
< close >
```

The `--json` flag is a client-side rendering choice; the protocol always returns the same `AgentList`.

### `pswarm rm <NAME> [--force]`

```
C → D : Rm { name, force }
D → C : Ok                          |  Error { NotFound }
< close >
```

If the process is alive, the daemon sends `SIGTERM` then `SIGKILL` after a short grace period, unless `force = true` (immediate `SIGKILL`). The registry row is removed regardless of the prior process state.

### `pswarm attach <NAME>`

```
C → D : Attach { name, initial_size }
D → C : Ok                          |  Error { NotFound | AlreadyAttached }
D → C : Stdout(...)                  // recent ring-buffer backlog, drained transparently
[bidirectional streaming]
  C → D : Stdin(...) | Resize(...) | Detach
  D → C : Stdout(...) | SessionEnded { exit_code }
[connection closes when either Detach or SessionEnded fires]
```

The backlog is sent as ordinary `Stdout` frames; the client cannot tell where backlog ends and live output begins.

### `pswarm doctor`

```
C → D : Status
D → C : Status { uptime_seconds, agent_count, version }
< close >
```

Plus client-side local checks printed before the daemon round-trip: socket path, daemon log path, kitty `KITTY_LISTEN_ON` presence, etc. (`Ping`/`Pong` is still defined for liveness probing but `doctor` now uses `Status` to surface uptime / agent count / daemon binary version.)

### `pswarm clean`

```
C → D : Clean
D → C : Cleaned { removed: [name, ...] }
< close >
```

The daemon refreshes each entry's `dead` flag via `try_wait`, then drops every entry observed dead. `removed` lists the names that were swept (empty if there was nothing to clean).

### `pswarm cwd <NAME>`

```
C → D : GetCwd { name }
D → C : AgentCwd { path: Some(PathBuf) }   // success
      | AgentCwd { path: None }            // pid known but cwd unreadable / unsupported platform
      | Error { NotFound }                  // no such agent / agent has no live pid
< close >
```

The daemon reads `/proc/<pid>/cwd` (Linux) and returns the resolved symlink. On non-Linux platforms it returns `AgentCwd { path: None }`.

### `pswarm send <NAME> [TEXT]`

```
C → D : Send { name, payload }
D → C : Ok                          |  Error { NotFound | Internal }
< close >
```

The daemon does not interpret `payload`; it forwards the bytes verbatim to the writer side of the agent's PTY. The CLI ensures `payload` ends with a newline (so Claude Code submits the message) unless the input already ends with one. Sending while another client is attached is allowed; the bytes interleave with the attached client's stdin. There is no built-in send-to-self guard at this protocol version — the caller is responsible for not invoking the CLI on its own agent.

### `pswarm daemon stop` / `restart`

```
C → D : Shutdown
D → C : Ok
< close >
[daemon stops accepting new connections, kills every live agent, removes the socket, and exits]
```

`restart` is `stop` followed by an explicit `pswarm daemon start` re-spawn from the client side.

## Defaults and constants

| Item | Value |
|---|---|
| Protocol version | `5` |
| Stdin/Stdout chunk cap | 16 KB per frame |
| Per-session ring buffer | 64 KB, in-memory only |
| Default agent command | `claude` |
| Detach key | `Ctrl-\` (byte `0x1c`) |
| Initial PTY size at spawn | 80 × 24 (until the spawning client supplies its size on first attach) |
| PTY size after client detach | last size from the most recent attach, kept until the next attach resizes it |
| Auto-start retry window | 2 seconds, exponential backoff |
| Graceful shutdown window after `SIGTERM` | 1 second before `SIGKILL` |

## Failure modes

| Situation | Behavior |
|---|---|
| Client disconnects mid-attach without sending `Detach` | Daemon treats it as a detach: PTY stays alive. The agent's status remains `Running` since the underlying process is still up; reporting "no client attached" is not modelled in `AgentStatus` after the v4 cleanup. |
| Daemon dies (panic, SIGKILL) | The registry is in-memory only, so it dies with the daemon. Live agent processes are children of the daemon and normally die with it; on a hard kill they may briefly survive as orphans of init (see issues/21-orphan-prevention.md). On next `pswarm` invocation a fresh daemon starts with an empty registry. |
| Process inside an agent exits | Daemon emits `SessionEnded { exit_code }` to any attached client, marks the agent `Dead`. Registry row remains until `pswarm rm` removes it. |
| Two clients try to attach to the same agent | Second `Attach` returns `Error { AlreadyAttached }`. Multi-client read-only attach is a future feature, not MVP. |
| Protocol version mismatch | Daemon returns `Error { ProtocolMismatch }` and closes. The client surfaces an error suggesting the daemon needs to be restarted to match the upgraded binary. A dedicated `pswarm daemon-restart` subcommand may be added later. |

## Out of scope (for this protocol version)

- Streaming logs without attaching (`peek` / `logs` subcommands).
- Multi-client concurrent attach.
- Authenticated multi-user access (the socket relies on filesystem permissions only).
