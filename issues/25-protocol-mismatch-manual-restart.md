# Protocol bumps require a manual daemon restart

- **Priority:** Low
- **Status:** Open

### Description

- **Summary:** When the wire protocol bumps (most recently
  `v7 → v8`), a freshly built client refuses to talk to the
  still-running daemon (`Error: ProtocolMismatch …`). The developer
  has to remember to `pswarm daemon stop` (using a client old enough
  to handshake) before the new build can do anything.
- **Impact:** Hits every contributor every time the protocol
  changes, which has been roughly once per shipped feature so far
  (`v3 → v8` over the project's lifetime). Easy to work around once
  the failure mode is recognised, but the first error message is
  cryptic enough that it costs minutes the first time.

### Symptoms observed

- During the `v7 → v8` bump for the `Event` work, the first
  `target/release/pswarm doctor` after rebuild errored with
  `daemon speaks protocol 7; this client speaks 8`. Recovery was
  `pswarm daemon stop` (with the old `~/.cargo/bin/pswarm`) followed
  by a `target/release/pswarm` invocation that auto-started a v8
  daemon.

### Approaches considered

TBD — could auto-restart on mismatch (opt-in), have the daemon do a
clean shutdown when it sees a higher-version `Hello`, or simply
improve the error message with the recovery command. Pick one before
implementing.

### References

- `src/client/connection.rs:50–60` — the version-mismatch error
  path.
- `src/daemon/server.rs:121–140` — the daemon-side handshake.
