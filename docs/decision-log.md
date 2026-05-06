# Decision Log

Long-form rationale behind picoswarm's design choices. CLAUDE.md and `plan.md` hold the conclusions; this file holds the *why* — context, alternatives weighed, and the reasoning at the time of the decision.

Entries are roughly chronological. The numbering corresponds to topics in the project's decision-log topic list and is stable across appends.

---

## 3. Existing tools landscape

### Context

Before committing to build picoswarm, we surveyed the agent-orchestration space for tools that already cover the use case. The goal was either to piggyback on an existing tool, or — failing that — to sharpen what picoswarm uniquely contributes.

### Tools evaluated

**Tmux-based (tmux is a hard dependency)**:

- **Agent of Empires (njbrake/agent-of-empires)** — Rust + tmux + git worktree, TUI/CLI, broad agent support (Claude Code, OpenCode, Codex CLI, Gemini CLI, Cursor CLI, Copilot CLI, Mistral Vibe, Pi.dev, Factory Droid). v1.0+, Homebrew/Nix/cargo distribution, active. Most feature-complete option in the space, but tmux is mandatory.
- **Bosun (yetidevworks/bosun)** — Rust + ratatui + tmux control mode (`tmux -C`) + actor pattern. Cleanly designed; dedicated tmux socket (`tmux -L bosun`). Tmux-locked.
- **ccswarm (nwiizo/ccswarm)** — Rust, uses the ai-session crate as its PTY layer. Multi-agent orchestration with an autonomous flavor.
- **agent-deck (asheshgoplani/agent-deck)** — Go + Bubble Tea + tmux. Multi-agent TUI aggregator across Claude/Gemini/OpenCode/Codex.
- **Composio agent-orchestrator (ComposioHQ/agent-orchestrator)** — Go/TypeScript, tmux as default runtime, web dashboard, autonomous fleet model. 6.4k stars.
- **Claude Squad** — Tmux-based session manager specialised for Claude Code.
- **Tutti (nutthouse/tutti)** — Workflow-oriented, web dashboard, tmux-based.
- **Batty (battyterm/batty)** — Hierarchical agent layout (Architect/Manager/Engineer), Maildir messaging, tmux-fixed.
- **ATM (Agent TMux Manager)** — Real-time monitoring for tmux-hosted Claude Code agents.

**Self-contained (no external session manager)**:

- **ccmanager (kbwo/ccmanager)** — TypeScript, no tmux, multi-agent (Claude / Gemini / Codex / Cursor / Copilot / Cline / OpenCode / Kimi). 1.1k stars, 140+ releases. The closest functional match. See item 4 for the result of testing it.
- **loom (cosmix/loom)** — Rust, own daemon, autonomous fire-and-forget orchestration. Different UX model: humans define plans up front, agents work unattended, no interactive attach concept. State recovery via `loom resume <stage-id>`, not interactive reattach.
- **claudectl (mercurialsolo/claudectl)** — Rust + ratatui, observes already-running Claude Code sessions via `~/.claude/sessions`, JSONL transcripts, and `ps`. Doesn't spawn or own sessions. Could complement picoswarm rather than compete.
- **claude-control (sverrirsig/claude-control)** — TypeScript + Electron + Next.js, macOS-only, observation-only.

**AI-driving-terminal (different use case)**:

- **pilotty (msmps/pilotty)** — Rust + portable-pty + vt100. Daemon-managed PTY for AI agents to drive other terminal applications (vim, htop, lazygit). Architecturally adjacent (same crates we use), but the use case is "AI is the user," not "human orchestrating multiple AI agents."
- **npcterm** — Rust, MCP server exposing terminal access to LLMs as structured tools.
- **agent-tui** — Rust, daemon-based TUI session manager for AI agents.

**Library / inspiration**:

- **retach** — Rust terminal multiplexer with native scrollback passthrough, daemon-client architecture, ~15k LoC. Library API only exposes the VT screen module; the daemon is private. Useful as a reference for "how big does a Rust mini-tmux end up."
- **tsm (adibhanna/tsm)** — Go, daemon-per-session + PTY-per-session, libghostty-vt for screen restoration, native splits via kitty/Ghostty/WezTerm. Architecturally the closest precedent to what picoswarm aims to be — but in Go.
- **muxtree** — Single bash script. Extreme-minimalism reference point.
- **sesh (joshmedeski/sesh)** — Go, tmux session registry. Relevant as a registry-CLI reference.
- **overstory (jayminwest/overstory)** — TypeScript, 11 runtime adapters, SQLite mail system. "PTY as escape hatch" design philosophy.

**Built-in to Claude Code**:

- **Anthropic Agent Teams** — Claude Code v2.1.32+ (2026-02), experimental (`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`). One Claude session acts as team lead; teammates work in their own context windows and can talk to each other. See item 10 for the implication on picoswarm's positioning.

### Decision

No exact match for picoswarm's intended position — Rust + own PTY daemon + agent registry + CLI-first + kitty-friendly. Continue building.

### Reasoning

- Tmux-based tools were ruled out by the user's discomfort with tmux as a baseline dependency (and by the user's broader desire for a Rust-native, dependency-light tool).
- ccmanager covered the right functional surface but in TypeScript and TUI-first (item 4).
- loom and Composio agent-orchestrator target an autonomous/fire-and-forget UX, not interactive attach.
- claudectl is read-only and operates on sessions that exist outside it; it would be a nice complement, not a substitute.
- pilotty and npcterm are oriented toward "AI drives terminal," not "human orchestrates AI."
- tsm is the closest architectural precedent, but is in Go.

The niche — Rust + own daemon + agent-agnostic + CLI-first + kitty-friendly — was empty.

---

## 4. ccmanager evaluation

### Context

ccmanager (kbwo/ccmanager) was the closest functional match found in the landscape survey: TypeScript, self-contained (no tmux), supports the same broad set of agents picoswarm cares about (Claude / Gemini / Codex / Cursor / Copilot / Cline / OpenCode / Kimi), well maintained (1.1k stars, 140+ releases at the time of evaluation). Before committing to build picoswarm, we agreed the user should install ccmanager and try it on the actual workflow.

### Result

The user's verdict: ccmanager is a TUI dashboard. To do anything you launch the dashboard, navigate menus, and press keys. The user prefers composable CLI commands they can type directly, pipe, alias, and (eventually) have agents invoke on themselves. The TUI-first UX did not fit.

### Decision

Don't adopt ccmanager. Continue building picoswarm. Promote **CLI-first** to a hard differentiator and record it explicitly in CLAUDE.md "Decisions that must not drift":

> The primary UX is CLI subcommands that compose with the shell. Do not make an interactive TUI the primary entry point. A TUI may be added later as a complementary view, but `pswarm` must remain useful as one-shot commands (this is the main differentiator from existing TUI-driven managers like ccmanager).

### Reasoning

