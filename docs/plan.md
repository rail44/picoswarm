# picoswarm plan

This document holds time-bound information: the current MVP scope, the rough roadmap, and the design decisions that are still open. It complements `CLAUDE.md`, which holds the time-invariant direction.

When proposing the next implementation step, consult this file first.

---

## MVP definition

A personal-use tool for the project owner. Single user, on Linux, running kitty.

Required behaviors:

1. Spawn a Claude Code session under picoswarm's session daemon, optionally with a working directory (e.g. a git worktree).
2. List currently registered agents with their status.
3. Re-attach to a previously spawned agent in a kitty tab and interact with it normally.
4. Detach from an attached agent without killing it.
5. Kill an agent and clean it out of the registry.
6. A `doctor` subcommand that reports daemon status and adapter (kitty) availability.

Out of MVP (deferred until a concrete need arises):

- `send` (programmatic input from outside an attached client)
- `link` / `tag` / parent-child relationships
- `--json` output, scripting affordances
- Agents calling `pswarm` on themselves (the env-var-carrying-id design)
- Multiple-client concurrent attach (read-only observers)
- Screen restoration on reattach (raw bytes + recent-output buffer is the MVP behavior)
- Adapters other than kitty

---

## Roadmap (tentative)

The MVP is the only firm commitment. Everything below it is a sketch and will be revised based on what use reveals.

### MVP

Items 1–6 in the MVP definition above.

### Next, once MVP is in daily use

Triggered by gaps that show up in real use, not by this list. Likely candidates, in no particular order:

- `send` (with a clear story for keys vs. text vs. signals)
- Agent self-invocation (env var, `pswarm send self ...` guard)
- A second adapter (wezterm or zellij) once another environment matters
- `tag` / `link` for grouping when there are enough agents to need it
- TUI (`pswarm tui`) only if the CLI list view stops being sufficient

### Later, only if justified

- Screen state restoration on reattach (introduces a VT parser dependency)
- Multi-client read-only observation
- Cross-host (separate machines)
- MCP server exposing picoswarm to agents as structured tools

---

## Open design decisions

These need to be settled before or during MVP implementation. Listed in the order that they likely matter.

1. **PTY size and resize when no client is attached.** Defaults are captured in `docs/protocol.md` (initial 80x24, client reports its size on attach). Still open: when the only attached client disconnects, does the daemon keep the last reported size on the PTY, or reset to 80x24? Working assumption: keep last size; revisit if a use case shows it matters.
2. **Config file.** TOML at `$XDG_CONFIG_HOME/picoswarm/config.toml`. Likely not needed for MVP — defaults plus CLI flags should be enough. Add only when a setting needs to persist between invocations.

### Resolved (captured here for visibility)

- Detach key: `Ctrl-\` (byte `0x1c`). Matches abduco; avoids tmux/ssh collisions.
- Ring buffer size: 64 KB per session, in-memory only.
- Encoding: `postcard` (replaced `bincode`, which is unmaintained per RUSTSEC-2025-0141).
- Registry persistence: **none for MVP**. The daemon holds the agent map in memory; when the daemon dies its child processes die too, so there is nothing meaningful to persist. Add a JSON-on-disk store (or similar) if a future use case justifies it.
- Client-daemon protocol shape: see `docs/protocol.md`. Length-prefixed `postcard` envelopes over a Unix socket; message variants align with CLI verbs.
- Daemonization: use the `daemonize` crate (handles fork / setsid / stdio redirect). The fork happens before the tokio runtime starts.
- PTY env policy: inherit the daemon's full env, plus inject `PSWARM_DAEMON=1`. Per-agent variables (`PSWARM_AGENT_ID`, `PSWARM_AGENT_NAME`, ...) are added later when agent self-invocation lands.
- Daemon log path: `$XDG_STATE_HOME/picoswarm/daemon.log` (fallback `$HOME/.local/state/picoswarm/daemon.log`). Append-only for MVP; rotation is out of scope.
- Agent name validation: must match `^[a-zA-Z0-9._-]{1,64}$`. Enforced at the CLI layer before the request hits the daemon.
- Repository layout: see "Repository layout" below.

### Repository layout

```
src/
├── main.rs            # dispatch (parse CLI, route to client or daemon code path)
├── cli.rs             # clap definitions
├── protocol.rs        # shared wire types
├── error.rs           # crate-wide error types
├── paths.rs           # XDG path helpers (socket, log, config)
├── daemon/
│   ├── mod.rs         # entrypoint for `pswarm daemon`
│   ├── lifecycle.rs   # daemonize + tokio runtime startup
│   ├── server.rs      # Unix socket accept loop + per-client task
│   ├── session.rs     # per-PTY state, ring buffer, fan-out to attached client
│   └── registry.rs    # in-memory agent map (lives inside the daemon)
├── client/
│   ├── mod.rs         # entrypoint for client subcommands
│   └── attach.rs      # raw-mode + bidirectional streaming loop
└── adapter/
    ├── mod.rs         # PaneHost trait + auto-detect
    └── kitty.rs       # kitty remote control implementation
```

Notes:
- `registry.rs` lives inside `daemon/` because the registry is in-memory and dies with the daemon. Wire types stay in the shared `protocol.rs`; the daemon converts between its internal representation and `AgentSummary` at the protocol boundary.
- `cli.rs` will receive the clap definitions currently inlined in `main.rs` once `main.rs` starts dispatching to module entry points.

---

## Concrete next steps

In order:

1. ~~Initialize the Cargo project~~ **Done**.
2. ~~Decide the client-daemon protocol shape and capture it in `docs/protocol.md`.~~ **Done**.
3. ~~Settle the remaining open design decisions.~~ **Done** (registry persistence, daemonization, env policy, log path, name validation, repo layout — see "Resolved" above). The only thing still open is the no-client PTY size policy, which can be decided when the resize path is wired.
4. Implement the daemon: socket listener, session table, PTY spawn via `portable-pty`, ring buffer, basic message loop.
5. Implement the client `attach` loop: connect to the daemon, forward stdin/stdout, handle the detach key (`Ctrl-\`).
6. Implement `pswarm run`, `pswarm ls`, `pswarm rm`, `pswarm doctor`. The daemon holds the registry in memory.
7. Wire the kitty adapter: on `pswarm run`, open a new kitty tab and run `pswarm attach <name>` in it.
8. Live-use the MVP. Capture friction in this file, decide what (if anything) graduates from "Next" into the next iteration.
