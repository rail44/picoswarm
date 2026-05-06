# `pswarm view` output is unreadable for humans

- **Priority:** Low
- **Status:** Open

### Description

- **Summary:** `pswarm view <name>` returns the agent PTY's recent
  output as raw bytes, including all the VT escape sequences a TUI
  emits to position the cursor and repaint regions. Forwarding bytes
  verbatim is correct for a future "render the snapshot back into a
  terminal" use case, but for ad-hoc human inspection (the most
  common use during development) it is unreadable without `cat -v`
  and squinting.
- **Impact:** Every "did Claude actually respond?" check during
  development becomes `pswarm view name 2>&1 | tail -10 | cat -v`,
  and even then the answer is buried in cursor-positioning noise.
  Encourages reaching for screen scraping when the new event
  channel would be more reliable.

### Symptoms observed

- During the plugin dogfood, several iterations of "did the prompt
  get through?" devolved into reading lines like
  `^[[2C^[[3A^[[?2026l^[]0;... Brief greeting^[\^[[?2026h…` to find
  the title-bar marker that confirmed Claude had received the prompt.
- The friction is *not* present for non-TUI agents (`/bin/cat`,
  `/bin/sh`) — only for raw-mode TUIs that draw with cursor
  movements rather than scrolling text.

### Approaches considered

TBD — could ship a `--decode` / `--text` mode that runs the bytes
through a VT parser and emits the visible-screen text only, document
the current behaviour as the byte-pipe contract, or split the
subcommand into "raw" and "human" variants. Pick one before
implementing.

### References

- `src/client/view.rs` — current pass-through.
- `src/daemon/output_session.rs` — ring buffer producing the bytes.
- `CLAUDE.md` "Decisions that must not drift" — the "do not
  implement a full terminal emulator" line is relevant context.
