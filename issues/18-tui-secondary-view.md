# TUI view (`pswarm tui`) — as a complementary view

- **Priority:** Low
- **Status:** Deferred — not picked up while the CLI is sufficient.

### Triggers to revisit

Reconsider when one of these lands:

- Agent count is regularly 10+ and `pswarm ls` output gets hard to
  scan → consider option 1 (ratatui dashboard).
- A lightweight "refresh `ls` at ~1 Hz" need surfaces → consider
  option 2 (`pswarm watch` standalone).

`CLAUDE.md` "Decisions that must not drift" makes "TUI is never the
primary entry point" non-negotiable, so a TUI is always the
complementary view. As long as the CLI is enough, the decision to
build one is driven by actual demand, not aspiration.

### Description

- **Summary:** `CLAUDE.md` "Decisions that must not drift" allows a
  TUI as a complementary view added later, just not as the primary
  UX. `docs/plan.md` lists the same: build it once the CLI list
  view stops being sufficient.
- **Impact:** Today the CLI is enough; immediate friction is zero.
  Value emerges when 10+ agents need to be monitored in parallel.

### Proposed Solutions

1. **Minimal `ratatui` dashboard** (medium–large, 3–5 days):
   `pswarm ls --watch`-equivalent plus a few-line preview of each
   agent's recent output. Combined with read-only attach (#06) it
   doubles as a tail view. Tradeoff: heavy dependency, real UX
   design effort.
2. **Just `pswarm watch`** (small–medium, 1–2 days): no TUI, just
   redraw `pswarm ls` at ~1 Hz. Tradeoff: not the final shape but
   probably enough to scratch the immediate itch.
3. **Skip** (none): keep the "CLI is sufficient" stance. Tradeoff:
   optimal if a TUI never proves necessary.

### References

- `CLAUDE.md` "Decisions that must not drift" — TUI must not be
  the primary entry point
- `docs/decision-log.md` item 4 (ccmanager evaluation) — the
  explicit rejection of TUI-first
- Related: #06, #08
