# picoswarm

A lightweight orchestrator for running multiple Claude Code and similar CLI coding agents in parallel. Written in Rust.

- **Project name**: picoswarm
- **CLI binary name**: `pswarm`

This file holds the time-invariant direction: architecture, conventions, hard constraints, glossary. For the current MVP scope, roadmap, and open design decisions, see `docs/plan.md` (always consult it before proposing the next implementation step). For background and how this direction was reached, see `docs/design-discussion.md` — that document predates a major thesis revision (see "History" below) and is preserved as a snapshot of earlier reasoning, not as current direction.

---

## Confirmed direction

### Scope

picoswarm is a self-contained agent orchestrator. It owns a small PTY daemon to keep agent sessions alive, plus an agent registry and lifecycle CLI on top.

In scope:

- Agent registry (id / name / worktree / parent-child / tags / status / session handle)
- Agent lifecycle operations (spawn / list / attach / detach / send / kill / link)
- A built-in PTY session daemon (one PTY per agent, kept alive across CLI invocations)

Out of scope:

- Screen splitting and window management (delegated to adapters that drive kitty / wezterm / etc.)
- Job DAG / workflow engine
- Scheduling / CI integration
- Implementing a full terminal emulator (we forward bytes; we do not emulate VT state in MVP)

### Architecture

picoswarm is a single binary that operates in two roles:

- **Daemon role** (`pswarm daemon`, usually auto-started): Long-lived process. Owns all PTYs for spawned agents. Listens on a Unix socket. Survives across CLI invocations.
- **Client role** (every other `pswarm <subcommand>`): Short-lived. Connects to the daemon's Unix socket and issues requests. Auto-starts the daemon on first use if not running.

Components:

- **daemon**: Holds PTYs via `portable-pty`. Multiplexes I/O between PTYs and connected clients. Maintains per-session output ring buffers so a reattaching client can see recent output.
- **registry**: The daemon's in-memory map from agent id/name to session metadata (name, worktree, status, etc.). Encapsulated by a `Registry` struct. Not persisted to disk: when the daemon dies, its agent processes die with it, so the registry has nothing meaningful to outlive. Persistence may be added later if a use case appears that justifies it.
- **adapter (PaneHost)**: Abstraction over the user's terminal/multiplexer for opening, focusing, and closing the windows that host attached clients. Implemented as the `PaneHost` trait, fully decoupled from core. First-class adapter: kitty.
- **single-binary CLI**: An agent (Claude Code itself) must be able to operate its own orchestrator via the `pswarm` command. Do not implement this as a fish/bash function (subshells cannot invoke it).

### Tech choices

- Rust
- `portable-pty` for PTY operations
- `tokio` for async I/O in the daemon
- `serde` + `postcard` for the client-daemon protocol
- (no separate registry crate — the daemon keeps agent metadata in memory)
- `clap` (derive) for the CLI
- `directories` for XDG paths
- `thiserror` / `anyhow` / `tracing` for errors and logging
- `uuid` for agent ids

Not currently used (would only be added if a concrete need arises):

- A VT parser (`vte`, or `libghostty-vt` once stable) for reconstructing screen state on reattach. The MVP forwards raw bytes plus a recent-output buffer, which is sufficient until proven otherwise.

### Implementation conventions

- The daemon and client roles share types via internal modules; the protocol module is the only thing both must agree on.
- Adapters are constructed once at CLI startup from config and passed as handles; do not call adapter implementations directly from core logic.
- Use `thiserror` for named error types at the library/low level; collapse to `anyhow::Result` in `main` and command handlers.
- User-facing errors must explain both what failed and what to do next.
- Tests use mock implementations behind the same internal traits the production code uses.

### Language of artifacts

Commit messages, documentation (README, files under `docs/`), and in-code comments are written in English.

---

## Decisions that must not drift

If you are about to propose something that violates one of these — for "simplicity" or to satisfy a feature request — confirm with the human first.

- The primary UX is CLI subcommands that compose with the shell. Do not make an interactive TUI the primary entry point. A TUI may be added later as a complementary view, but `pswarm` must remain useful as one-shot commands (this is the main differentiator from existing TUI-driven managers like ccmanager).
- picoswarm owns its session daemon. Do not introduce a hard dependency on tmux, shpool, or other external session managers as the primary path.
- Adapters (display / window management) stay fully decoupled from core. Core code is environment-agnostic.
- The registry's primary key is `agent_id` (UUID); a session handle is an attribute of the agent, not its identity.
- Agents must be able to invoke the CLI on themselves (single binary, callable from a subshell).
- Do not build in screen splitting, do not build in a job DAG, do not build in scheduling.
- Do not implement a full terminal emulator. Output is forwarded as raw bytes plus a recent-output ring buffer. If reattach UX requires more, introduce a focused VT parser dependency rather than rolling our own.

---

## Glossary

- **agent**: A single instance of a CLI coding agent (Claude Code, OpenCode, Codex CLI, etc.).
- **session**: A PTY held by the picoswarm daemon, hosting exactly one agent process.
- **registry**: The daemon's in-memory store of agent metadata and the agent ↔ session mapping. Not persisted to disk in MVP.
- **daemon**: The long-lived `pswarm daemon` process that owns all PTYs and serves clients over a Unix socket.
- **client**: A short-lived `pswarm <subcommand>` invocation that talks to the daemon.
- **adapter (PaneHost)**: Abstraction over an external tool (kitty, wezterm, etc.) that opens, focuses, and closes the windows hosting attached clients.
- **worktree**: A git worktree. The isolation unit when an agent is given its own working directory.

---

## History

The project initially planned to be a "pure layer" delegating PTY persistence to existing tools (tmux / shpool / none) behind a `Backend` trait. After investigating available libraries (libshpool, retach, libghostty-vt, ai-session, portable-pty) and the user's environment and preferences, that thesis was revised: picoswarm now owns its session daemon, dropping the backend-pluggability abstraction in favor of a single in-house implementation. The earlier reasoning is preserved in `docs/design-discussion.md` as historical context.