The CLI-first preference is not just stylistic; it has a concrete consequence baked into picoswarm's stated thesis: **agents themselves can invoke `pswarm send`, `pswarm ls`, etc. as plain shell commands** through their tool layer. A TUI-first tool either can't expose this surface or exposes it as a second-class, separate compatibility CLI. picoswarm makes the CLI the primary interface and any TUI is a complementary view on top.

This was the moment picoswarm's value proposition sharpened from a soft "Rust + kitty native" to a much harder "**CLI-first** + Rust + own daemon." The latter is harder for an existing tool to argue with, because converting a TUI-first product to be CLI-first is a substantial rewrite.

### Reflection

Trying the closest competitor in production usage is the cheapest, highest-signal validation step in this kind of project. Half a day of installing and using ccmanager prevented building the wrong thing for weeks.

---

## 5. Library landscape

### Context

With the thesis revising toward an own PTY daemon (see item 8), picoswarm needed a concrete library stack: PTY operations, optional VT parsing, async I/O, daemonization, IPC encoding, terminal raw mode, XDG path resolution, and miscellaneous utilities. Each had several candidates of varying maturity.

### PTY operations

- **portable-pty** (wezterm) — Cross-platform PTY abstraction, mature, well documented. Synchronous I/O. **Adopted.**
- **pty-process** — Smaller alternative with both blocking and async APIs. Less feature-complete than portable-pty. Rejected.
- **pseudoterminal** — Newer crate with async support and Windows ConPTY. Considered but portable-pty was sufficient.

A throwaway "spike" was used to confirm portable-pty works correctly under daemonised conditions (no controlling terminal): `openpty` + `spawn_command` + `resize` + read/write + `try_wait` + `kill` all behaved as expected when the test process self-detached via `setsid`. No hidden requirement for additional `setsid` or `ioctl` work in our daemon.

### VT parser (deferred)

- **vte** (alacritty) — Mature, no Zig requirement, used by retach. The natural pick when VT state restoration is needed.
- **libghostty-vt** — Pre-1.0, requires Zig 0.15.x at build time, no released versions of the Rust binding (uzaaft/libghostty-rs has 80+ commits but 0 release tags). Powerful (SIMD-optimised parsing, Ghostty-quality Unicode handling) but the build complexity was disqualifying for MVP. **Deferred — revisit when libghostty-vt tags a stable release.**
- **libghostty vs libghostty-vt distinction** — `libghostty` is the full terminal (rendering, windowing); `libghostty-vt` is just the VT parser. Easy to confuse; the user flagged this explicitly. We would only ever consider libghostty-vt.

VT parsing is not needed for MVP. PTY output is forwarded as raw bytes to the attached client, plus a 64 KiB ring buffer for reattach backlog. Screen-state restoration is a deliberate post-MVP feature.

### Session manager as library

- **libshpool** — Published, embeddable as a library: `Args` constructible programmatically, `Hooks` trait for callbacks, `run()` entry point. Originally designed so Google could ship an internal version with telemetry. Inherits shpool's two limitations: no concurrent attach, and no external `send` mechanism. Both block picoswarm's intended capabilities (multi-client observation, programmatic input from agents). **Rejected.**
- **ai-session** crate (ccswarm) — Marketed as "AI-optimised terminal session management," includes `SessionManager`, `MultiAgentSession`, `MessageBus`. Looks great on paper, but ccswarm's own README states the message bus integration is "not yet wired" and the crate is tightly coupled to ccswarm's internal architecture. **Rejected** as too unstable to depend on.
- **retach (as a library)** — Only the `screen` module (VT emulator) is publicly exposed; the daemon, session management, and protocol code are private to the binary. Cannot be embedded as-is.

### Encoding

- **bincode** — Was the obvious default until we discovered it is **unmaintained** (RUSTSEC-2025-0141: the maintainers ceased development after a doxxing/harassment incident; v3.0.0 ships a deliberate `compile_error!` pointing to xkcd/2347). v1.3.3 is "complete" per the team. **Rejected** — almost shipped on it without checking.
- **postcard** (jamesmunns/postcard) — Stable v1, serde-compatible, varint-encoded so frame sizes are typically smaller than bincode's, embedded community well-known maintainer. Wire format documented and stable since v1.0.0. **Adopted.**
- **bincode-next** — Active fork, but at v3.0.0-rc.13 with uncertain organisational continuity (Apich-Organization). Skipped in favour of the more established postcard.
- **rkyv** (zero-copy) — More complex API; overkill for our request/response and short-streaming-chunk use case.
- **rmp-serde** (MessagePack), **ciborium** (CBOR) — Heavier wire formats; no compelling reason over postcard.

### Daemonization

- **daemonize** crate (knsd/daemonize) — v0.5.0, last release Feb 2023. Daemonisation is a well-defined task (fork → setsid → fork → chdir → close stdio → reopen); a 3-year-old release for a solved problem is not a red flag. **Adopted.**
- **daemonize-me** — Active fork. Considered but the original was sufficient.
- **daemonizr** — Adds PID-file locking and daemon-search functionality; we don't need either yet.
- **Self-implementation** (~30 lines via `nix`) — Considered but has subtle gotchas (signal handling order, pidfile races, fd ordering across forks). Adopted the crate to avoid them.

### Path resolution

- **directories** — XDG-style paths, cross-platform fallbacks. The GitHub repo (`soc/directories-rs`) was archived in 2025-02 and development moved to Codeberg (`codeberg.org/dirs/directories-rs`); the Codeberg repo is actively maintained as of 2026-03. **Adopted.**
- Manual `$XDG_*` env reads — Considered but `directories` keeps the door open for non-Linux targets cheaply.

### Async runtime

- **tokio** — Standard for async I/O in Rust. **Adopted** with feature set `rt-multi-thread + macros + io-util + net + sync + signal + time`. The `process` feature was dropped when it became clear we use portable-pty (synchronous, run in `spawn_blocking`) for process spawning.

### Terminal raw mode (added with attach)

- **crossterm** — Heavier (~50 KB) but supports raw mode + `SIGWINCH` watching + cross-platform terminal size. **Adopted.**
- **nix** + manual `termios` — Lighter but more code; would have re-implemented what crossterm already gives us.
- **termion** — Older, Unix-only; no advantage over crossterm for our needs.

### Persistence

- **rusqlite** (bundled SQLite) — Was the original plan (the old CLAUDE.md referenced an SQLite registry). Removed during the persistence-design discussion: the daemon's children die with the daemon, so reviving registry rows would describe nothing real. **Rejected — registry is now in-memory only, see item 12.**
- **sled**, **redb**, **heed** — Embedded KV alternatives. Not needed once persistence itself was dropped.

### Decision

Final dependency set:
```
portable-pty + tokio + postcard + daemonize + directories + crossterm
+ anyhow / thiserror / tracing / tracing-subscriber / clap / serde / serde_json / uuid
```

Notable absences: no SQLite (registry is in-memory), no VT parser (raw bytes forwarded as-is for MVP).

### Reasoning

