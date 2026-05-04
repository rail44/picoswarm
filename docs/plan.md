# picoswarm plan

This document holds time-bound information: the current MVP scope, the rough roadmap, and the design decisions that are still open. It complements `CLAUDE.md`, which holds the time-invariant direction.

When proposing the next implementation step, consult this file first.

---

## MVP definition

A personal-use tool for the project owner. Single user, on Linux, running kitty.

Required behaviors:

1. Spawn a Claude Code session under picoswarm's session daemon. The agent inherits the directory the user runs `pswarm run` from, so working in a git worktree is just a `cd` away — picoswarm itself does not manage worktrees.
2. List currently registered agents with their status.
3. Re-attach to a previously spawned agent from any terminal and interact with it normally. Window placement is the user's responsibility (e.g. opening a new kitty tab manually and running `pswarm attach <name>` inside it).
4. Detach from an attached agent without killing it.
5. Kill an agent and clean it out of the registry.
6. A `doctor` subcommand that reports daemon status and environment.

Out of MVP (deferred until a concrete need arises):

- A kitty (or any other) adapter that automates window placement on `pswarm run` / `pswarm attach`. Add this once managing kitty tabs by hand becomes annoying enough to motivate it.
- `send` (programmatic input from outside an attached client)
- `link` / `tag` / parent-child relationships
- Agents calling `pswarm` on themselves (the env-var-carrying-id design)
- Multiple-client concurrent attach (read-only observers)
- Screen restoration on reattach (raw bytes + recent-output buffer is the MVP behavior)

---

## Roadmap (tentative)

The MVP is the only firm commitment. Everything below it is a sketch and will be revised based on what use reveals.

### MVP

Items 1–6 in the MVP definition above.

### Next, once MVP is in daily use

Triggered by gaps that show up in real use, not by this list. Likely candidates, in no particular order:

- A kitty adapter — on `pswarm run`, open a new kitty tab and run `pswarm attach <name>` inside it. The trigger to build this is the user finding manual tab placement tedious.
- `send` (with a clear story for keys vs. text vs. signals)
- Agent self-invocation (env var, `pswarm send self ...` guard)
- Adapters for other terminals (wezterm, zellij, …) once another environment matters
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

- Detach trigger: **`Ctrl-\`** (single key). Recognised in two encoding forms — the raw C0 byte `0x1c` (no keyboard protocol), and the CSI-u sequence `\e[92;5u` (kitty kbd protocol level 1, "disambiguate escape codes"). The matcher carries state across stdin reads so the CSI-u form may straddle reads. Higher kbd-protocol levels (event types `:T`, associated text `;NN`) are not currently parsed; in real use Claude Code only enables level 1, so this works today, but if Claude graduates the matcher needs to grow. The earlier `Ctrl-Q` + `q` two-key trigger was tried (to work around the level-1 ambiguity issue we have since solved) and then dropped — single key + multi-encoding match is sufficient and ergonomically better.
- Stdin debugging: setting `PSWARM_DEBUG_STDIN=/path/to/file` makes the attach client append every raw stdin chunk it sees (as space-separated hex bytes) to that file. Use this to find out what bytes a particular keypress actually produces in the user's terminal when detach is misbehaving.
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
├── protocol.rs        # shared wire types and frame I/O
├── paths.rs           # XDG path helpers (socket, log, config)
├── daemon/
│   ├── mod.rs         # entrypoint for `pswarm daemon`
│   ├── lifecycle.rs   # daemonize + tokio runtime startup
│   ├── server.rs      # Unix socket accept loop + per-client task
│   ├── session.rs     # PTY spawn, ring buffer drain
│   └── registry.rs    # in-memory agent map (lives inside the daemon)
└── client/
    ├── mod.rs         # client subcommand entrypoints
    ├── connection.rs  # connect + Hello handshake + auto-spawn daemon
    ├── doctor.rs
    ├── run.rs
    ├── ls.rs
    ├── rm.rs
    └── attach.rs      # raw-mode + bidirectional streaming loop (TBD)
```

Notes:
- `registry.rs` lives inside `daemon/` because the registry is in-memory and dies with the daemon. Wire types stay in the shared `protocol.rs`; the daemon converts between its internal representation and `AgentSummary` at the protocol boundary.
- An `adapter/` module will appear when the kitty adapter is added (post-MVP).

---

## Concrete next steps

In order:

1. ~~Initialize the Cargo project.~~ **Done**.
2. ~~Decide the client-daemon protocol shape and capture it in `docs/protocol.md`.~~ **Done**.
3. ~~Settle the remaining open design decisions.~~ **Done** (registry persistence, daemonization, env policy, log path, name validation, repo layout — see "Resolved" above). The only thing still open is the no-client PTY size policy, which can be decided when the resize path is wired.
4. ~~Implement the daemon: socket listener, session table, PTY spawn via `portable-pty`, ring buffer.~~ **Done** for `run` / `ls` / `rm` / `doctor` (Hello + Ping/Pong + Run/Ls/Rm). Attach handling still pending.
5. Implement the client `attach` loop: connect to the daemon, forward stdin/stdout, handle the detach key (`Ctrl-\`). Adds the `crossterm` dependency for raw mode and `SIGWINCH`. Daemon-side: per-agent session task that fans out PTY output to the ring buffer and any attached client.
6. Switch `pswarm run` to default-attach (matching the `docker run` mental model that motivated the verb) and add `-d` / `--detach` for the current spawn-only behavior.
7. Live-use the MVP and capture friction in this file. Decide which "Next" candidate (likely the kitty adapter) graduates first based on what actually hurts.
