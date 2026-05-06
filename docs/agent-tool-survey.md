# Agent-side notification streams: survey of CLI coding agents

A survey of whether CLI coding agents other than Claude Code expose a way to
**stream external events into the running conversation** the way Claude Code's
`Monitor` tool does. Picoswarm is considering using `Monitor("tail -F <inbox>")`
as a receive box for inter-agent messages — sender writes a line to a file,
receiver Claude sees each line arrive as a notification while still in the same
session. Before committing to that pattern, we want to know whether the same
shape generalises.

For reference, the Claude Code primitive being matched is roughly:

> Subscribe to a long-running shell process. Each line of stdout becomes a
> notification message in the agent's conversation context, *separate from
> user-typed input*, while the agent keeps running. The agent can keep
> responding in the same turn or queue the lines for the next turn.

This is a snapshot from May 2026.

## Comparison table

| Agent | Monitor-equivalent? | Closest mechanism | Push or poll? | Notes |
|---|---|---|---|---|
| **Claude Code** (baseline) | Yes | `Monitor` tool subscribed to a background `Bash` shell | Push (per-line notification) | Lines arrive as conversation events, not as tool-call returns. |
| **OpenAI Codex CLI** | No | `shell` tool returns aggregated stdout/stderr on completion; `codex exec resume -` pipes a fresh prompt | Neither (one-shot) | MCP integration is request-response only; no push channel surfaced to the agent. |
| **Google Gemini CLI** | No | `run_shell_command` with `&` returns a PID immediately, but the agent does not get streamed output — only the user's `/shells` dashboard does. Interactive shell streams snapshots to the *user*, not into the LLM context. | Neither for the agent | The infrastructure (node-pty + 1s snapshot loop) exists; it just isn't wired to the conversation. |
| **Cursor CLI** | No | Shell tool with output display; `--output-format stream-json` is one-directional output for an external controller. | n/a | No documented way to push events back into a running session. |
| **GitHub Copilot CLI** | No (closest of the bunch) | MCP tools with `taskSupport: "required"` register as **background agents** discoverable via `list_agents` / `read_agent`. | Poll | Experimental flag (`/experimental on` or `--experimental`). Issue [#2682][copilot-2682] tracks user-visible live output for long-running shell. |
| **OpenCode** (`sst/opencode`) | No | Community plugin [`opencode-background`][oc-bg] exposes `createBackgroundProcess` / `listBackgroundProcesses` (last 100 lines stored). Plugin event bus is subscribe-only — no API for a plugin to inject new conversation messages. | Poll | A custom plugin could in principle write to a session, but the documented API doesn't expose it. |
| **Aider** | No | `--watch-files`: a background `watchfiles` thread scans the repo for `AI!` / `AI?` code comments and triggers a fresh chat turn when one appears. | Push, but only into a *new* turn | Triggers on AI-marker comments only; not a free-form line stream into the current turn. Requires interactive mode. |

[copilot-2682]: https://github.com/github/copilot-cli/issues/2682
[oc-bg]: https://github.com/zenobi-us/opencode-background

The headline result: **Claude Code's `Monitor` tool has no direct counterpart
in any of the other agents surveyed**. The MCP spec defines server→client
notifications (JSON-RPC notifications, optionally over SSE), and several agents
have *some* form of background-process surface, but in every case the agent
only sees new output if it explicitly issues another tool call. The "free
notification arrives mid-turn" shape is, today, a Claude Code idiosyncrasy.
The closest non-Claude approximation is Aider's `--watch-files`, which is push
but only fires on AI-comment markers and starts a new turn rather than
appending to the current one.

---

## Per-agent details

### OpenAI Codex CLI

Built-in tool surface: `shell`, file edit, web fetch, web search, image input,
image generation. None of these stream — the `shell` tool returns aggregated
stdout/stderr plus an exit code when the command finishes. There is no `tail`,
`watch`, `monitor`, or `subscribe` tool.

MCP servers can be configured in `~/.codex/config.toml` and Codex starts them
at session boot, exposing their tools alongside the built-ins. Calls into MCP
servers are request-response from the agent's point of view; even if a server
emits JSON-RPC notifications per the MCP spec, the Codex CLI client does not
surface them as conversation events. Streaming HTTP MCP servers exist
([Streaming HTTP transport][mcp-streaming]) but Codex's exposure of those
streams to the LLM is not documented.

[mcp-streaming]: https://dev.to/varungujarathi9/mcp-streaming-http-deep-dive-1n5e

`codex exec` (non-interactive) supports `-` as a `PROMPT` argument to read a
single prompt from stdin, and `codex exec resume [SESSION_ID]` continues a
prior session with one follow-up prompt. Both are one-shot: there is no mode
where Codex stays attached to an open stdin and treats each new line as a new
event/message into a live session.

Sources:
- [Codex CLI features](https://developers.openai.com/codex/cli/features)
- [Codex CLI command reference](https://developers.openai.com/codex/cli/reference)
- [Codex MCP integration](https://developers.openai.com/codex/mcp)

### Google Gemini CLI

`run_shell_command` is the only relevant built-in. Documented behaviour:

- Synchronous run: returns `Stdout`, `Stderr`, `Exit Code`, plus a `Background
  PIDs` field listing any child processes the command backgrounded with `&`.
  Output is aggregated, not streamed to the agent.
- Background commands (`some-cmd &`): tool returns immediately. The PID is
  surfaced; subsequent stdout is **not** pushed to the LLM. The user can run
  `/shells` to view a dashboard of live output, but that's a TUI affordance
  for the human, not a conversation event for Gemini.
- Interactive shell mode (`enableInteractiveShell: true`, announced in the
  "new level of interactivity" blog): Gemini CLI spawns an inner pty via
  `node-pty` and snapshots terminal state at ~1000 ms intervals so the
  *user* sees real-time output. The blog is silent on whether snapshots feed
  the model context, and the shell tool reference describes the agent input
  as the same `Stdout`/`Stderr`/`Exit Code` JSON. Net: the streaming pipeline
  exists, but it terminates at the UI, not at the LLM.

No `watch`, `tail`, `monitor`, `subscribe`, or push-style tool is documented.

Sources:
- [Gemini CLI shell tool reference](https://geminicli.com/docs/tools/shell/)
- [Gemini CLI shell command tutorial](https://geminicli.com/docs/cli/tutorials/shell-commands/)
- [Google Developers Blog: new level of interactivity](https://developers.googleblog.com/say-hello-to-a-new-level-of-interactivity-in-gemini-cli/)
- [DeepWiki: Shell mode and command execution](https://deepwiki.com/google-gemini/gemini-cli/3.5-shell-command-execution)

### Cursor CLI

Cursor CLI inherits the IDE agent's tool set: shell, file edits, MCP. MCP
servers configured via `mcp.json` work in the CLI in the same shape they do
in the IDE — request/response tool calls, no surfaced server notifications.

The headless mode (`cursor-agent --output-format stream-json`) emits JSON
events *outward* so an external controller can observe progress; it does
**not** offer a reverse channel to inject events into a running session.
Documentation does not describe any tool for tailing files, watching streams,
or subscribing to events.

A current real-world wrinkle worth noting: an open bug report says
`mcpToolCall` events stopped firing in `cursor-agent 2026.04.17` even though
servers are listed as ready. So even regular MCP tool calls are flaky on the
CLI right now ([forum thread][cursor-mcp-bug]); pushing notifications through
MCP is not a near-term option here.

[cursor-mcp-bug]: https://forum.cursor.com/t/cursor-agent-cli-mcp-tool-calls-silently-stopped-working-in-2026-04-17/158988

Sources:
- [Cursor CLI overview](https://cursor.com/cli)
- [Cursor CLI January 2026 changelog](https://cursor.com/changelog/cli-jan-16-2026)

### GitHub Copilot CLI

Copilot CLI is the only agent in this survey that ships a first-party concept
of long-running background agents. With experimental mode on (`/experimental
on` or `--experimental`), an MCP tool can declare `taskSupport: "required"`
and Copilot will run it as a non-blocking background task. The agent then has
two related tools to inspect those tasks:

- `list_agents` — enumerate background tasks
- `read_agent` — read accumulated output of a specific task

This is **poll, not push**: Copilot only sees new output if the model decides
to call `read_agent` again. There is no notification-on-new-line mechanism.
Issue [#2682][copilot-2682] requests live streaming output for long shell
commands but is open and is framed as a UI feature rather than a conversation
feature.

Hooks (12 events) are unrelated to mid-turn streaming — they fire at lifecycle
points, not on output of a running command.

Sources:
- [Using GitHub Copilot CLI](https://docs.github.com/copilot/how-tos/use-copilot-agents/use-copilot-cli)
- [Copilot CLI background tasks issue #2682][copilot-2682]
- [GitHub Copilot CLI release notes](https://github.com/github/copilot-cli/releases)

### OpenCode (`sst/opencode`)

OpenCode's plugin system is the most expressive of the bunch but does not
publish an API for injecting new messages into a live session. The plugin
context exposes `project`, `directory`, `worktree`, `client`, and `$` (shell);
plugins can subscribe to a wide event taxonomy (`session.idle`,
`message.updated`, `tool.execute.before/after`, etc.) and define custom
tools. There is no documented `session.send` / `session.append` /
`session.input` API.

The community-maintained `opencode-background` plugin gives the agent three
methods — `createBackgroundProcess`, `listBackgroundProcesses`, `killProcesses`
— with the last 100 lines of a process's output retained in memory. The agent
must call `listBackgroundProcesses` to see new output. Strictly poll-based.

A determined integrator could probably write a plugin that captures the
internal `client` and pushes synthetic messages, but this is undocumented and
brittle territory; we should not rely on it.

Sources:
- [OpenCode plugins docs](https://opencode.ai/docs/plugins/)
- [`zenobi-us/opencode-background` README][oc-bg]
- [OpenCode tool system](https://deepwiki.com/sst/opencode/5-tools-and-permissions)

### Aider

Aider has no MCP integration and no general tool sandbox; instead, two
file-system features are adjacent to "external events into the conversation":

- `--watch-files`: a background `watchfiles` thread scans the repo for
  one-line code comments containing `AI!` or `AI?`. When one appears, Aider
  collects the comment plus surrounding code as context and **starts a new
  chat turn**. This is genuinely push-driven and survives an idle session.
  Caveat: it only fires on AI-marker comments in source files — it can't be
  pointed at an arbitrary inbox file, and each detection starts a fresh turn
  rather than appending to the current one.
- `--message-file MESSAGE_FILE`: load a single message from a file and exit
  after the reply. One-shot.

There is no FIFO mode, no streaming-stdin mode, and no concept of a
notification message distinct from a user prompt. Once `--watch-files`
fires, the AI! comment effectively *is* the user prompt for the next turn.

Sources:
- [Aider in your IDE / watch mode](https://aider.chat/docs/usage/watch.html)
- [Aider options reference](https://aider.chat/docs/config/options.html)
- [Aider release history](https://aider.chat/HISTORY.html)

---

## Implications for picoswarm

1. **The inbox-via-Monitor pattern does not generalise.** It works in Claude
   Code because `Monitor` injects each line of background-process stdout as a
   notification in the agent's conversation. None of Codex CLI, Gemini CLI,
   Cursor CLI, Copilot CLI, OpenCode, or Aider expose an equivalent. If we
   commit to this pattern as the cross-agent inbox, we are committing to it
   as a Claude-Code-only feature.

2. **MCP push notifications are spec-supported but not client-supported
   today.** Server→client JSON-RPC notifications and SSE streaming are part of
   MCP, but Claude Code itself doesn't yet surface them to the agent
   ([anthropics/claude-code#36665][cc-36665]), and the other clients are even
   further behind. Designing the inbox on top of "MCP notifications" would
   require waiting on every target client to add the surface.

   [cc-36665]: https://github.com/anthropics/claude-code/issues/36665

3. **Push-vs-poll is the real axis, not "tool name".** The agents that have a
   background-process concept (Copilot CLI's `read_agent`, OpenCode's
   `listBackgroundProcesses`) make the agent poll. Polling burns context
   tokens and only catches messages on the next tool call. This is a notable
   downgrade from the Claude Code experience and worth flagging in
   integration docs.

4. **The closest non-Claude analog is Aider's `--watch-files`, but it is
   narrow.** It's reactive and doesn't require human input, but it only
   triggers on `AI!` / `AI?` comments in source files. Repurposing it as a
   picoswarm inbox would mean the sender writes an `AI!` comment into a
   sentinel file, which is a hack and would also rope every Aider invocation
   into the watch mechanic.

5. **Two reasonable picoswarm responses, depending on how cross-agent we want
   to be:**

   - **Lean in to Claude as the inbox-rich agent.** Document Monitor-as-inbox
     as a Claude-Code-specific affordance; for other agents, accept that
     inter-agent messaging is delivered on the *next* user-prompt boundary
     rather than mid-turn. This keeps the design simple and lets Claude
     sessions feel natively orchestrable.
   - **Build a polling shim on the receiving side.** For non-Claude agents,
     have `pswarm send` write to the inbox file *and* nudge the recipient via
     whatever lifecycle surface that agent does expose (Codex `notify`,
     Gemini hook, Copilot hook, OpenCode `session.idle` plugin). The agent
     reads the inbox at turn boundaries. Worse latency, but works
     uniformly.

   These are not mutually exclusive: the Claude path can be the
   pretty-by-default experience, and the polling path is the fallback for
   everyone else.

6. **Don't build an MCP-notification-based inbox yet.** It would be the most
   elegant cross-agent design, but it depends on multiple downstream clients
   shipping a feature none of them have. Revisit when at least two of
   {Claude Code, Codex CLI, Cursor CLI, Copilot CLI} support surfacing MCP
   server notifications to the agent.