Each piece of the stack is a mature, mainstream crate; the runtime cost is bounded; the build does not require Zig or other unusual toolchains; and the wire format is stable.

### Reflection

Two decisions in this set were forced moves discovered late:

- bincode going dark (RUSTSEC-2025-0141) was uncovered by the user asking "any deprecated deps?" — almost shipped on it.
- libghostty-vt's Zig build requirement made it disqualifying for MVP despite being technically appealing. Lesson: shiny new libraries with unusual build requirements are a future commitment, not a free upgrade.

The lesson generalised: do an explicit dep audit even when "everything looks fine." A two-minute `cargo info` per crate would have caught both ahead of time.

---

## 7. The "persistence + no external deps + thin code" trilemma

### Context

While debating which backend to build on, three properties kept appearing in the discussion as desirable:

- **Real PTY persistence** — sessions survive across CLI invocations, terminal restarts, etc.
- **No external runtime dependencies** — installing picoswarm should not require also installing tmux / shpool / abduco.
- **Thin implementation** — the codebase stays small enough that one person can hold it in their head.

It became clear that no single design satisfies all three. Each pair forces a third trade.

### The trade-off

| Pick | Lose | Concrete shape |
|---|---|---|
| Persistence + no deps | Thin | Build our own daemon (~1000–1500 LoC) using portable-pty + tokio. We hold the PTYs ourselves, no external session manager. |
| Persistence + thin | No deps | Glue around an existing session manager (tmux backend in ~300 LoC, abduco backend in ~400 LoC). Persistence comes for free, but the user has to install it. |
| No deps + thin | Persistence | Treat a kitty tab itself as the "session." Closing the tab kills the agent. Trivial code, no install, but no real persistence. |

### Decision

Pick **persistence + no external deps**. Accept a moderate amount of code (own daemon) as the cost.

### Reasoning

Going through the user's hard requirements one at a time:

- "Real persistence" was non-negotiable: the user explicitly rejected the kitty-tab-as-session option as "throwaway code."
- "No external deps" matters in two ways the user cares about — distribution friction (`brew install ...` is a barrier) and code ownership ("the project is glue on someone else's work" feels off).
- "Thin code" is desirable but bendable. A daemon in the 1000–1500 LoC range is still well within personal-tool size.

Once persistence is mandatory, only the daemon path remains.

The `~1000–1500 LoC` estimate was anchored to two reference points: tsm's `internal/session/daemon.go` is 1242 LoC for substantially more functionality than picoswarm needs (libghostty-vt screen restoration, scrollback passthrough, etc.); and we explicitly skip VT parsing in MVP, which removes the largest chunk of complexity.

### Reflection

Naming the trilemma was the most useful single move in the architecture discussion. Until then we kept circling because each candidate "almost worked." Once each option was framed as "it gives you exactly two of the three," it was obvious which trade-off matched the user's priorities.

The trilemma also reframes the value of "thin": the win is not minimal LoC but **bounded LoC**. Owning ~1500 lines you understand is fine. Owning ~5000 is not. The number that matters is the upper bound, not the absolute minimum.

---

## 8. Thesis revision: from external backends to own daemon

### Context

The original CLAUDE.md (preserved as `docs/design-discussion.md`) framed picoswarm as a *pure layer that delegates persistence and screen splitting to external tools*, with PTY persistence behind a `Backend` trait. The first-class backends were tmux / shpool / none. This was the operating thesis going into the conversation.

### The revision path

The thesis didn't survive contact with the user's real preferences. Each candidate backend failed for a specific reason:

1. **kitty as the backend** — Use a kitty tab as the session unit; spawn = `kitty @ launch`, attach = `kitty @ focus-tab`, send = `kitty @ send-text`. Zero install (the user already runs kitty). **Rejected** by the user: closing a tab destroys the session, which means the work can't be made into "lasting code."

2. **shpool** — Rust-native, philosophically close to picoswarm (PTY persistence and nothing else). After verifying its CLI:
   - shpool only allows one client attached per session at a time.
   - shpool has no external `send` mechanism.
   These limits turn into picoswarm's limits, and they break two things picoswarm specifically needs: programmatic agent observation (an agent can't `pswarm attach` another while a human is attached) and `Backend::send` itself (literally not implementable). **Rejected.**

3. **tmux** — Fully featured (send-keys, multi-client attach, capture-pane). Could be made nearly invisible to the user via a dedicated socket and a stripped-down config. **Rejected** because the user's discomfort with tmux is a starting motivation, not a side issue. Hiding tmux's UI doesn't change that tmux is in the install footprint and the dep tree.

4. **abduco** — Tiny (~1k LoC C), explicitly minimal, no TUI surface to hide. Closer to picoswarm's spirit. **Rejected** when we worked through what picoswarm would have to add on top: `send` via fifo wrapper, log capture via tee, snapshot via byte buffer, signal handling, multi-client read-only — at which point picoswarm has reimplemented mini-tmux on top of abduco, contradicting the "delegate to external tools" thesis.

5. **shpool as a library (libshpool)** — Embeddable in our binary, eliminates the runtime dep. **Rejected** because we'd inherit shpool's limits (no concurrent attach, no external send) at the API layer; the symptoms move from "external tool we drive" to "library we depend on" but stay the same.

6. **portable-pty + libghostty-vt + own daemon** — Build the daemon ourselves. **Adopted.** Sized using tsm as a reference (item 5).

### Decision

picoswarm owns its session daemon. The Backend trait is removed (item 9). The thesis is updated:

> picoswarm is a self-contained agent orchestrator. It owns a small PTY daemon to keep agent sessions alive, plus an agent registry and lifecycle CLI on top.

### Reasoning

The original thesis correctly identified what picoswarm should *not* contain (no screen splitting, no DAG, no scheduling), but mis-identified the dividing line between "core" and "delegated." Persistence isn't naturally a delegated concern: it leaks back into core through the abstractions over `send`, `attach`, observability, and concurrent clients. Each external option either failed to provide what picoswarm needs (shpool, abduco) or was off the table for non-technical reasons (tmux).

Owning the daemon turns "what can each backend do?" into "what should picoswarm do?" — a single, controllable design space.

### Reflection

The original thesis was internally consistent but assumed a richer set of backend candidates than actually existed for our use case. Two of the three named backends (shpool, none) couldn't deliver the operations picoswarm needs without picoswarm rebuilding them on top — which is the same as building the daemon outright. The third (tmux) was unacceptable for human reasons.

The lesson: when designing around an abstraction, list what each concrete implementation would have to do for a representative set of operations. If you can't fill out that table, the abstraction is decorative.

---

## 9. Dropping the Backend abstraction

### Context

For most of the design conversation I framed backend choice as a problem-solving lever: "if shpool can't do X, switch to tmux." This reading kept showing up in my proposals — "the limit is fine for MVP, future versions can swap backends to relax it" — and it kept generating bad recommendations.

### The user's correction

The user pushed back explicitly:

