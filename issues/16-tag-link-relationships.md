# Tag / link / parent-child relationships

- **Priority:** Medium
- **Status:** Deferred — held for all three sub-features until
  there's concrete operational pressure.

### Triggers to revisit

Reconsider when one of these becomes a real friction:

- Agent count is regularly 5+ and identifying/filtering by name alone
  is annoying (→ start with tag).
- #02 (self-invocation) lands and a natural use for `--parent self`
  surfaces (→ parent-child).
- Batch operations like "kill every worktree agent" become routine
  (→ tag + `rm --tag`).

Even tag alone takes 1–2 days once protocol changes, filter
semantics (AND/OR), display, and `rm --tag` safety are factored in.
Designing after the operational pattern is visible reduces wrong
shapes. `link` is the weakest of the three and should land last,
if at all.

### Description

- **Summary:** `CLAUDE.md` "Confirmed direction / Scope / In scope"
  lists "parent-child / tags," but none of protocol / registry /
  CLI implements any of them. `docs/plan.md` keeps the trio under
  Out-of-MVP "link / tag / parent-child relationships."
- **Impact:** No way to organise agents once the count grows
  (`pswarm ls --tag worktree`, `pswarm rm --tag stale`). No way for
  an agent to discover its "parent." Long-term these are still
  expected because `CLAUDE.md` commits to them.

### Proposed Solutions

1. **Tag only, first** (small–medium, 1–2 days): `pswarm run --tag
   foo --tag bar`; add `tags: Vec<String>` to `AgentSummary`;
   `pswarm ls --tag` filters; `pswarm rm --tag` does batch delete.
   Tradeoff: parent-child stays out of scope for now.
2. **Tag + parent-child together** (medium, 3–5 days): `--parent
   <name|id|self>` to create the edge; `pswarm ls --tree` to
   display. With `PSWARM_AGENT_ID` from #02, an agent spawning its
   own children naturally falls out. Tradeoff: requires deciding
   the relation model (1:N vs N:M) and wire format extension.
3. **Add `link` (arbitrary labelled edges)** (medium, +1–2 days
   on top of #2): `pswarm link <a> <b> --as upstream`. Tradeoff:
   probably over-general for our scale.

### References

- `CLAUDE.md` — "In scope" entry
- `docs/plan.md` Out-of-MVP and Next candidates
- `src/daemon/registry.rs` — extend `AgentEntry` with `tags` /
  `parent_id`
- Related: #02 (self-invocation pairs naturally with parent-child)
