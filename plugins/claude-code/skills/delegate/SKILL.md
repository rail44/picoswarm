---
name: delegate
description: Use when the user wants to delegate a unit of work to a fresh child Claude agent — spawning it under pswarm, briefing it, coordinating bidirectionally over the inbox, and integrating the result. Pick this over the built-in Task tool when the work needs human visibility or mid-run intervention, runs in a different cwd / model / version, requires OS-level isolation, needs Claude TUI features (skills, plugins, MCP), or has to outlive the current parent turn.
---

# picoswarm: delegate to a child Claude

The simplest 1-parent + 1-child shape on top of picoswarm. Foundation
for larger shapes (`fanout`, `pipeline`, …); on its own it is the right
tool when one sub-task benefits from a separate, visible, durable
session rather than an opaque sub-call.

## When to use pswarm vs the built-in Task tool

Task launches a sub-agent inside the parent's process. pswarm spawns a
real `claude` TUI in a daemon-owned PTY.

Use **pswarm** when at least one of these applies:

- Human visibility / mid-run intervention (`pswarm attach`, `pswarm send`).
- Bidirectional comm during the run (status pings, course corrections).
- Lifecycle outlives the parent turn.
- Different cwd / model / Claude version / env from the parent.
- OS-level isolation (own process, own fds, own signal scope).
- Needs real Claude TUI features — skills, plugins, MCP, slash commands.

Use **Task** when the work is one-shot research that fits in a single
turn, no human-in-the-loop is needed, and the result should land inline
in the parent's reasoning context.

Quick rule: if the user might want to *watch* or *talk to* the worker,
use pswarm. If the parent just wants an answer, use Task.

## Spawn pattern

The picoswarm Claude Code plugin (v0.3.0+) wires `SessionStart`, `Stop`,
`Notification`, and `SessionEnd` into the daemon, so a freshly spawned
agent reports `idle` / `attention` / `exit` events automatically.

```sh
pswarm run -d <name> -- claude
```

- `-d` detaches; the parent shell does not stay attached to the new PTY.
- The child receives `$PSWARM_AGENT_NAME=<name>` and
  `$PSWARM_AGENT_ID=<uuid>` automatically. Hooks and `pswarm inbox` use
  those to identify themselves.
- Use bare `claude`. Do not pass `--dangerously-skip-permissions` —
  let the user approve sensitive operations through the child's normal
  permission UI.
- For a different cwd, `cd` first; picoswarm does not manage worktrees.

## Wait for ready

Don't `pswarm send` immediately after `pswarm run` — Claude takes ~1s to
finish booting. The plugin's `SessionStart` hook fires `idle` once
Claude is up; poll for that:

```sh
until pswarm ls --json \
    | jq -e '.[] | select(.name=="<name>") | .last_event.event=="idle"' \
        >/dev/null 2>&1; do
    sleep 0.3
done
```

Inside Claude Code (the parent), wrap that in `Bash(run_in_background:
true)` so the parent turn is not blocked — it emits exactly one
completion notification and stops. (`Monitor` is for streams of
events, not single readiness signals; the inbox case below is what
`Monitor` is for.)

## Brief the child

Send the briefing as one multi-line prompt:

```sh
pswarm send <name> "<full brief>"
```

`pswarm send` (v0.3.0+) handles bracketed-paste framing and the
multi-line submit problem automatically.

The child has no context from the parent except this prompt. Be
concrete: file paths to read, section / function targets, conventions
to follow, acceptance criteria, how to report back.

**Pre-plan the approach upstream.** Decide *what* should change before
sending. Briefs that ask the child to "propose 2-3 options" produce
output too low-precision to merge cleanly; resolve uncertainty before
dispatching, not by delegating it.

**Have the child review the brief before working.** Even a carefully
written brief usually has a gap visible only to a reader who isn't the
author. Build a one-round review handshake into the brief itself: the
child reads the brief with fresh eyes, posts any clarifying questions
back to the parent's inbox, waits for answers, *then* starts. If
nothing is unclear, the child posts a brief "starting" ack and
proceeds. This catches ambiguity when fixing it is cheap (one inbox
round-trip) instead of after the child has produced something
mis-aimed.

A working brief shape, including the inbox handshake and review step:

```
First, use the Monitor tool with description: "inbox messages from
parent", command: "pswarm inbox read --follow", persistent: true.
Acknowledge briefly when armed.

Then read the rest of this brief carefully, as if you have no prior
context (you don't). If any acceptance criterion, file path, or
instruction is ambiguous, post your clarifying questions to the
parent and wait for the reply before starting:

  pswarm inbox post driver "Q: <questions>" --from <name>

If everything is clear, post a brief "starting" ack and proceed:

  pswarm inbox post driver "starting" --from <name>

Work: <read X, edit Y to do Z, run V to validate, write output to
<path>>.

When done, post via: pswarm inbox post driver "done at <path>"
--from <name>. If you receive a notification with body "ship it" or
"approved" you are done; otherwise treat the body as review feedback
and iterate.
```

## Coordinate via inbox