> backend 切替の価値は発生しません。あくまで、このpicoswarmの行う仕事の範囲外のツールは、ユーザーは何を使ってもいいよという意味のバックエンド切替機能です。

Reframed: **the Backend abstraction's purpose was to respect the user's existing tooling choice, not to solve problems by switching.** If a user is a tmux user, picoswarm should integrate with tmux. If they're a shpool user, picoswarm should integrate with shpool. picoswarm doesn't switch between them to work around limitations — whichever the user picked is the one picoswarm lives with.

This inverts a lot of the earlier reasoning. "shpool can't do concurrent attach" is no longer "we'll fix it later by switching" but "shpool users won't have concurrent attach in picoswarm. Period."

### Decision

Once we committed to the own-daemon thesis (item 8), the Backend abstraction had **one** implementation. With one impl, the trait is decoration — and worse, decoration that invites the bad reading ("we can swap"). The trait was removed from CLAUDE.md and the code.

The PaneHost (adapter) abstraction remains, because it serves a different purpose: it adapts to the user's terminal/multiplexer (kitty, wezterm, ...) for window placement. That choice genuinely is per-user and per-environment, and there will be multiple implementations.

### Reasoning

Two related forces pointed in the same direction:

1. The corrected understanding of backend abstraction's purpose. Once it's about "respect the user's tool," and the user has rejected every external session manager, the abstraction has nothing to abstract over.
2. YAGNI applied honestly. We don't have a second backend in flight. We're not even sure we want one. Building the trait now risks designing it wrong (the previous trait shape, e.g. `send`, was designed without knowing how it would be implemented for real).

