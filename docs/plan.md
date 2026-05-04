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

1. **Client-daemon protocol shape.** Request/response messages, framing on the Unix socket, how an attach session multiplexes control messages and PTY bytes on the same connection (or two connections). Encoding: `bincode` is the default unless a reason emerges to use length-prefixed JSON.
2. **Daemon lifecycle.** Auto-start strategy on first client invocation, single-instance guarantee (lockfile? socket-based?), graceful shutdown, what happens to live sessions if the daemon crashes (likely: they die; document it).
3. **Output ring buffer.** In-memory only or spill to disk? What size? Should there also be a per-session log file on disk for `peek`-style use cases later?
4. **PTY size and resize.** How a client communicates its terminal size to the daemon, how resizes propagate to the PTY, what happens when no client is attached.
5. **Detach key.** Default escape sequence used by the client to leave an attach session. Configurable later; needs a reasonable default (e.g. `Ctrl-\` like abduco, since `Ctrl-b` collides with tmux users' muscle memory and `Ctrl-]` is used by ssh).
6. **Repository layout.** Concrete `src/` module split (daemon vs. client vs. shared). Likely:
   - `src/main.rs` (dispatch)
   - `src/cli.rs` (clap)
   - `src/daemon/` (server, session, lifecycle)
   - `src/client/` (attach loop)
   - `src/protocol.rs` (shared)
   - `src/registry.rs`
   - `src/adapter/` (kitty)
   - `src/paths.rs`, `src/error.rs`
7. **SQLite schema.** Minimum: `agents(id, name, worktree, cmd, created_at, updated_at)`. `parent_id` and `tags` deferred. `session_handle` (the daemon's identifier for the PTY, e.g. an integer or UUID) is part of the row.
8. **Config file.** TOML at `$XDG_CONFIG_HOME/picoswarm/config.toml`. May not be needed for MVP at all — defaults plus CLI flags may be enough.

---

## Concrete next steps

In order:

1. Initialize the Cargo project: `Cargo.toml` with the dependencies listed in CLAUDE.md "Tech choices", a stub `src/main.rs` with a clap skeleton, `.gitignore`, placeholder `README.md`. (No source modules yet — let them emerge as decisions are made.)
2. Decide the client-daemon protocol shape (open question 1) on paper before writing daemon code. Capture the chosen shape in `docs/plan.md` or a new `docs/protocol.md`.
3. Implement the daemon: socket listener, session table, PTY spawn via `portable-pty`, ring buffer, basic message loop.
4. Implement the client `attach` loop: connect, forward stdin/stdout, handle the detach key.
5. Implement `pswarm new`, `pswarm ls`, `pswarm kill`, `pswarm doctor`. Wire registry (rusqlite).
6. Wire kitty adapter: on `pswarm new`, open a new kitty tab and run `pswarm attach <name>` in it.
7. Live-use the MVP. Capture friction in this file, decide what (if anything) graduates from "Next" into the next iteration.
