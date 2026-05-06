# Forward agent permission prompts without requiring an attach

- **Priority:** Medium
- **Status:** Open

### Description

- **Summary:** When a spawned Claude agent hits a permission prompt
  (a tool wants to run, a file wants to be edited, etc.), the only
  way to act on that prompt today is for the human to `pswarm
  attach <name>` into the agent's PTY and answer it interactively.
  Detached drivers, scripts, and "many agents one operator" flows
  all stall the moment any agent asks a question.
- **Impact:** The whole point of detached agents is that the
  operator does not have to babysit each PTY. The current attach
  requirement re-couples each agent to the operator's terminal
  exactly when the agent is least productive. The pressure to reach
  for `--dangerously-skip-permissions` instead is what keeps
  bringing this up.

### Symptoms observed

- During this session's dogfood, `--dangerously-skip-permissions`
  was used as a convenience and pushed back on; reverting to the
  default left the workflow unable to proceed if Claude had asked
  to use any tool.
- The `Notification` hook now records `attention` events
  (`pswarm ls` shows the state), so the daemon already knows when
  this is happening. The missing piece is the forward-and-reply
  channel.

### Approaches considered

TBD — many shapes possible (`pswarm approve <name> yes/no`, a
desktop-notification daemon hook, a TUI inbox view, structured
OSC9 alerts to the operator's terminal). The receiver side is in
place; the relay back into the agent's PTY is the open question.
Pick before implementing.

### References

- Auto-memory `project_approval_forwarding_future.md` — flagged
  this as a desired direction during the plugin work.
- `docs/decision-log.md` item 17 — `attention` event already wired
  through `Notification` hook.
- `plugins/claude-code/hooks/hooks.json` — current `attention` glue.
