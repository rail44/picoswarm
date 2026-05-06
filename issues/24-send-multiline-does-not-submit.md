# `pswarm send` of multi-line text does not submit

- **Priority:** Medium
- **Status:** Open

### Description

- **Summary:** `pswarm send <name> '<multi-line text>'` writes the
  text into the agent's PTY and appends a single CR, expecting that
  CR to act as Enter. For a TUI in raw mode that handles its own
  paste detection (Claude Code, and likely others), the multi-byte
  blob is recognised as a bracketed-paste and the trailing CR is
  consumed as part of the paste payload rather than as a separate
  Enter. The text lands in the input box, but is **not** submitted.
- **Impact:** Every multi-line `pswarm send` requires a follow-up
  `pswarm send <name> ''` to fire a bare Enter. Easy to forget,
  doubles the round-trip per prompt, and is a sharp footgun for
  scripts that drive agents non-interactively.

### Symptoms observed

- During the Claude-plugin dogfood, sending a 26-line research
  prompt left Claude showing `[Pasted text #1 +26 lines]` in the
  input box. A second `pswarm send demo ''` was needed to actually
  submit.
- Single-line `pswarm send <name> 'hi'` is unaffected, so the bug is
  specifically in how raw-mode agents distinguish "user paste" from
  "user typing followed by Enter".

### Approaches considered

TBD — could detect multi-line and adjust on the send side, surface
an explicit `--enter` / `--no-enter` flag, or emit bracketed-paste
markers ourselves so the trailing CR lands outside the paste. Pick
one before implementing.

### References

- `src/client/send.rs` — current trim-then-CR logic.
- `docs/decision-log.md` item 16 — the PTY-wrap thesis that makes
  this kind of TUI quirk unavoidable in principle.
