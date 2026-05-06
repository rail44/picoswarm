# Wait for an agent event programmatically

- **Priority:** Medium
- **Status:** Open

### Description

- **Summary:** With the `pswarm event` machinery now in place
  (`docs/decision-log.md` item 17), the daemon knows when an agent
  has signalled `idle` / `attention` / `exit`. There is no
  client-side way to *block* on that signal — every script that
  drives an agent has to fall back on `sleep N` and hope.
- **Impact:** During the work that landed the plugin itself, the
  verification cycle was littered with `sleep 6`, `sleep 22`, and
  `sleep 25` calls, each of which over-waited or under-waited
  depending on what Claude was doing. The new `last_event` field is
  the right signal; the missing piece is letting a script subscribe
  to it.

### Symptoms observed

- During the Claude-plugin dogfood, every "send → check result"
  iteration looked like `pswarm send demo 'hi'; sleep 22; pswarm ls`,
  and each cycle lost time to the slack in the timer.
- The integration test that exercises `pswarm event self idle`
  spawns `bash -c 'pswarm event self idle && sleep 60'` for the same
  reason — there is no way to say "run until the next event".

### Approaches considered

TBD — several plausible shapes (long-poll subcommand, watch-style
streaming, tail-the-event-log file). Pick one before implementing.

### References

- `docs/decision-log.md` item 17 — `Event` enum and `last_event`
  field that this would consume.
- `src/daemon/registry.rs` — `AgentEntry.last_event` is the source
  of truth.
