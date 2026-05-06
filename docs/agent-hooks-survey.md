# Agent-side hooks: survey of CLI coding agents

A survey of the lifecycle / hook mechanisms exposed by CLI coding agents other
than Claude Code. The focus is on whether each agent has an equivalent of
Claude Code's `Stop` hook — a callback that fires when the agent has finished
its turn and is ready for the next user input — because that primitive is what
picoswarm needs to mark an agent as "idle" without a screen scraper.

This is a snapshot from May 2026; hooks are a young area for most of these
projects and the surface is moving quickly.

## Comparison table

| Agent | Hooks system? | Stop / turn-complete equivalent | Config location | Notification mechanism |
|---|---|---|---|---|
| **Claude Code** (baseline) | Yes (21+ events) | `Stop`, `SubagentStop` | `~/.claude/settings.json`, `.claude/settings.json` | Command / HTTP / prompt / agent handlers; JSON via stdin → stdout |
| **OpenAI Codex CLI** | Yes (6 events, behind feature flag) | `Stop` hook **and** top-level `notify` setting (`agent-turn-complete`) | `~/.codex/hooks.json`, `~/.codex/config.toml` (`[hooks]`), repo-level `.codex/` | Hooks: command, JSON stdin/stdout, exit code 0/2. `notify`: external program, single JSON argv. |
| **Google Gemini CLI** | Yes (~12 events) | `AfterAgent` | `settings.json` (`hooks` object) | Command, JSON via stdin → stdout, exit code 0/2 |
| **Cursor CLI** | Yes (4 events on CLI; richer set in IDE) | `stop` | `<project>/.cursor/hooks.json`, `~/.cursor/hooks.json` | Spawned process, JSON over stdio |
| **GitHub Copilot CLI** | Yes (12 events) | `agentStop` / `Stop` (plus `sessionEnd`) | `.github/hooks/*.json` (repo) | Command, JSON over stdio |
| **OpenCode** (`sst/opencode`) | Yes — JS/TS plugins only, no shell hooks | `session.idle` | `opencode.json` + plugin module under `.opencode/plugins/` or `~/.config/opencode/plugins/` | In-process JS/TS callback on event bus |
| **Cline** (VS Code) | Yes (8 events) | `TaskComplete` | `~/Documents/Cline/Hooks/`, `.clinerules/hooks/` | Executable script (extensionless on \*nix, `.ps1` on Windows), JSON over stdio |
| **Aider** | No general hooks | `--notifications-command` (single global command) | CLI flag / `AIDER_NOTIFICATIONS_COMMAND` env | Bare shell command, no structured payload |

Common shape: most projects converged on Claude Code's design — a shell
command receiving a JSON event on stdin and optionally returning JSON on
stdout, with exit code 2 reserved for "block". Codex's separate `notify`
setting and Aider's `--notifications-command` are the two odd ones out, both
simpler than the hook protocol.

---

## Per-agent details

### OpenAI Codex CLI

Codex CLI has two distinct mechanisms, both relevant.

- **Hooks system** (feature-flagged with `[features] codex_hooks = true`).
  Loaded from `~/.codex/hooks.json` or inline `[hooks]` in `~/.codex/config.toml`,
  plus the same pair under `<repo>/.codex/`. Six matchers: `SessionStart`,
  `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`,
  `Stop`. The `Stop` hook fires when a turn completes; if any matching `Stop`
  hook returns `{"continue": false}`, that wins over other hooks' continuation
  decisions. Hooks are invoked as commands with JSON on stdin and JSON on
  stdout; exit code 2 blocks/denies, default timeout is 600s.
- **`notify` setting** — a top-level config key that points at an external
  program: `notify = ["python3", "/path/to/notify.py"]`. Codex spawns the
  program once per supported event (currently only `agent-turn-complete`) and
  passes a **single JSON argument** on argv (not stdin) with `type`,
  `thread-id`, `turn-id`, `cwd`, `input-messages`, `last-assistant-message`.
  This is essentially a built-in webhook for "agent finished its turn".
- A separate `tui.notifications` block controls in-TUI bell/OSC9 alerts and is
  not relevant to an external orchestrator.

For picoswarm, `notify` is the cleanest target: it doesn't require enabling
the hooks feature flag, and it's literally designed for "the agent just
finished a turn".

