# macOS daemon-hard-crash orphan prevention

- **Priority:** Medium
- **Status:** Open

### Description

- **Summary:** Linux spawning installs `PR_SET_PDEATHSIG(SIGTERM)` (see
  decision-log #11 / #15) so a daemon hard-crash takes its agent
  processes down with it. macOS has no kernel-level equivalent; the
  Phase-1 macOS support gates `prctl` to Linux only, leaving
  daemon-hard-crash as a real failure mode where agent processes
  orphan to launchd and the user has to clean them up manually
  (`pkill -f claude` etc.).
- **Impact:** picoswarm's "the daemon owns its agents" guarantee is
  one of the differentiators. Without orphan prevention on macOS, a
  user who experiences a daemon crash mid-session is stuck with stale
  agent processes that picoswarm has no record of and cannot
  reclaim. The graceful-shutdown path (`pswarm daemon stop`) still
  cleans up correctly — only hard crash is exposed.

### Symptoms observed

- Anticipated pain rather than observed: the issue is filed at the
  moment macOS support lands so that the next time a daemon crash
  leaves orphans behind, there's an existing entry to revive instead
  of rediscovering the gap.

### Approaches considered

The shape we've settled on without committing to implementation is
W1 from the Phase-1 design discussion:

1. **kqueue `EVFILT_PROC` + `NOTE_EXIT` shim** *(deferred)*. Spawn a
   small watchdog process (a hidden subcommand on the picoswarm
   binary itself, e.g. `pswarm internal-watchdog --daemon-pid=N --
   <real-cmd>`) between fork and exec. The shim spawns the actual
   agent, registers a kqueue watch on the daemon PID, and on
   `NOTE_EXIT` sends SIGTERM to its child and exits. ~50–60 LOC +
   `kqueue` crate (macOS-only dep) + new internal subcommand.
   Trade-off: 1 extra process per agent (PID + a few KB RSS); the
   only path that gives macOS the same UX as Linux's
   `PR_SET_PDEATHSIG`.

Rejected:

2. **Polling `kill(getppid(), 0)` in the child** — `getppid()` is
   reparented to launchd on parent death, so the polling check
   silently switches reference; brittle.
3. **Mach IPC** — too OS-specific, large surface for one signal.
4. **Process group / session leader tricks** — daemonised processes
   have no controlling terminal, so SIGHUP propagation does not
   apply.

### Refile / pickup trigger

- A first observed daemon hard-crash that leaves orphans on macOS
  (real pain, not anticipated).
- A user reports the cleanup workaround (`pkill`) as recurring
  friction.
- We have appetite for the +60 LOC complexity; until then, the
  decision-log entry that gates `prctl` to Linux notes the gap
  explicitly.

### References

- `src/daemon/session.rs` — current `PR_SET_PDEATHSIG` install site,
  Linux-gated in Phase-1.
- `docs/decision-log.md` items 11 and 15 — original orphan-prevention
  reasoning (Linux only at the time).
- `docs/decision-log.md` item TBD — Phase-1 macOS gate + this
  deferred-orphan-prevention rationale.
