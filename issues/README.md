# Issues

A finer-grained backlog that complements `docs/plan.md` (which holds
overall direction). Each entry is a candidate unit of work; whether
to actually pick one up is a separate decision.

> Note: some duplication with `docs/plan.md` is intentional. The
> issues list is meant to be triage-friendly; the plan is meant to
> hold the larger direction.

## Index

### High (foundational / differentiator-critical)

- ~~01 PaneHost (kitty adapter)~~ — moved out of scope (composition +
  `docs/integration.md`, see decision-log #13)
- [02 Agent self-invocation (env injection + send-to-self guard)](02-agent-self-invocation.md)
  — deferred (revisit when a feature needing `self` is in scope)
- ~~03 `pswarm send` subcommand~~ — resolved (text + stdin + auto-newline,
  protocol bump 4→5)
- ~~04 Sync `docs/protocol.md`~~ — resolved (protocol.md updated to v4
  in the same commit window)

### Medium (feature gaps / existing rough edges)

- ~~05 Agent lifecycle log~~ — dropped (no concrete consumer with
  recovery out of scope; decision-log #14 records the parking note)
- [06 Concurrent client attach (read-only observers)](06-multi-client-readonly-attach.md)
  — deferred (revisit when #02 lands or `pswarm view` polling becomes
  painful)
- ~~07 Screen restoration on reattach (VT parser)~~ — dropped (the
  "biggest friction" framing was speculation, not observed pain;
  refile when it actually hurts)
- ~~08 View an agent's recent output without attaching~~ — resolved
  (`pswarm view <name>` one-shot snapshot, protocol bump 5→6)
- ~~09 Graceful `pswarm rm` (SIGTERM → SIGKILL)~~ — resolved (1 s grace
  with `--force` to skip SIGTERM)
- ~~10 Enriched `pswarm ls` human output~~ — rejected as unnecessary
- ~~11 `pswarm run` env injection~~ — resolved (redesigned as automatic
  client-env inheritance; `-e` flag dropped)
- ~~12 Drop unused `AgentStatus::Idle` / `Unknown`~~ — resolved (removed,
  protocol bump 3→4)
- ~~13 Bash / zsh completions~~ — resolved (adopted clap_complete
  `unstable-dynamic`; dynamic completion in bash / zsh / fish / elvish
  / powershell)
- ~~14 PTY size policy when no client is attached~~ — resolved (keep
  last; documented)
- [15 Config file (`config.toml`)](15-config-file-toml.md) — deferred
  (revisit when a downstream feature needs persistent settings)
- [16 Tag / link / parent-child relationships](16-tag-link-relationships.md)
  — deferred (revisit at 5+ agents regularly, or when #02 makes
  `--parent self` natural)
- ~~17 Daemon log rotation~~ — resolved (tracing-appender daily rotation,
  7-day retention; crash log separated)
- ~~21 Orphan agent prevention on hard daemon crash~~ — resolved (swapped
  to pty-process and installed `PR_SET_PDEATHSIG` via `pre_exec`)
- ~~22 Attach UX on daemon restart~~ — resolved (`daemon stop|restart`
  refuses while clients are attached; `-f` to override; protocol
  bump 6→7)

### Low (only when requirements firm up)

- [18 TUI view (`pswarm tui`) as a complementary view](18-tui-secondary-view.md)
  — deferred (revisit at 10+ agents regularly or when a watch-style
  refresh need emerges)
- ~~19 Cross-host support~~ — rejected as out of scope
- ~~20 Expose pswarm as an MCP server~~ — rejected (Bash-tool path is
  sufficient and MCP itself is in a plateau; revisit only if a real
  cross-vendor consumer surfaces)

## Template

New issues should follow this template:

```markdown
# <Title>

- **Priority:** <High | Medium | Low>

### Description

- **Summary:** brief framing of the problem
- **Impact:** what improves / what we lose by not doing this
- **Proposed Solutions:**
  1. **<approach name>** (size/difficulty): description and tradeoff
  2. **<approach name>** ...
- **References:**
  - related code `path/to/file.rs:LL`
  - related docs `docs/...`
  - external links
  - related issues: #NN
```