The parent and child talk over `pswarm inbox`, an append-only JSON
Lines file per agent at `$XDG_STATE_HOME/picoswarm/inbox/<name>.jsonl`
(plus a sidecar `<name>.cursor` for the reader's position).

`Monitor` is a Claude Code deferred tool: if either side does not have
its schema loaded, fetch it via `ToolSearch select:Monitor` before the
first call.

### Arm the child as a receiver

The child has no awareness of the inbox until *the parent's brief tells
it to use one*. There is no side-channel; the brief above does it as
its first instruction. Once the child runs that, each line the parent
posts arrives as a system-emitted notification of the form
`{"ts":<unix>,"from":"<label>","body":"<text>"}`. The child's reasoning
layer parses the JSON and reacts. (When `$PSWARM_AGENT_NAME` is unset
— i.e. outside pswarm — `pswarm inbox read` silently no-ops, so the
brief's first instruction is safe to run unconditionally.)

### Receive on the parent side

The parent picks an arbitrary label for itself — `driver`, `parent`,
or task-specific like `feature-x-orchestrator` — and arms its own
`Monitor`:

```sh
PSWARM_AGENT_NAME=driver pswarm inbox read --follow
```

(description: `inbox replies from <child>`, persistent: true.)

The env-override is needed because the parent has no
`$PSWARM_AGENT_NAME`. Posting to a non-existent label simply creates
the inbox file. Pass the chosen label to the child in the brief.

Pick a unique label per concurrent driver — two parents both choosing
`driver` would share one inbox.

### Posting

From outside pswarm, `--from` is required:

```sh
pswarm inbox post <child-name> "<message>" --from driver
```

From inside another pswarm-spawned agent, `from` auto-fills:

```sh
pswarm inbox post <child-name> "<message>"
```

The child posts back to the parent's label the same way:

```sh
pswarm inbox post driver "draft ready at <path>" --from <child-name>
```

### Inbox vs `pswarm send`

- **inbox** — async, peer-style; the receiving agent reads at its own
  pace via Monitor. Default channel.
- **`pswarm send`** — direct PTY input as if the human typed; interrupts
  the child's turn. Use to override the child or answer a permission
  prompt out of band.

Encoded JSON Lines are capped at 4000 bytes (atomicity boundary) —
oversize messages are rejected; for large payloads, write a file and
post the path.

## Integrate the result

Both parent and child share the host filesystem; the child writes
files directly and the parent reads them after the child signals done
over the inbox.

End-of-task:

1. Child writes output to the path the brief specified.
2. Child posts `done` (or a structured payload) to the parent's inbox.
3. Parent (notified by its own Monitor) reads, validates, integrates.
4. Parent commits if warranted. **Do not have the child commit** unless
   the user explicitly asked — review-then-commit is the parent's job.
5. `pswarm rm <name>` deletes the agent + its inbox JSONL + cursor
   file. For multi-step work, leave the child running and continue.

## Scaling to N children

Dispatching N children is N `delegate` calls — the single-child rules
carry directly. The N-specific deltas:

- **Unique names**: task-specific labels (`feature-impl`,
  `bug-triage`) or numeric suffixes (`worker-1` … `worker-N`).
  Collisions surface as `NameTaken` at spawn. Use `pswarm register`
  for the parent's own identity to avoid driver-vs-driver inbox
  collisions.
- **Per-child briefs > one broadcast**. Each child gets a scoped
  sub-task with its file regions / modules called out explicitly,
  so concurrent edits don't conflict. This is delegate's "pre-plan
  upstream" rule scaled: divide before dispatching, not after.
- **Synchronisation**: wait for N `done` posts (a counter on the
  parent's Monitor side) or for each child's `last_event` to flip.
  Decide up front whether one child's failure aborts the rest or
  the others continue.
- **Resource ceiling**: ~4 concurrent children is the practical
  limit for a human watching kitty splits, plus API throughput
  costs scale linearly.

A future `picoswarm:fanout` skill is not planned — at this scope the
guidance fits in one section.

## Common pitfalls

- **Stale `pswarm` binary on `$PATH`.** A previous `cargo install` may
  shadow a newer build symlinked into `~/bin`. Confirm:

  ```sh
  which pswarm
  md5sum "$(which pswarm)" target/release/pswarm
  ```

  Different hashes mean the CLI and the build are out of sync.

- **Daemon protocol mismatch after a rebuild.** A protocol-version bump
  triggers daemon auto-shutdown via the higher-version Hello; an
  internals-only rebuild does not. If something looks off right after
  rebuild, `pswarm daemon restart`.

- **Plugin version drift in a live Claude session.** A running Claude
  keeps the plugin code it loaded at startup. Reinstalling the plugin
  does not affect already-running sessions; restart the session (or
  spawn a new child) to pick up the new version.

- **`pswarm` not on the child's `$PATH`.** The child inherits the
  daemon's `$PATH`. If the daemon was started from a shell without
  `pswarm` resolvable, the child's inbox posts will fail silently.
  Verify with `pswarm send <child> 'which pswarm'` and
  `pswarm view <child>`.

- **`attention` racing `idle` on the readiness loop.** If the child
  fires `attention` first (a permission prompt at startup), the
  `last_event=="idle"` loop never exits. Either widen to
  `select(.event=="idle" or .event=="attention")`, wrap with
  `timeout 30 sh -c '...'`, or run the child in auto-mode so the
  prompt doesn't fire. Default plugin-loaded children fire `idle`
  first in practice.
