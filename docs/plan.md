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

1. ~~**Client-daemon protocol shape.**~~ **Resolved**, see `docs/protocol.md`. Length-prefixed `postcard` envelopes over a Unix socket; message variants align with CLI verbs.
2. **Daemon lifecycle.** Auto-start strategy on first client invocation (sketched in `docs/protocol.md`), single-instance guarantee via socket bind, graceful shutdown, what happens to live sessions if the daemon crashes (they die; reconcile registry on next start). The auto-start mechanism (how the client forks the daemon process and waits for the socket) still needs to be pinned down.
3. **PTY size and resize.** How a client communicates its terminal size to the daemon, how resizes propagate to the PTY, what happens when no client is attached. (Defaults captured in `docs/protocol.md`; need to confirm behavior when the connected client is the only sizing authority and disconnects — keep last size, or reset to 80x24?)
4. **Repository layout.** Concrete `src/` module split (daemon vs. client vs. shared). Likely:
   - `src/main.rs` (dispatch)
   - `src/cli.rs` (clap)
   - `src/daemon/` (server, session, lifecycle)
   - `src/client/` (attach loop)
   - `src/protocol.rs` (shared)
   - `src/registry.rs`
   - `src/adapter/` (kitty)
   - `src/paths.rs`, `src/error.rs`
5. **SQLite schema.** Minimum: `agents(id, name, worktree, cmd, created_at, updated_at, status)`. `parent_id` and `tags` deferred.
6. **Config file.** TOML at `$XDG_CONFIG_HOME/picoswarm/config.toml`. May not be needed for MVP at all — defaults plus CLI flags may be enough.

### Resolved (captured here for visibility)

- Detach key: `Ctrl-\` (byte `0x1c`). Matches abduco; avoids tmux/ssh collisions.
- Ring buffer size: 64 KB per session, in-memory only.
- Encoding: `postcard` (replaced `bincode`, which is unmaintained per RUSTSEC-2025-0141).

---

## Concrete next steps

In order:

1. ~~Initialize the Cargo project~~ **Done**.
2. ~~Decide the client-daemon protocol shape and capture it in `docs/protocol.md`.~~ **Done**.
3. Settle the remaining open design decisions (daemon lifecycle details, PTY size handling, repo layout, SQLite schema). These are best decided in the order daemon-shape → repo-layout → schema.
4. Implement the daemon: socket listener, session table, PTY spawn via `portable-pty`, ring buffer, basic message loop.
5. Implement the client `attach` loop: connect to the daemon, forward stdin/stdout, handle the detach key (`Ctrl-\`).
6. Implement `pswarm run`, `pswarm ls`, `pswarm rm`, `pswarm doctor`. Wire the registry (rusqlite).
7. Wire the kitty adapter: on `pswarm run`, open a new kitty tab and run `pswarm attach <name>` in it.
8. Live-use the MVP. Capture friction in this file, decide what (if anything) graduates from "Next" into the next iteration.
