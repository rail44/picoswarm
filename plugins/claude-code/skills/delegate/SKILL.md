---
name: delegate
description: Use when the user wants to delegate a unit of work to a fresh child Claude agent — spawning it under pswarm, briefing it, coordinating bidirectionally over the inbox, and integrating the result. Pick this over the built-in Task tool when the work needs human visibility or mid-run intervention, runs in a different cwd / model / version, requires OS-level isolation, needs Claude TUI features (skills, plugins, MCP), or has to outlive the current parent turn.
---

# picoswarm: delegate to a child Claude

The simplest 1-parent + 1-child shape on top of picoswarm: spawn one `claude`
session under `pswarm`, brief it, talk to it over the inbox while it works,
and integrate its output back into the parent. This is the foundation for
larger shapes (`fanout`, `pipeline`, …) but on its own is the right tool when
you have a single sub-task that benefits from a separate, visible, durable
session rather than an opaque sub-call.

## When to use pswarm vs the built-in Task tool

The built-in `Task` tool launches a sub-agent inside the parent's process.
`pswarm` spawns a real `claude` TUI in a daemon-owned PTY. The two have
different costs and different reach.

Use **pswarm** when at least one of these applies:

- **Human visibility / mid-run intervention.** The user wants to peek at the
  child with `pswarm attach <name>`, redirect it with `pswarm send`, or
  approve permission prompts that surface in the child's TUI. Task's
  sub-agent runs invisibly until it returns.
- **Bidirectional comm during the run.** Parent and child exchange messages
  while the child is still working (status pings, course corrections,
  intermediate findings). Task is one-shot: prompt in, result out.
- **Lifecycle outlives the parent turn.** The child must keep running while
  the parent ends its turn, gets compacted, or even exits. Task's sub-agent
  dies when the parent turn ends.
- **Different cwd / model / Claude version / env.** The child must run in a
  worktree, on a different model, or with a different `CLAUDE_CONFIG_DIR`.
  Task inherits the parent's everything.
- **OS-level isolation.** The child has its own process, its own file
  descriptors, its own signal scope. Useful when the work might OOM, hang,
  or otherwise misbehave.
- **Real Claude TUI features.** Skills, plugins, MCP servers, slash
  commands, and the actual permission system are only available in a real
  `claude` session. Task sub-agents share the parent's tool surface, not
  the parent's TUI surface.

Use **Task** when:

- The work is one-shot research that fits in a single turn ("find all
  callers of X", "summarise this file", "draft a paragraph").
- No human-in-the-loop is needed.
- The setup cost of a separate session is not worth it.
- You want the result inline in your reasoning context.

Quick decision rule: if the user might want to *watch* or *talk to* the
worker, use pswarm. If the parent just wants an answer, use Task.

## Spawn pattern

The picoswarm Claude Code plugin (v0.3.0+) wires `SessionStart`, `Stop`,
`Notification`, and `SessionEnd` into the daemon, so a freshly spawned
agent reports `idle` / `attention` / `exit` events automatically. The
plugin is loaded as long as the picoswarm marketplace is installed (or the
parent loaded it via `claude --plugin-dir`).

Spawn the child:

```sh
pswarm run -d <name> -- claude
```

- `-d` detaches; the parent shell does not stay attached to the new PTY.
- The child receives `$PSWARM_AGENT_NAME=<name>` and `$PSWARM_AGENT_ID=<uuid>`
  in its environment automatically. Hooks and `pswarm inbox` use those
  variables to identify themselves.
- Use the bare `claude` command. **Do not** pass `--dangerously-skip-permissions`
  by default — let the user approve sensitive operations through the
  child's normal permission UI. (If the user has explicitly opted into
  auto-mode for the whole session, that's their call to make, not yours.)
- If the child needs a different cwd, `cd` first or pass it via a wrapper
  script; picoswarm does not manage worktrees.

## Wait for ready

Don't `pswarm send` immediately after `pswarm run` — Claude takes ~1s to
finish booting and the input would land on a TUI that isn't ready to read
it. The plugin's `SessionStart` hook fires `idle` once Claude is up, so
poll the registry for that:

```sh
until pswarm ls --json \
    | jq -e '.[] | select(.name=="<name>") | .last_event.event=="idle"' \
        >/dev/null 2>&1; do
    sleep 0.3
done
```

Inside Claude Code (the parent), wrap that in `Bash(run_in_background:
true)` so you get a single completion notification when the child is ready
instead of blocking the parent turn:

```
Bash(run_in_background=true,
     command="until pswarm ls --json | jq -e '.[] | select(.name==\"<name>\") | .last_event.event==\"idle\"' >/dev/null 2>&1; do sleep 0.3; done")
```

Do **not** use `Monitor` for this — `Monitor` is for streams of events,
not single readiness signals; the `until`-loop exits in seconds and emits
exactly one notification.

## Brief the child

Send the briefing as a single multi-line prompt:

```sh
pswarm send <name> "<full brief>"
```

`pswarm send` (v0.3.0+) handles bracketed-paste framing and the multi-line
submit problem automatically — long pastes are not collapsed to
`[Pasted text +N lines]` placeholders. Send the full brief in one call,
not line-by-line.

The child has no context from the parent except what fits in this prompt.
Be concrete:

- File paths it should read first (`CLAUDE.md`, relevant docs, the actual
  file under change).
- Section / function targets — point at line numbers when you know them.
- Conventions to follow (project style, language constraints, tests to
  run, "no emojis", etc.).
- Acceptance criteria — what counts as done.
- How to report back (write a file at `<path>`, or post to the parent's
  inbox, or both).

**Pre-plan the approach upstream.** Decide *what* should change before
sending. Briefs that ask the child to "propose 2-3 options" produce
output that is too low-precision to merge cleanly without a second round
of work; the parent ends up doing the planning anyway, only later. If the
parent is unsure, resolve that uncertainty *before* dispatching, not by
delegating it. (The opposite habit — single concrete brief, single concrete
output — has been the durable lesson from the multi-agent attempts so far.)

A useful brief is roughly: "Read X. Edit Y to do Z, following convention
W. Run V to validate. Write the diff to /tmp/foo.patch and post `done` to
my inbox."

## Coordinate via inbox

The parent and child talk over `pswarm inbox`, an append-only JSON Lines
file per agent at `$XDG_STATE_HOME/picoswarm/inbox/<name>.jsonl` with a
sidecar `<name>.cursor` tracking the reader's position.

### Arm the child as a receiver

The child's first action — before doing real work — should be to subscribe
to its own inbox via `Monitor`:

```
Monitor("inbox messages from parent",
        "pswarm inbox read --follow",
        persistent=true)
```

Each line written by the parent arrives in the child as a notification of
the form `{"ts":..., "from":"parent", "body":"..."}`. The child's
reasoning layer parses the JSON and reacts (or ignores). When
`$PSWARM_AGENT_NAME` is unset (i.e. the child happens to be running
*outside* pswarm), `pswarm inbox read` silently no-ops, so the same
Monitor command is safe to run unconditionally.

Note: this Monitor pattern is **Claude-Code-specific**. Other CLI agents
(Codex, Gemini, Cursor, Copilot, OpenCode, Aider) do not surface
background-process stdout as in-conversation events; they would have to
poll `pswarm inbox read` at turn boundaries. See
`docs/agent-tool-survey.md` for the full picture.

### Parent posts to the child

From outside pswarm (driver Claude session) — `--from` is required
because there's no `$PSWARM_AGENT_NAME` to derive from:

```sh
pswarm inbox post <child-name> "<message>" --from <label>
```

From inside another pswarm-spawned agent — `from` is auto-filled:

```sh
pswarm inbox post <child-name> "<message>"
```

### Child posts back to the parent

The child sends to the parent's inbox the same way. If the parent is a
driver Claude (no `$PSWARM_AGENT_NAME`), the parent's inbox is named
whatever the parent picks for itself — pass that name to the child as
part of the brief, e.g. "post status updates to `pswarm inbox post driver
... --from <yourname>`".

### Inbox vs `pswarm send`

Two channels, two purposes:

- **`pswarm inbox post`** — async, peer-style messages handled by the
  receiving agent's reasoning layer at its own pace. Use for status
  pings, "ready for review", coordination, anything the child is
  expected to *think about* alongside its current work.
- **`pswarm send`** — direct PTY input, types straight into the child's
  TUI as if a human typed it. Use for "stop what you're doing and follow
  this new prompt instead", or to answer a permission prompt out-of-band.
  This interrupts the child's turn.

Default to inbox. Use `send` when you mean to override the child.

### Caps and atomicity

Each encoded JSON Line is capped at 4000 bytes so a single `O_APPEND`
write stays atomic across concurrent senders. Messages over the cap are
rejected with an error pointing at "split or pass a file path" — for
large payloads, write a file and inbox-post the path, don't try to inline
the contents.

## Integrate the result

Both parent and child share the host filesystem, so the child writes
files directly — diffs, reports, generated code — and the parent reads
them after the child signals done over the inbox.

Typical end-of-task sequence:

1. Child finishes its work, writes output(s) to a path the brief
   specified.
2. Child posts `done` (or a structured payload) to the parent's inbox.
3. Parent (notified via its own Monitor on its inbox, or on the child's
   `last_event` flipping to `idle`) reads the output, validates it,
   and integrates.
4. Parent commits if the work warrants a commit. **Do not have the child
   commit** unless the user explicitly asked — review-then-commit is
   the parent's job.
5. Parent removes the child:

   ```sh
   pswarm rm <name>
   ```

   This deletes the agent process, its inbox JSONL, and its cursor file
   (clean slate against stale residue). For multi-step work, leave the
   child running and continue dispatching tasks instead.

## Common pitfalls

These have all bitten the project at least once. Check them before
spending time debugging.

- **Stale `pswarm` binary on `$PATH`.** A previous `cargo install` may
  have left `~/.cargo/bin/pswarm` shadowing a newer build symlinked into
  `~/bin`. Confirm:

  ```sh
  which pswarm
  md5sum "$(which pswarm)" target/release/pswarm
  ```

  If the hashes differ, the daemon and CLI are running different code.

- **Daemon protocol mismatch after a rebuild.** When the protocol version
  bumps, a higher-version `Hello` from a fresh client triggers daemon
  auto-shutdown. When it does *not* bump (same protocol, changed
  internals), the running daemon keeps the older internal code. If you
  observe odd behaviour right after a rebuild without a protocol bump,
  restart the daemon manually:

  ```sh
  pswarm daemon restart
  ```

- **Plugin version drift in a live Claude session.** A running Claude
  session keeps the plugin code it loaded at startup. Reinstalling the
  picoswarm plugin from the marketplace does not affect already-running
  sessions; restart the session (or spawn a new child) to pick up the
  new plugin version.

- **Multi-line paste collapse (legacy).** Older `pswarm send` versions
  could deliver a long multi-line message into the Claude TUI as a
  collapsed `[Pasted text +N lines]` placeholder, and the prompt would
  not auto-submit. v0.3.0 fixes this with bracketed-paste framing
  followed by two separate CR writes; if you observe a collapse, your
  binary or daemon is older than v0.3.0 — see the binary / daemon
  pitfalls above.

- **Forgetting `--from` from a driver session.** `pswarm inbox post`
  outside a pswarm-spawned agent has no `$PSWARM_AGENT_NAME` to derive
  from and will reject the post. Pass `--from <label>` explicitly.

- **Sending before ready.** Skipping the `last_event=idle` wait drops the
  briefing into the void. Always wait for `idle` after `pswarm run`
  before the first `pswarm send`.