If a real second backend appears later (the most plausible candidate is ACP once Claude Code's support matures), reintroducing a trait at that point is straightforward: the daemon's existing internals already form one implementation, and a second can be added behind `dyn Backend` with no more friction than it would have today.

### Reflection

I projected the wrong purpose onto the abstraction (problem-solving via swap) and let that purpose guide design. The abstraction looked sensible from the inside — `Backend::send`, `Backend::attach` — but those signatures depend on what backends can actually do, and for shpool and `none` the answer was "less than we need." The trait would have papered over that incompatibility.

The general principle: an abstraction that everyone can implement at the signature level but only some can implement at the *behavioural* level isn't an abstraction; it's a leaky tag.

---

## 10. Anthropic Agent Teams: built-in multi-agent in Claude Code

### Context

During the landscape survey we discovered that Claude Code itself ships built-in multi-agent orchestration as of v2.1.32 (2026-02): set `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1` and one Claude session can act as a team lead, coordinating teammates that work in their own context windows and can talk to each other directly.

This is *not* the same as Claude Code subagents (which run inside a single session); teammates are independent, peer Claude sessions.

### Implication

Built-in multi-agent in the target product is the kind of thing that, in the wrong year, would obsolete a tool like picoswarm. So it deserves a hard look.

It doesn't, for three reasons:

1. **Experimental, off by default.** Most users won't have it on. Even those who do are using the feature alongside, not instead of, multi-window workflows.
2. **Claude-only.** Agent Teams is a Claude Code feature. picoswarm orchestrates *whatever you spawn in a PTY* — Claude Code, Codex CLI, Gemini CLI, your own scripts. The orchestration layer is agent-agnostic.
3. **Different layer.** Agent Teams is task-internal coordination ("this team is solving one problem together, with a lead and assistants"). picoswarm is operator-driven concurrency ("I have five things going at once, let me look in on each"). Both can coexist; one is "I'm Claude organising my own sub-Claudes," the other is "I'm a human running multiple things at once."

### Decision

Continue building picoswarm. Sharpen the positioning to make the distinction explicit:

- **picoswarm is agent-agnostic and human-driven.** Multiple independent agents under operator control.
- **Agent Teams is Claude-specific and agent-driven.** One task, multiple cooperating Claudes.

These do not compete; they layer.

### Reasoning

The risk we took on in deciding to build picoswarm despite Agent Teams was small. The agent-agnostic + human-driven niche is structurally outside what a vendor-specific feature can occupy: Anthropic isn't going to ship "Anthropic Agent Teams that also drives Codex CLI," because Codex isn't theirs. The only way picoswarm gets eaten by built-in features is if every CLI agent gains Agent-Teams-style coordination *and* a meta-tool emerges to coordinate them. Both of those are years away if they happen at all.

### Reflection

The ground moves. Two months before this conversation, "Claude Code with built-in multi-agent" wouldn't have been a category. It was worth asking whether picoswarm's premise still held, and explicitly recording why it does. If a future development changes that answer (for example: an OS-level standard for agent coordination, or a Claude Code adapter that directly speaks to Codex / Gemini / Cursor), this entry is the place to revisit.

---

## 11. PTY library swap: portable-pty → pty-process

### Context

Issue #21 (orphan agent prevention) wanted to install `PR_SET_PDEATHSIG` on each spawned child so the kernel SIGTERMs them if the daemon dies hard. The natural place to set it is a `pre_exec` closure between fork and exec.

Investigation found that **portable-pty 0.9.0's `CommandBuilder` does not expose `pre_exec`** — its unix backend already calls `Command::pre_exec` once for setsid/TIOCSCTTY setup, and `std::process::Command::pre_exec` is last-write-wins. Adding a hook would require either vendoring portable-pty or sending an upstream PR (no existing wezterm issue/PR for this — checked).

### Tools considered

- **Vendor portable-pty's unix backend**: ~150 LoC fork, ongoing maintenance burden.
- **Upstream PR to wezterm**: half-day to write, indeterminate merge timeline.
- **PID file + startup orphan reaper**: 100-150 LoC, doesn't prevent orphans, only cleans them up at next daemon start.
- **`pswarm reap-orphans` subcommand using `/proc/*/environ`**: 50-70 LoC, manual or auto-triggered.
- **Switch to `pty-process`** (doy/Jesse Luehrs): exposes `Command::pre_exec` natively, properly chains with internal session_leader setup, ~1-2 hour migration.

### Decision

Switch to `pty-process`. Add `pre_exec` that calls `nix::sys::prctl::set_pdeathsig(SIGTERM)`.

### Reasoning

- The original `docs/decision-log.md` 5 entry rejected pty-process as "less feature-complete than portable-pty" without specifying which features. Re-examining under the current requirement (need `pre_exec`), the assessment flips: pty-process has the feature we want and portable-pty doesn't.
- "Less feature-complete" was implicitly about Windows ConPTY support — pty-process is Linux/macOS-focused. picoswarm targets POSIX systems (WSL is sufficient for Windows users), so the feature gap is irrelevant.
- pty-process is actively maintained: 3.46M total downloads, 330K in the last 90 days, last release 2025-07 (0.5.3, edition 2024). Maintainer (doy) is well-known in the Rust ecosystem; cargo and uv reference its API surface.
- The `unstable-` style risk is lower than clap_complete's: pty-process exposes a stable `pre_exec` (no feature flag), and the API has been frozen since 0.5.0 (Jan 2025).
- Migration cost was bounded (~150 LoC across `session.rs`, `registry.rs`, `server.rs`, plus `tests/common/pty.rs`), and the resulting `AgentEntry` is structurally simpler — one `Arc<Pty>` replacing three `Arc<Mutex<Box<dyn Trait>>>` fields.

### Reflection

Two lessons:

- **A library-choice decision-log entry should record what was rejected and why.** The original entry 5 said "less feature-complete" without enumerating which features mattered. When requirements shifted (we suddenly needed `pre_exec`), there was no way to tell from the log whether the original rejection still held. Future entries should list the specific axes of comparison.
- **Multi-threaded `PR_SET_PDEATHSIG` is not the footgun the manpage warns about, in our case.** The man page warns the signal fires when the parent *thread* dies, not the parent process — which would be catastrophic with tokio's blocking thread pool. In practice, tokio's worker threads are pooled and stay alive for the runtime's lifetime, so the signal only fires when the daemon process truly exits. Verified via live test: `kill -9 <daemon>` → child SIGTERM'd within ~1s.

---

## 12. Strict clippy lint configuration and its review triggers

### Context

picoswarm's `Cargo.toml` `[lints]` section enables `clippy::pedantic`,
`clippy::nursery`, and a hand-picked set of `clippy::restriction` lints
(`unwrap_used`, `expect_used`, `panic`, `let_underscore_must_use`,
`unwrap_in_result`, `dbg_macro`, `todo`, `unimplemented`,
`redundant_clone`). Several individually-noisy pedantic/nursery lints
are explicitly `allow`ed at the workspace level. Test files allow the
test-only-noisy ones (`unwrap_used` / `expect_used` / `panic`) at the
file level, and only specific Drop impls / post-action drains carry
`#[allow(clippy::let_underscore_must_use)]`.

The configuration is deliberately at the strict end of what's
maintainable for a solo CLI. `just check` runs `cargo clippy
--all-targets -- -D warnings`, so any warning breaks the build.

### Decision

Keep the strict configuration as the default and accept the friction
it imposes on prototyping. Each `allow` entry is a known trade-off —
this section records what would justify changing the trade-off.

### Review triggers

Revisit specific lints when one of these happens:

- **`cast_possible_truncation` / `cast_sign_loss` / `cast_precision_loss`** (currently `allow`): re-enable if any new module starts doing numeric processing beyond PIDs, terminal sizes, or simple seconds-to-i64 conversions. These lints catch real overflow bugs; we silenced them only because every `as i32` for PID conversion was firing.
- **`significant_drop_tightening`** (currently `allow`): re-enable if a real lock-contention symptom surfaces. The most likely trigger is issue #06 (multi-client read-only attach) or a future feature that holds the registry lock during I/O.
- **`must_use_candidate`** (currently `allow`): re-enable, or add `#[must_use]` on individual fns, if picoswarm starts exposing a stable library API that external consumers depend on.
- **`shadow_unrelated` / `shadow_reuse`** (currently not enabled): enable if a shadowing-related bug ever lands. Skipped on day one because the idiomatic `let mut foo = foo.lock()` pattern would fire on every Mutex use.
- **`pedantic` group as a whole**: a clippy toolchain bump may introduce a new pedantic lint that fires on existing code. When this happens, evaluate the new lint individually — most pedantic additions over the last 18 months have been justified, but a few are bikeshed-bait.
- **`unwrap_used` / `expect_used` / `panic` in app code**: relax (move to `allow` or scope to specific modules) if the friction during prototyping starts outweighing the regressions caught. The current pattern is per-site `#[allow]` with a comment explaining why; if those allows accumulate above ~15 in app code, the lint has stopped paying for itself and should be relaxed.
- **Test-side `let_underscore_must_use`** (currently allowed only on Drop impls and post-action drains): if test refactors push that count up significantly, revisit whether the lint is still catching real test-logic regressions.

### Reasoning

The rules above are explicit so future revisits don't have to re-derive
the original cost/benefit. Each trigger names a concrete observable
event ("contention symptom", "external consumer", "friction count
above N") rather than "if it feels wrong" — that's the form least
prone to drift.

The strict-by-default choice is justified by one shaped-by-experience
observation: the discussion that motivated this configuration started
because a `let _ = ...` had been silently swallowing real errors for
weeks. Catching the next instance of that class of bug is worth the
ergonomic friction.

### Reflection

Most of the cost was front-loaded into one cleanup session (220 → 0
warnings in two commits). The ongoing cost is small per change but
non-zero: every new `.unwrap()` or `let _ =` in app code triggers a
build failure that needs a deliberate fix or `#[allow]`. The hope is
that this small recurring tax pays for itself by surfacing exactly
the pattern that prompted this decision.

---

## 13. Drop the PaneHost adapter abstraction

### Context

The original design (preserved in `docs/design-discussion.md`, summarised in older revisions of `CLAUDE.md`) named **adapter (PaneHost)** as a first-class component: a Rust trait that picoswarm core would call to open / focus / close terminal panes for attached clients, with **kitty** as the first-class implementation. Issue #01 was filed to build it.

When that issue came up for triage we re-examined the assumption.

### Reconsidering

Two observations made the abstraction look weaker than it had at design time:

1. **Modern terminals' own CLIs are richer than any picoswarm trait would expose.** `kitty @` covers `launch`, `focus-tab`, `close-window`, `set-tab-title`, `send-text --match`, `ls`, etc. Wrapping a subset (`open_pane(name)`) in a Rust trait would always be a lossy compromise; the user would still reach for `kitty @` directly for anything beyond the wrapped subset.
2. **The same job is a 5-line shell function.** `function pswt; pswarm run -d $argv; kitty @ launch --type=tab pswarm attach $argv[1]; end` does what `pswarm run --tab` would have done, with full access to kitty's flags. Same for tmux, wezterm, etc. Each user gets exactly the integration they want without the picoswarm authors having to anticipate every variation.

The shape of the argument matched item 9 (Backend abstraction drop): an abstraction that everyone can implement at the signature level but only some can implement at the *behavioural* level isn't an abstraction; it's a leaky tag.

### Hook alternatives we considered before settling

If picoswarm needed to invoke external commands (e.g., when an agent itself wants to launch another agent into a pane it doesn't own), four mechanisms were on the table:

- **A. Documentation only** (no code mechanism): users compose with their shell. Adopted for now.
- **B. `PSWARM_PANE_HOOK` env var**: rejected. Env propagates into spawned agents, and our daemon already broadcasts client env to agents (resolves issue #11). A recursive `pswarm` invocation from inside an agent would inherit the same hook, run it, and potentially loop or run unintended commands.
- **C. Config-file hook (`config.toml [hooks] pane = "/path/to/script"`)**: deferred. When an agent-launches-agent-into-pane scenario actually shows up, this is the path forward. The hook is a script path (not a shell command string), invoked as `Command::new(path).arg(name)` without shell interpretation, configured in a file the user can audit.
- **D. Plugin / RPC system**: overkill, rejected.

### Decision

picoswarm core does not embed any terminal- or multiplexer-specific code. Window/pane management is the user's responsibility, composed via their terminal's CLI. The `PaneHost` trait, the `adapter/` module, and the kitty-specific code that issue #01 contemplated are all out of scope. `docs/integration.md` captures the recipes and the future-hook contract.

If a hook becomes necessary, it lands as **option C** (config-file path to an executable script), never **option B** (env variable). This is recorded both here and in `CLAUDE.md` "Decisions that must not drift".

### Reasoning

- **Composition over abstraction.** picoswarm primitives plus a user's shell suffice for every integration we've imagined. Each new abstraction is a permanent maintenance commitment with diminishing returns.
- **Bounded scope.** Once picoswarm absorbs window management, every related ask ("auto-focus on attach", "close pane on exit", "preview agent output in a status bar") becomes an in-tree feature. Keeping it out of scope keeps the project small.
- **Security under the env-inherit model.** Our agents inherit the client's env (resolves #11). An env-based hook would silently propagate into every agent and every subshell, with the same value — that's a privilege-escalation surface, not a feature.

### Reflection

Both backend (item 9) and adapter (this item) were rejected for the same shape of reason: an abstraction designed before its concrete needs were known turned out to span only one realistic implementation that worked, and the abstraction itself added cost without buying flexibility. **Lesson: abstractions should be introduced when the second implementation lands, not the first.** Naming the abstraction in advance creates a phantom commitment to fill it in.

The future-hook contract (config-file path to a script, no shell interpretation) is recorded now even though no hook exists — so when the question comes up again, the constraint that env-based hooks are unsafe under our env-inherit model is already on the page.

---

## 14. Parked: structured daemon logging vs. a separate lifecycle log

### Context

Issue #05 originally proposed a separate JSONL lifecycle log
(`spawned` / `attached` / `detached` / `exited` / `removed` events
with structured payloads). Triage rejected it: the strongest
motivating use case (recovering past agents on daemon crash) is
explicitly out of scope per item 8, and the remaining uses (history
queries, audit, replay) are mostly already covered by `daemon.log`
+ shell history. #05 was deleted rather than deferred.

### The parked question

If at some future point picoswarm *does* want lifecycle events
queryable programmatically, two paths are open:

- **Reinstate a separate JSONL file** with event-shaped writes from
  the daemon. Adds an mpsc + writer task; one extra file to
  rotate/manage.
- **Switch the daemon-mode tracing subscriber from text to JSON**
  (`tracing_subscriber::fmt().json()`), keeping foreground stderr
  output as text. ~10 lines in `daemon/lifecycle.rs::init_tracing`,
  no new file. Tradeoff: `just log` becomes less pleasant for
  human-tailing (`jq -C` mitigates, but it's a step down for live
  development).

The hybrid (foreground=text, background=JSON) is the cheaper of
the two and reuses existing tracing field usage. The separate-file
approach is more invasive but gives full control over the schema.

### Decision

Not deciding now. This entry exists so that whoever revisits the
question doesn't have to re-derive the comparison from scratch. The
trigger to revisit is the same as #05's original triggers: a real
consumer wanting structured event data, or a `pswarm history`
subcommand becoming desirable.

---

## 15. Dedicated spawner thread for `PR_SET_PDEATHSIG` (corrects entry 11)

### Context

Item 11 introduced `PR_SET_PDEATHSIG(SIGTERM)` via `pre_exec` so that
the kernel kills agents when the daemon dies hard. The reflection
section claimed:

> Multi-threaded `PR_SET_PDEATHSIG` is not the footgun the manpage
> warns about, in our case. […] In practice, tokio's worker threads
> are pooled and stay alive for the runtime's lifetime, so the
> signal only fires when the daemon process truly exits.

That claim was wrong. The bug surfaced in real use: every agent
died ~10 s after spawn.

### What actually happens

`session::spawn_session` was being called via
`tokio::task::spawn_blocking(...)`. The blocking task runs on a
worker from tokio's **blocking pool**, which is distinct from the
runtime's worker pool. Blocking-pool workers have a default
`thread_keep_alive` of 10 s — once a worker has been idle for
10 s, tokio reaps it. When the worker thread terminates, the
kernel fires `PR_SET_PDEATHSIG` for every child it forked, sending
each agent SIGTERM.

The runtime's *own* worker pool does stay alive for the runtime's
lifetime. The blocking pool's workers do not. Item 11 conflated the
two.

### The fix

Introduce a dedicated long-lived OS thread (`daemon::spawner`)
that owns all forks. The spawner thread receives spawn requests
over a `std::sync::mpsc` channel and runs `session::spawn_session`
synchronously on its own stack. Because the thread is started in
`lifecycle::start` and lives until the daemon process exits, it
becomes the kernel-level "parent thread" for every agent's
`PR_SET_PDEATHSIG`, and the signal fires only when the daemon
really goes away.

The spawner needs the tokio runtime so that
`session::spawn_session`'s call into `output_session::spawn`
(which uses `tokio::spawn`) can find a reactor. We pass the
`Handle` into `Spawner::new` and `handle.enter()` it inside the
spawner thread's loop.

### Reasoning

Three options were weighed before picking the dedicated thread:

- **Drop `PR_SET_PDEATHSIG`**: simplest, but loses item 11's gain
  (orphan-on-hard-kill prevention) and would require building an
  alternative reaper. Not worth backing out a working mechanism.
- **`thread_keep_alive(very-long)` on the runtime**: a one-line
  knob change that keeps blocking workers alive indefinitely.
  Effective in practice but couples our orphan-prevention to a
  performance-tuning configuration; future readers wouldn't
  recognise the load-bearing role of the value, and changing it
  later for unrelated reasons would silently re-introduce the bug.
- **A `tokio::spawn`'d task on a tokio worker**: would also keep
  the parent thread alive for the runtime's lifetime, but relies
  on undocumented worker-lifetime semantics and on the task not
  migrating between workers between forks. Less explicit than a
  dedicated OS thread.

The codebase already uses `std::thread` directly for the per-agent
PTY-reader thread (`session::drain_into_session`), so adding one
more long-lived OS thread fits the existing pattern rather than
introducing a new concept.

### Reflection

The original entry 11 reflection said tokio worker threads "are
pooled and stay alive for the runtime's lifetime." That's true for
the runtime's main worker pool but **not** for the blocking pool,
which has an independent reaping policy. When relying on
thread-bound kernel state, the right model isn't "trust tokio to
keep threads alive" — it's "own a thread whose lifetime I can
state precisely in one sentence."

The bug was masked in tests (every test finishes well under 10 s)
and in our earlier smoke verification (we killed the daemon ~2 s
after spawn). A test that explicitly waits longer than the
keep-alive timeout would have caught it. Adding such a test is
costly (the suite would gain ~12 s of wall time) and the failure
mode is now well-understood; we rely on the dedicated-thread fix
plus this entry to keep us honest.

---

## 16. Wrap the agent's TUI in a PTY, don't depend on its SDK or protocol

### Context

The coding-agent space currently has at least three programmatic
integration surfaces below the actual user-facing terminal app:

- vendor SDKs (Claude Code SDK, etc.)
- open agent protocols (ACP — Agent Client Protocol)
- vendor APIs called directly (Anthropic Messages API, OpenAI
  Responses API, etc.)

An orchestrator could plausibly target any of these instead of
running the vendor's own TUI binary. picoswarm doesn't.

### Decision

picoswarm runs the official TUI binary the user already has
installed (`claude`, `codex`, `gemini`, …) inside a PTY held by its
daemon. The orchestrator never imports an SDK, never speaks a
vendor's API, never speaks ACP.

### Reasoning

- **Vendor velocity is in the TUI.** New tools, modes, status
  displays, and ergonomics ship to the agent's user-facing TUI first.
  SDKs and protocol abstractions lag behind, sometimes permanently —
  parity isn't even guaranteed direction by the vendor. Running the
  TUI binary means picoswarm inherits every improvement the moment
  the user upgrades their agent.
- **Agent-agnosticism falls out for free.** A PTY-wrapped TUI is just
  bytes in, bytes out. There's no SDK type, no per-vendor protocol
  shape, no per-vendor branch in our code. Any PTY-spawnable
  interactive agent works the same way. (See `CLAUDE.md` Glossary's
  deliberately broad definition of "agent.")
- **What we lose is consistent with item 4.** This stance gives up on
  running agents headlessly via API-only pipelines, on programmatic
  fine-grained tool intercept, and on transports that don't carry a
  TTY. Item 4 already locks in that the orchestrator's primary UX is
  interactive (CLI subcommands composed with shell + agents attached
  to PTYs), so the loss is consistent with the project's scope.

### Reflection

The choice was implicit until this entry: every earlier decision
(`portable-pty`/`pty-process`, postcard wire format, daemon ownership
of PTYs) assumed PTY-wrapping without ever explaining why we didn't
go SDK-first. Recording it now gives a future "wouldn't it be cleaner
to use the Claude SDK?" instinct something to push back on.

The trade reveals a property we hadn't named explicitly: picoswarm's
"agent-agnostic" stance isn't a feature we built — it's a side-effect
of the integration surface we picked. Surface choice shapes feature
scope as much as any explicit decision does.

## 17. Agent readiness via the agent's own hooks; Claude Code gets a real plugin, others get config recipes

### What we decided

To detect "agent ready for next input" without scraping the TUI,
picoswarm exposes a thin receiver — `pswarm event <event>` — that
reads the agent's name from `$PSWARM_AGENT_NAME` (injected by the
daemon at spawn) and asks the agent's own lifecycle hooks to call
it. For Claude Code we ship the wiring as a real plugin bundle
(`plugins/claude-code/`, installable via `claude --plugin-dir`).
For other agents, the equivalent is a one-line config snippet in
`docs/integration.md`, not a bundled artifact.

(Earlier revisions of this entry described `pswarm event <name> <event>`
plus a `self` keyword that triggered env resolution and silent
no-op semantics. That `self` keyword was removed in plugin v0.3.0
once we settled on the principle "external CLI face = self-only,
debug = env override" — see the inbox / `self` discussion below.)

### Why hooks rather than output scraping

Several heuristics for "is this agent ready" were considered: output
quiescence (silence for N ms), TUI pattern matching (look for the
input box prompt), VT cursor position, BEL/OSC9 inspection. All of
them are fragile against TUI version drift, mode toggles, and
language settings. The agents we care about already expose a
turn-complete callback (`docs/agent-hooks-survey.md` catalogues
them). Calling the agent's own primitive is both more accurate and
agent-version-stable.

### Why a closed enum for the event vocabulary

The first sketch made the event a free string ("daemon stays
agent-agnostic"). On reflection that flexibility was illusory: the
producers (hook configs) and the consumer (`pswarm ls` formatting)
are both code we write, so a free-string surface buys nothing and
loses compile-time exhaustiveness. The protocol now carries
`enum Event { Idle, Attention, Exit }`. New variants ride a protocol
bump — a low-cost operation we already do (`v3 → v8` so far).

### Why an asymmetric integration story

Claude Code has a rich plugin format (manifest, hooks file, scripts
under `${CLAUDE_PLUGIN_ROOT}`, `claude --plugin-dir` for local
loading). Building against it means a user can install our wiring
with one command and get all three signals (`Stop`, `Notification`,
`SessionEnd`) without hand-editing settings. The other surveyed
agents do not converge on a comparable bundle format — most accept
hooks via plain `settings.json` snippets, OpenCode uses npm packages,
Aider has only `--notifications-command`. Trying to ship a uniform
"plugin" abstraction across all of them would force the lowest common
denominator on each one. Instead, the receiver (`pswarm event`) is
agent-agnostic, and per-agent recipes live in `docs/integration.md`.

### Why no wrapper shell script in the plugin

The first cut shipped a `scripts/event.sh` that handled
`PSWARM_AGENT_NAME` absence, suppressed errors, and called `pswarm
event self <event>`. Once that logic was identified, it was clearly
all already inside our binary's natural responsibilities — agents
already inherit the daemon's `PATH`, so `pswarm` is reliably callable
from a hook. The plugin's `hooks.json` now invokes `pswarm event
<event>` directly, and the subcommand silently exits 0 when
`PSWARM_AGENT_NAME` is unset (or the daemon is unreachable, or
returns NotFound). Hook callers get the right behaviour without a
shell-script intermediary, and the plugin is reduced to a manifest
plus a `hooks.json`.

### Closure of issue #02 (agent self-invocation, env injection)

This work needed `$PSWARM_AGENT_NAME` available inside the spawned
agent so the hook script could resolve the `self` keyword. That
requirement is exactly the trigger noted in
`issues/02-agent-self-invocation.md`'s "Triggers to revisit" section.
We took Approach A (env injection only): `PSWARM_AGENT_NAME` and
`PSWARM_AGENT_ID` are set at spawn, and the agent name `self` is
reserved so `pswarm event self idle` etc. compose cleanly. The
send-to-self loop guard remains deferred to a later issue.

### Reflection

The plan opened with "what's the most general primitive we can build"
and almost shipped a uniform abstraction over disparate agent hook
shapes. Walking back to "ship the Claude plugin now, write recipes
for the others" was the right call: less code, no premature
abstraction, and the per-agent recipes double as documentation that
the receiver itself doesn't change. The concrete-first move also
made the env-injection prerequisite (issue #02) actionable —
before this, we had no consumer that needed identity in env, so the
issue stayed parked.

## 18. Drop `self` from the user-facing CLI; env is the identity surface

### What we decided

Originally the `pswarm event` subcommand took
`pswarm event <name|self> <event>`, where the literal `self` resolved
the name from `$PSWARM_AGENT_NAME` and switched on best-effort
silent-error semantics. Plugin v0.3.0 removes that argument
entirely: `pswarm event <event>` always reads the agent name from
`$PSWARM_AGENT_NAME`, exits 0 silently when the variable is unset,
and delegates "I want to inject an event for an explicit name" to a
shell-level env override (`PSWARM_AGENT_NAME=foo pswarm event idle`).

This applies as a project-wide principle for any new commands that
care about agent identity: the user-facing CLI takes a name only
when targeting *another* agent (recipient of `send`, target of
`attach`, target of an `inbox post`, etc.), and never uses a `self`
sentinel. Self-targeting comes from env.

### Why `self` was over-generalised

Issue #02 originally framed `self` as a cross-subcommand convention
that would extend to `attach self`, `send self`, `cwd self`, etc.
Working through use cases:

- `attach self` — meaningless; the agent already runs *inside* its
  own PTY.
- `cwd self` — UNIX `pwd` is the right answer.
- `send self` — strictly send-to-self loops; should be refused, not
  enabled.
- `event self` — useful, but only as a *modal hint* that switched
  semantics (env resolution + silent errors).
- `inbox read self` — useful, same modal-hint flavour.

In every "useful" case, the keyword wasn't selecting a target — it
was switching to a different *behaviour mode* (env-driven + silent
errors). Pushing that mode-switch into a subcommand argument
overloads the slot. Letting the env presence drive the mode, and
the CLI surface drive the subcommand contract, separates the two.

### Why env override is enough for debug

The "explicit-name event injection" use case (manual testing, fault
injection) is rare and well-served by a shell idiom every developer
already knows: prefix the env. No new subcommand surface, no
discoverability cost beyond docs. The integration tests target the
protocol directly (not the CLI), so they're unaffected.

### Migration

`plugins/claude-code/hooks/hooks.json` rewritten to call
`pswarm event <event>` (no `self`); plugin version bumped 0.2.0 →
0.3.0; `validate_name`'s reservation of `self` retained
(forward-compatibility against re-introducing the keyword by
accident, plus avoids "agent named self" confusion in `pswarm send`
log lines).

### Reflection

The instinct that prompted `self` — "make the env-driven mode
explicit at the CLI level so a reader of `hooks.json` can see what's
happening" — wasn't wrong, just located in the wrong layer. The
mode is a property of the *invocation context* (env present or
absent), not of the *command*. Surfacing it as a keyword conflated
the two and ate an argument slot that the principle "name argument
= targeting another agent" should own. Removing it makes both the
CLI grammar and the hook-config easier to read.

## 19. `pswarm inbox` — file-direct, line-cursor inter-agent messaging

### What we decided

Add `pswarm inbox post <to> <body>` and `pswarm inbox read [--follow]`
as a file-backed message channel between agents. Each agent has an
append-only JSON Lines file at
`$XDG_STATE_HOME/picoswarm/inbox/<name>.jsonl` plus a sidecar
`<name>.cursor` holding the reader's last-read line number (1-based).
No daemon mediation — senders and readers touch the files directly,
through the `pswarm inbox` thin wrapper.

Receiver-side Claude wraps `pswarm inbox read --follow` in Monitor
to fold inbox notifications into its conversation as they land
(stream of stdout lines = stream of notifications). Non-Claude
agents poll at turn boundaries via plain `pswarm inbox read`.

### Why file-direct (not daemon-mediated)

Agreed earlier (see decision-log item 18 for the analogous `event`
discussion). Concretely: keeps the daemon out of message-passing
state, makes debug trivial (`cat <inbox>.jsonl`), survives daemon
restarts (a daemon-memory cursor would re-emit history on every
restart), and avoids a protocol bump.

### Why line-number cursor (not byte offset)

Both work; line-number wins on partial-write resilience (only
fully-written `\n`-terminated lines are counted) and on debug
ergonomics ("resume from line 32" reads more naturally than "byte
1842"). The performance argument for byte-offset (`O(1)` seek vs
`O(N)` line skip) doesn't matter at expected inbox sizes.

### Why no daemon protocol bump

The inbox lives entirely outside the daemon's protocol. Senders
and readers `open()` the file directly. The daemon's only
involvement is *file lifecycle* — wiping `<name>.jsonl` and
`<name>.cursor` at agent insert (clean slate against crash residue)
and at agent remove / prune. That sits in `Registry::insert` /
`Registry::remove` / `Registry::prune_dead` as best-effort `unlink`
calls.

### Why a hard 4000-byte per-line cap

POSIX guarantees `O_APPEND` writes under `PIPE_BUF` (4096 on Linux)
are atomic. Capping each encoded JSON Line at 4000 bytes (with
~96-byte headroom for envelope + safety) lets the post path do a
single `write_all` and leave atomicity to the kernel — no flock,
no chunking, no reorder hazards. Messages over the cap are
rejected with a clear error pointing at "split or pass a file
path"; the alternative (silently chunking long messages) hides the
boundary problem rather than surfacing it.

### Why sender identity from env (with `--from` override)

Same principle as item 18: self-identity flows through
`$PSWARM_AGENT_NAME`. Driver Claudes (= sessions running pswarm
without themselves being under pswarm) don't have that env, so
they pass `--from <label>` explicitly. There is *no* auto-detection
fallback to `$USER`, `$AI_AGENT`, or anything else — a label that
the user picks beats one we infer, and survey at
`docs/agent-tool-survey.md` confirmed there is no portable
session-id env across agents anyway.

### Why receiver-side silent no-op when env is unset

If a Claude session loaded `pswarm inbox read --follow` via Monitor
and `$PSWARM_AGENT_NAME` is missing (e.g. Claude wasn't spawned
via pswarm), surfacing an error would clutter the conversation.
Same reasoning as the `pswarm event` silent-no-op outside pswarm:
the wrapper is meant to be a true no-op when there's no inbox to
read.

### Why no `<name>` argument on `read`

Following the principle settled in item 18: external CLI face is
self-only (env-driven), debug is `cat $XDG_STATE_HOME/picoswarm/
inbox/<name>.jsonl`. Multi-reader scenarios (e.g. parent + agent
both watching the same inbox) aren't required for MVP and would
need per-reader cursor files anyway.

### Reflection

The inbox started in conversation as a "Monitor as receive box"
sketch tied tightly to Claude Code. Walking through the design
forced two corrections: (1) the file-cursor / file-jsonl layer
generalises beyond Claude — non-Claude agents can poll the same
files via plain `pswarm inbox read` at turn boundaries, so the
primitive is agent-agnostic even if Monitor is currently
Claude-only on the consumer side; (2) keeping the daemon out of
message-passing state preserved the file-direct architecture we
had landed on for `event` and the related cleanup sites, which in
turn meant no protocol bump and no new daemon code beyond a few
`unlink` calls. The inbox is the smallest primitive that lets two
agents talk; the bigger questions (broadcast, topics, ack) are
deferred until use forces them.

