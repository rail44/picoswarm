# Concurrent client attach (read-only observers)

- **Priority:** Medium
- **Status:** Deferred — the streaming use case only becomes valuable
  once #02 (self-invocation) lands; until then, `pswarm view` (#08)
  covers ~80% of the practical need.

### Triggers to revisit

Reconsider when one of these lands:

- #02 (self-invocation) is implemented (= the "agent watches another
  agent live" scenario gains its prerequisite).
- `pswarm view` (#08) starts being polled in a loop as a substitute
  and the friction becomes obvious.
- #18 (TUI dashboard) is built and needs multiple concurrent
  subscribers as backing.

### Description

- **Summary:** Today only one client can be attached to a given agent
  at a time (`AlreadyAttached` error). `docs/plan.md` lists "Multiple-
  client concurrent attach (read-only observers)" under Out-of-MVP.
  The feature gains real value once `pswarm send` (#03) and
  agent-watching-agent scenarios are in regular use.
- **Impact:** "Human attached + another terminal/agent peeks" and
  "CI healthcheck" are currently impossible. The
  agent-observes-another-agent capability that
  `docs/decision-log.md` item 4 calls out as picoswarm's
  differentiator can't actually be exercised.

### Proposed Solutions

1. **Read-only secondary attach** (medium, 2–3 days): add
   `AttachReadOnly` to the protocol and allow multiple subscribers.
   `SessionInbox::subscribe` already supports many subscribers, so
   the daemon work is mostly demoting the `attached` flag to
   "writer exclusion only" and refusing stdin/Resize from non-writer
   attaches. Tradeoff: the "who is the writer" logic needs deciding
   (first-come-first-served vs `--steal`).
2. **All attaches are read-write** (medium–large, 3–5 days): every
   attach can write; concurrent stdin is resolved by the PTY
   (= interleaved). Tradeoff: looks simple but is hard to debug.
3. **Implement `pswarm peek` only as a separate path** (small, 1 day):
   one-shot dump of the ring buffer with no streaming. This is
   essentially issue #08, and the full multi-attach work is then
   deferred. Tradeoff: covers "observation" but leaves "intervention
   bridging" (read+write) unsolved.

### References

- `docs/plan.md` Out-of-MVP, the relevant entry
- `src/daemon/server.rs` — current single-attach exclusion via
  `compare_exchange` on the `attached` flag
- `src/daemon/output_session.rs` — `subscribers: Vec<...>` is
  already designed for multiple subscribers
- Related issues: #03, #08
