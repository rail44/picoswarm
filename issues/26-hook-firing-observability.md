# No first-class observability for plugin hook firings

- **Priority:** Medium
- **Status:** Open

### Description

- **Summary:** When a Claude Code plugin's hook either does not fire
  or fires but fails silently, the only diagnostic path today is
  agent-side: spawn Claude with `--debug-file /tmp/foo.log` and grep
  for `event.sh|completed with`. Nothing on the picoswarm side
  surfaces "this agent has not produced an event yet, here is why."
- **Impact:** The first dogfood pass of `plugins/claude-code/`
  silently failed for two unrelated reasons (`*` matcher, then
  bracketed-paste swallowing the prompt submission). Each one took
  several rounds of agent-side `--debug-file` archaeology to nail
  down. Picoswarm could have helped, and didn't.

### Symptoms observed

- `pswarm ls` shows `last_event: null` indefinitely with no
  indication of *why* — fired-and-rejected (NotFound) and never-fired
  (matcher mismatch) look identical to the operator.
- The `pswarm event self idle` invocation silently exits 0 in the
  hook context (deliberately — see `docs/decision-log.md` item 17),
  which is correct for production but actively hostile to
  diagnosing why nothing reached the daemon.
- The daemon's tracing log only records lifecycle (start / stop /
  PTY EOF) — Event handler invocations are not logged, so even
  reading `daemon.YYYY-MM-DD.log` does not help.

### Symptoms ruled out

- Not specific to the plugin: any agent integration that drives
  `pswarm event` from a hook surface (Codex `notify`, Gemini
  `AfterAgent`, …) inherits the same blind spot.

### Approaches considered

TBD — daemon-side event tracing, a `pswarm doctor --integration`
probe, surfacing the last NotFound rejection on `pswarm ls`, or
something else. Pick one before implementing.

### References

- `src/daemon/server.rs::handle_event` — currently has no logging.
- `plugins/claude-code/hooks/hooks.json` — the hook surface that
  motivated discovering this gap.
