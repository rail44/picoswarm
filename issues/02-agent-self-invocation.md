# Agent self-invocation (env injection + send-to-self guard)

- **Priority:** High
- **Status:** Resolved in shape (env injection landed for `pswarm
  event` — `docs/decision-log.md` items 17 / 18). The dedicated
  `self` keyword once envisioned for `attach` / `cwd` / `send` etc.
  was abandoned: walking through use cases showed those commands
  either don't have a meaningful self target (`attach`, `cwd` —
  UNIX equivalents already exist) or should refuse self
  (`send` — loop hazard). Self-driven commands now read identity
  from `$PSWARM_AGENT_NAME` directly, no `self` keyword. The
  send-to-self loop guard remains a small future task.

### Description

- **Summary:** `CLAUDE.md` "Decisions that must not drift" states "Agents
  must be able to invoke the CLI on themselves (single binary, callable
  from a subshell)." Two pieces are required: (a) inject the agent's id
  into the spawned PTY's environment, (b) resolve `pswarm <verb> self`
  to that id and refuse send-to-self loops. `docs/plan.md`'s "PTY env
  policy" already promised the env injection, but `src/daemon/session.rs`
  currently sets only `PSWARM_DAEMON=1`.
- **Impact:** "CLI-first" was nailed down as picoswarm's differentiator
  (`docs/decision-log.md` item 4). That story isn't complete until an
  agent can find itself via the same CLI. However, **#03 (send) does not
  depend on self-identification** — `pswarm send <other-name>` works
  without it — so the work can wait until `self` keywords or
  send-to-self guards are actually in scope.

### Identification approaches considered

| Approach | Mechanism | Pros | Cons |
|---|---|---|---|
| **A. env injection** | inject `PSWARM_AGENT_ID` / `_NAME` at spawn | ~5 lines / standard Unix idiom (TMUX, KITTY_LISTEN_ON, …) / survives subshell, nohup, setsid | leaks into all child processes / can be unset/spoofed inside the agent |
| **B. getpeercred + ppid walk** | daemon reads connecting peer's pid, walks `/proc/<pid>/status` PPid up to a registered agent pid | zero env pollution / unspoofable | 30–50 lines / fragile under `nohup` (session detach) / pid reuse race (essentially zero impact in practice) |
| C. cwd match | match the connecting client's cwd against the agent's cwd | simple | ambiguous when multiple agents share a worktree → rejected |
| D. controlling tty | reverse-look-up from peer's tty to the agent's PTY | natural | agent's own children share the same tty, so the indirection is the same as env and harder to implement |
| E. marker file in cwd | drop `.pswarm-agent-id` in the worktree | no env use | pollutes the user's worktree → rejected |
| F. socket fd inheritance | hand an open socket fd to the spawned agent | unspoofable | subshells don't inherit fds (= weaker than env) → rejected |

The realistic candidates are **A (env)** and **B (peercred + ppid walk)**.
Given the threat model is "single user on a local machine," spoof
resistance is a weak requirement. Env is enough for now.

### Proposed Solutions

- **(first pick when this is taken up) Approach A: env injection only**
  (small, half-day): add `PSWARM_AGENT_ID` / `PSWARM_AGENT_NAME` to
  `cmd.env` in `session.rs`. Provide an `agent_id_from_env()` helper
  inside `pswarm` so client subcommands can pick it up when needed. The
  send-to-self guard is added when send actually needs it. Tradeoff:
  the safety story is delegated.
- env + dedicated `self` keyword (medium, 1 day): on top of A, make
  `pswarm <verb> self` resolve to the env id for attach / send / cwd /
  etc. Tradeoff: introduces a cross-subcommand convention.
- env + global self-guard middleware (medium, 1–2 days): in the
  connection layer, refuse any operation where `PSWARM_AGENT_ID`
  matches the target. Tradeoff: adds a layer to the otherwise thin
  client implementation.
- Approach B (peercred + ppid walk) (medium–large, 2–3 days):
  alternative for when env pollution becomes unacceptable. Hard to
  justify under the current threat model — held as a fallback for
  when env runs into trouble.

### Triggers to revisit

Reconsider when one of these actually lands:

- #16 (tag/link) needs `--parent self`.
- #03 (send) gains a send-to-self guard (= a loop accident becomes
  visible).
- A lifecycle-log-style attribution ("which agent invoked which") is
  desired.

### References

- `CLAUDE.md` "Decisions that must not drift" — self-invocation bullet
- `docs/plan.md` Resolved "PTY env policy" — env injection is promised
- `src/daemon/session.rs:51-58` — current env-injection site
- Related issues: #03 (send works standalone), #16 (tag/link), and
  the dropped #05 (lifecycle log)