Sources:
- [Codex CLI hooks docs](https://developers.openai.com/codex/hooks)
- [Codex CLI advanced configuration (`notify`)](https://developers.openai.com/codex/config-advanced)
- [Codex CLI config reference](https://developers.openai.com/codex/config-reference)

### Google Gemini CLI

Gemini CLI ships a hook system explicitly modelled on Claude Code's. Hooks
are declared in `settings.json` under a `hooks` object keyed by event name.
Documented events include `BeforeTool` / `AfterTool`, `BeforeAgent` /
`AfterAgent`, `BeforeModel` / `AfterModel`, `BeforeToolSelection`,
`SessionStart` / `SessionEnd`, `Notification`, and `PreCompress`.

The Stop equivalent is **`AfterAgent`**, which fires "once per turn after the
model generates its final response". Hooks are spawned commands; input is
JSON on stdin, output must be valid JSON on stdout (the docs are explicit:
no plain text on stdout). Exit code 0 is success, 2 is a system-level block,
other non-zero codes are warnings.

Sources:
- [Gemini CLI hooks reference (GitHub)](https://github.com/google-gemini/gemini-cli/blob/main/docs/hooks/reference.md)
- [Gemini CLI hooks docs site](https://geminicli.com/docs/hooks/)
- [Google Developers Blog announcement](https://developers.googleblog.com/tailor-gemini-cli-to-your-workflow-with-hooks/)

### Cursor CLI

Cursor introduced hooks in v1.7 (October 2025) for the IDE agent, and added
hook support to the **Cursor CLI** in the January 2026 update — initially
with a smaller event set: `sessionStart`, `sessionEnd`, `prompt`, `stop`.

`stop` fires "when the agent loop ends" and uniquely supports
auto-submitting a follow-up via `followup_message` in the response (default
loop limit 5, configurable via `loop_limit`). The hook receives `status`
(`completed` | `aborted` | `error`) and `loop_count`.

Configuration lives in `<project>/.cursor/hooks.json` or
`~/.cursor/hooks.json`. Hooks are spawned processes communicating over
stdio in JSON, exit code 2 blocks. The IDE has many more events (e.g.
`beforeShellExecution`, `afterFileEdit`, `subagentStop`); the CLI subset
will likely grow but is currently narrower.

Sources:
- [Cursor hooks docs](https://cursor.com/docs/hooks)
- [Cursor CLI January 2026 changelog (announces CLI hooks)](https://cursor.com/changelog/cli-jan-16-2026)
- [InfoQ: Cursor 1.7 adds hooks for agent lifecycle control](https://www.infoq.com/news/2025/10/cursor-hooks/)

### GitHub Copilot CLI

Copilot CLI exposes 12 hook events:
`sessionStart`, `userPromptSubmitted`, `preToolUse`, `postToolUse`,
`postToolUseFailure`, `agentStop`, `subagentStart`, `subagentStop`,
`errorOccurred`, `preCompact`, `notification`, `sessionEnd`. There are two
plausible "ready for next input" events:

- **`agentStop` / `Stop`** — fires when the main agent finishes a turn;
  output is processed and can force continuation.
- **`sessionEnd`** — fires when the whole session terminates, with reason
  `complete` / `error` / `abort` / `timeout` / `user_exit`.

`agentStop` is the right one for "ready for next input"; `sessionEnd` is
process-exit, more like our daemon noticing the PTY closed.

Configuration is loaded from `.github/hooks/*.json` in the repo. Hooks are
external command invocations using JSON over stdio, supporting both
`command` (shell) and `prompt` (auto-submitted text, only at `sessionStart`)
handler types. Notification hooks are explicitly fire-and-forget — they
never block.

Note: this hook system is fairly new and there are open issues asking for
broader event coverage and global configuration ([#1157][copilot-1157],
[#971][copilot-971]).

[copilot-1157]: https://github.com/github/copilot-cli/issues/1157
[copilot-971]: https://github.com/github/copilot-cli/issues/971

Sources:
- [GitHub Copilot CLI hooks reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-hooks-reference)
- [Using hooks with Copilot CLI](https://docs.github.com/en/copilot/how-tos/copilot-cli/customize-copilot/use-hooks)
- [Hooks configuration](https://docs.github.com/en/copilot/reference/hooks-configuration)

### OpenCode (`sst/opencode`)

OpenCode is the outlier: there is **no shell-callable hook surface**.
Extensibility goes through JavaScript/TypeScript plugins that subscribe to
the internal event bus. Plugins are listed in `opencode.json` under a
`plugin` array (npm packages) and/or auto-loaded from
`.opencode/plugins/` and `~/.config/opencode/plugins/`.

The published event taxonomy includes `session.created`, `session.idle`,
`session.compacted`, `session.error`, `session.updated`, `message.updated`,
`tool.execute.before` / `tool.execute.after`, `permission.asked` /
`permission.replied`, `file.edited`, plus TUI-specific events.

The Stop equivalent is **`session.idle`**, which is what the official
notification example listens for. Because the handler runs in-process as
JS/TS, an external orchestrator like picoswarm would need a thin glue
plugin that calls out (HTTP, exec, write to a known socket/file) on
`session.idle`.

Sources:
- [OpenCode plugins docs](https://opencode.ai/docs/plugins/)
- [Hook lifecycle events overview](https://dev.to/einarcesar/does-opencode-support-hooks-a-complete-guide-to-extensibility-k3p)
- [OpenCode plugin development guide (community)](https://lushbinary.com/blog/opencode-plugin-development-custom-tools-hooks-guide/)

### Cline (VS Code extension)

Cline supports 8 hook types: `TaskStart`, `TaskResume`, `TaskCancel`,
`TaskComplete`, `PreToolUse`, `PostToolUse`, `UserPromptSubmit`,
`PreCompact`. Hooks live in `~/Documents/Cline/Hooks/` (global) and
`.clinerules/hooks/` (workspace, version-controllable); both run when both
exist, global first.

Hook handlers are platform-specific executables — extensionless scripts on
macOS/Linux, `.ps1` on Windows — invoked by the file name matching the
event. Input is JSON on stdin (`taskId`, `hookName`, `timestamp`,
`workspaceRoots`, plus event-specific fields); output is JSON on stdout
(`cancel`, `contextModification`, `errorMessage`).

The closest analog to `Stop` is **`TaskComplete`**, which fires when a task
finishes successfully. There is no separate "ready for next message" hook
for the multi-turn case — Cline's model is task-centric, not turn-centric,
which is a meaningful difference from Claude Code.

Caveat: Cline runs inside VS Code, not as a standalone CLI. Picoswarm
spawning Cline directly is not really in scope; this entry exists for
completeness.

Sources:
- [Cline hooks docs](https://docs.cline.bot/customization/hooks)

### Aider

Aider has no hook system. The only relevant primitives are CLI flags:

- `--notifications` / `--no-notifications` — terminal bell when LLM responses
  are ready (default off).
- `--notifications-command COMMAND` (env: `AIDER_NOTIFICATIONS_COMMAND`) —
  run an arbitrary shell command instead of the bell.

The command is invoked as a single string with no structured payload — no
JSON, no event type, no agent identity. To use it from picoswarm, the
spawning code would have to set an env var (e.g. `PSWARM_AGENT_ID`) before
launching Aider so that the wrapper script can identify which agent just
became ready.

There is also `--watch-files` for AI-comment-driven edits, but it is not a
lifecycle callback in any useful sense.

Sources:
- [Aider options reference](https://aider.chat/docs/config/options.html)

---

## Implications for picoswarm

1. **Claude Code's `Stop`-hook shape generalises.** Five of the seven
   non-Claude agents surveyed expose a "turn complete" event delivered as a
   spawned process with JSON over stdio (Codex hooks, Gemini, Cursor, Copilot,
   Cline). A picoswarm helper subcommand — say `pswarm signal-ready` — that
   reads a JSON event on stdin and reports an idle signal back to the daemon
   would slot into all of them with a one-line config snippet per agent.

2. **Identity must come from the environment, not the payload.** None of the
   agents include a "this is agent X in orchestrator Y" field. Picoswarm
   already plans to set `PSWARM_AGENT_NAME` (per the in-flight Stop-hook +
   readiness-signal design); that env var is what the wrapper script should
   key on, not anything in the JSON.

3. **Codex's `notify` is the easiest target of all.** It's a top-level config
   key, doesn't need a feature flag, fires exclusively on `agent-turn-complete`,
   and passes the event as a single JSON argv. Wire it before bothering with
   the full Codex hooks system.

4. **OpenCode requires a glue plugin, not a config snippet.** Because hooks
   are JS/TS in-process callbacks, picoswarm would need to ship a small npm
   package (or local plugin file) that subscribes to `session.idle` and calls
   out to the daemon. This is a different integration cost from the others
   and may be worth deferring until OpenCode users actually show up.

5. **Aider barely qualifies.** A single unstructured shell command is the
   whole surface. It's enough for a "the agent went idle" ping, but only
   because the env-var trick gives us identity.

6. **Cline is out of scope by construction.** It runs inside VS Code, not as
   a standalone CLI process picoswarm could spawn into a PTY. Skip.

7. **Naming.** Across agents the readiness event is variously called `Stop`,
   `AfterAgent`, `agentStop`, `session.idle`, `TaskComplete`,
   `agent-turn-complete`, plus Aider's nameless `--notifications-command`.
   Picoswarm's internal vocabulary should pick one — `idle` (matching
   OpenCode and reading well in `pswarm ls`) is a reasonable pick — and
   document the per-agent mapping in the integration guide.

8. **Don't build a uniform abstraction yet.** The shapes differ enough
   (JSON-on-stdin vs JSON-on-argv vs unstructured command vs in-process JS)
   that a single trait would be premature. A per-agent recipe in
   `docs/integration.md`, plus the `pswarm signal-ready` helper, is enough
   to unblock real users without committing to an abstraction we'd rewrite.
