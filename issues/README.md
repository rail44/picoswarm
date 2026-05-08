# Issues

A historical record of friction observed in picoswarm, plus a small
backlog when there is currently-felt pain that is not yet resolved.

## Principle

**Issue = current pain.** A new issue file is filed when something
genuinely hurts *now* — a daily annoyance during use, a recurring
bug, a feature gap that blocks something the user wants to do today.
Speculative "we might want this someday" entries do not earn a file;
they live (briefly) in this index as struck-through history with a
note about what trigger would justify refiling.

This convention emerged from a sweep on 2026-05-07 that drained the
backlog of speculative entries — see the strikethrough entries below
for what was considered and dropped, with refile triggers attached.

## Index

### Open

- [29 macOS daemon-hard-crash orphan prevention](29-macos-orphan-prevention.md)
  — open (Linux installs `PR_SET_PDEATHSIG`; macOS has no kernel
  equivalent and gets no orphan protection on hard crash; deferred
  pending appetite for a kqueue-shim approach)

### High (foundational / differentiator-critical)

- ~~01 PaneHost (kitty adapter)~~ — moved out of scope (composition +
  `docs/integration.md`, see decision-log #13)
- ~~02 Agent self-invocation (env injection + send-to-self guard)~~ —
  partially resolved (env injection landed via decision-log #17 / #18);
  the send-to-self loop guard was speculative and dropped — refile if
  a real self-loop bug appears
- ~~03 `pswarm send` subcommand~~ — resolved (text + stdin + auto-newline,
  protocol bump 4→5)
- ~~04 Sync `docs/protocol.md`~~ — resolved (protocol.md updated to v4
  in the same commit window)

### Medium (feature gaps / existing rough edges)

- ~~05 Agent lifecycle log~~ — dropped (no concrete consumer with
  recovery out of scope; decision-log #14 records the parking note)
- ~~06 Concurrent client attach (read-only observers)~~ — speculative;
  refile when `pswarm view` polling becomes painful or a multi-watcher
  use case lands
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
- ~~15 Config file (`config.toml`)~~ — speculative; refile when a
  downstream feature actually needs persistent settings
- ~~16 Tag / link / parent-child relationships~~ — speculative; refile
  when 5+ concurrent agents become routine
- ~~17 Daemon log rotation~~ — resolved (tracing-appender daily rotation,
  7-day retention; crash log separated)
- ~~21 Orphan agent prevention on hard daemon crash~~ — resolved (swapped
  to pty-process and installed `PR_SET_PDEATHSIG` via `pre_exec`)
- ~~22 Attach UX on daemon restart~~ — resolved (`daemon stop|restart`
  refuses while clients are attached; `-f` to override; protocol
  bump 6→7)
- ~~24 `pswarm send` of multi-line text does not submit~~ — resolved
  (wrap multi-line payloads in bracketed-paste markers + send the CR
  in a separate request so Claude's placeholder transition lands
  before the Enter)
- ~~26 Hook firing observability~~ — resolved (daemon now logs every
  `pswarm event` arrival: `debug!` on success, `warn!` on rejection,
  rotated daily log file is the diagnostic surface)
- ~~28 Forward agent permission prompts without an attach~~ — depends
  on agent-side hook/tool ecosystem evolution; refile if a usable
  surface emerges (memory `project_approval_forwarding_future`
  records the desired direction)

### Low (only when requirements firm up)

- ~~18 TUI view (`pswarm tui`) as a complementary view~~ — speculative;
  refile when 10+ agents become routine or watch-style refresh
  becomes painful
- ~~19 Cross-host support~~ — rejected as out of scope
- ~~20 Expose pswarm as an MCP server~~ — rejected (Bash-tool path is
  sufficient and MCP itself is in a plateau; revisit only if a real
  cross-vendor consumer surfaces)
- ~~23 Wait for an agent event programmatically~~ — obviated by
  `until <jq on pswarm ls>; do sleep 0.3; done` pattern; the
  `pswarm watch` streaming follow-on is speculative — refile when
  multi-agent driving needs streaming
- ~~25 Protocol bumps require a manual daemon restart~~ — workaround
  (`pswarm daemon stop`) is fine; refile if rebuild cycles get slow
  enough to matter
- ~~27 `pswarm view` output is unreadable for humans~~ — obviated by
  the event-based workflow (#24 + #26); the VT-decoder follow-on is
  speculative — refile when programmatic text capture becomes a real
  need

## Template

```markdown
# <Title>

- **Priority:** <High | Medium | Low>
- **Status:** Open

### Description

- **Summary:** brief framing of the problem
- **Impact:** what specifically hurts right now

### Symptoms observed

- concrete, recent examples of the friction
- timestamps / commits that reproduce help

### Approaches considered

- TBD when filed for the pain alone
- otherwise, candidate implementations with trade-offs:
  1. **<approach name>** (size/difficulty): description and tradeoff
  2. **<approach name>** ...

### References

- related code `path/to/file.rs:LL`
- related docs `docs/...`
- related issues: #NN
```

A new file is filed only when there is concrete pain *now*. If the
strongest framing is "we might want this someday", that's not an
issue — leave it for when the pain shows up. The strikethrough
entries above each carry a refile trigger so future-us can recognise
when the pain has actually arrived.
