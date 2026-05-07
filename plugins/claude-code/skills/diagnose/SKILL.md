---
name: diagnose
description: Use when picoswarm or its Claude Code plugin behaves unexpectedly — `pswarm send` doesn't submit, hook errors appear in Stop-hook feedback, an `event` doesn't show up in `pswarm ls`, an `inbox post` doesn't reach the receiver, the daemon refuses a fresh client, or a child agent is silent. Walks through the small set of recurring failure modes in the order they should be checked.
---

# picoswarm: diagnose

A flowchart for the recurring failure modes. Each section gives the
symptom signature, the cheap-to-run check, and the fix.

## First look (always cheap)

```sh
pswarm doctor                # daemon up + uptime + version
pswarm ls                    # alive agents and their last_event
which pswarm                 # is this the binary you think it is?
md5sum "$(which pswarm)" target/release/pswarm  # CLI vs build in sync?
ls -t ~/.local/state/picoswarm/daemon.*.log | head -1 | xargs tail -50
```

`daemon.YYYY-MM-DD.log` is the diagnostic surface for everything the
daemon does — `pswarm event` arrivals, rejections, lifecycle. With
`RUST_LOG=picoswarm=debug` the success path also lands there. Tail
it as the first step in any unclear symptom.

## "My hook / event isn't doing the right thing" — pick a section

- Hook ran but errored (you see `Stop hook feedback: …`) → §Stop hook
  feedback
- Hook didn't run at all (no log lines, no event in `pswarm ls`) →
  §pswarm event doesn't show up
- Plugin commands (`/picoswarm:…`) missing entirely → §Plugin doesn't
  load

## "Stop hook feedback: error: unexpected argument" / hook command fails

**Cause**: a live Claude session loaded a plugin version whose hook
commands no longer match the current `pswarm` CLI shape. Plugins are
loaded at session start; reinstalling does not affect already-running
sessions.

**Check**:
```sh
cat $(find ~/.claude/plugins/cache/picoswarm -name 'hooks.json' \
        -newer plugins/claude-code/.claude-plugin/plugin.json) \
    2>/dev/null | jq '.hooks | keys'
```

**Fix**: restart the affected Claude session so it reloads the
plugin. The noise is informational, not blocking — if the session is
expensive to restart, it can run with the old plugin to completion;
new sessions will pick up the current plugin automatically.

## Stale `pswarm` binary on `$PATH`

**Symptom**: things that should work behave as if a feature were
missing or a flag were unknown — usually after a `cargo build`.
`cargo install` may have left a copy in `~/.cargo/bin/pswarm` that
shadows a symlink in `~/bin/`.

**Check**:
```sh
which pswarm
md5sum "$(which pswarm)" target/release/pswarm
```

If the hashes differ or `which` doesn't point at the symlink:

**Fix**:
```sh
rm ~/.cargo/bin/pswarm                                   # remove stale
ln -sf "$PWD/target/release/pswarm" ~/bin/pswarm         # ensure symlink
```

Symlinking `~/bin/pswarm` to the build output means `cargo build`
alone is enough; no `cargo install` step needed.

## Daemon protocol mismatch after a rebuild

**Symptom**: `Error: daemon speaks protocol N; this client speaks M`,
or odd behaviour right after a `cargo build` even with no version
mismatch error.

The daemon auto-shuts on a *higher*-version `Hello` (so a fresh
post-bump CLI immediately triggers daemon restart on next call). But
when the rebuild does not bump the wire protocol (internals only),
the running daemon keeps the older code and the new client connects
fine yet hits stale daemon-side logic.

**Fix**:
```sh
pswarm daemon restart
```

If the running daemon is too old to even understand the restart
request, kill its pid directly (`pkill -f 'pswarm daemon'`) and let
the next CLI call auto-start a fresh one.

## `pswarm send` lands as `[Pasted text +N lines]` and never submits

**Symptom**: a multi-line `pswarm send` shows the placeholder in the
child's TUI but the prompt is never sent. `pswarm view` is one-shot
(snapshot, not stream) so this is safe to grep:

```sh
pswarm view <name> | grep -i pasted
```

**Cause**: long pastes (>~6 lines) are collapsed to a placeholder
in Claude Code's TUI; the trailing CR after the paste-end marker is
consumed by the placeholder transition rather than read as Enter.

**Fix**: in current `pswarm send` (v0.3.0+) the two-CR dance is
automatic and this should never happen. If it does, the binary or
daemon is older than v0.3.0 — see §Stale binary above and
§Daemon protocol mismatch. As a one-time workaround until the
binary catches up, manually send an empty CR after the multi-line:

```sh
pswarm send <name> '<long text>'
pswarm send <name> ''        # forces submit
```

## `pswarm event` doesn't show up in `pswarm ls`

**Symptom**: a hook fires (visible in agent-side `--debug-file`) but
`pswarm ls` shows `last_event: null` or stale.

**Check** the daemon log first:
```sh
ls -t ~/.local/state/picoswarm/daemon.*.log | head -1 \
    | xargs grep -i 'event for unknown agent\|event recorded'
```

Three usual causes:

1. **`event for unknown agent`** in the log → the agent name in
   `$PSWARM_AGENT_NAME` doesn't match a registered entry. Check what
   the hook actually sends:
   ```sh
   pswarm view <name> | strings | grep -E 'PSWARM_AGENT|pswarm event'
   ```

2. **No event arrival in the log** → the hook isn't firing, or the
   plugin's `hooks.json` doesn't have the event wired. The plugin's
   version may be stale (see "Stop hook feedback" above).

3. **Plugin `hooks.json` matcher set to `"*"`** → that's a tool-name
   pattern; it doesn't match non-tool events like `Stop`,
   `Notification`, `SessionStart`, `SessionEnd`. The matcher must be
   either omitted or empty (`""`) for those.

## `pswarm inbox post` doesn't reach the receiver

**Symptom**: parent posts via `pswarm inbox post X "..."`, no
notification arrives in X's Monitor.

**Check** the inbox file directly — it bypasses the daemon entirely:
```sh
cat ~/.local/state/picoswarm/inbox/X.jsonl
cat ~/.local/state/picoswarm/inbox/X.cursor
```

If the file has the message but X's cursor is past the line: X has
already consumed it (Monitor delivered, agent saw or ignored it).

If the file doesn't have the message:

1. **Sender's `pswarm` not on the child's `$PATH`** — the daemon's
   PATH is inherited; if `which pswarm` returns nothing in the child,
   the post failed silently. Verify with
   `pswarm send <child> 'which pswarm'` and check `pswarm view`.

2. **Wrong recipient name** — a typo in the `to` argument creates an
   inbox file under the wrong name. Check the inbox dir
   (`ls ~/.local/state/picoswarm/inbox/`) for unexpected entries.

3. **`--from` missing from a non-pswarm sender** — `pswarm inbox post`
   from a session without `$PSWARM_AGENT_NAME` rejects without
   `--from <label>`. The error surfaces immediately in the caller's
   shell; if it was swallowed (e.g. by a hook script), the post is
   gone. Always test from an explicit shell first.

## Plugin doesn't load or `picoswarm:` commands missing

**Symptom**: `claude plugin list` doesn't include `picoswarm`, or the
expected hooks don't fire.

**Check**:
```sh
claude plugin list | grep -A3 picoswarm
ls ~/.claude/plugins/cache/picoswarm/picoswarm/
```

**Fix**: if the marketplace is registered but the install is stale or
absent:
```sh
claude plugin marketplace update picoswarm
claude plugin uninstall picoswarm@picoswarm
claude plugin install picoswarm@picoswarm
```

Already-running sessions still hold the prior plugin code — restart
them to pick up the new install.

## Child agent is silent / not responding

**Symptom**: a spawned child agent isn't acknowledging anything;
`pswarm ls` shows it as `running` but no events ever fire.

**Check**:
```sh
pswarm view <name> | tail -5 | cat -v   # raw bytes, escape codes visible
pswarm ls --json | jq '.[] | select(.name=="<name>")'
```

Common causes:

1. **Sent a multi-line prompt before the child was ready** — the
   prompt landed before `SessionStart` `idle`. Always wait first:

   ```sh
   until pswarm ls --json \
       | jq -e '.[] | select(.name=="<name>") | .last_event.event=="idle"' \
           >/dev/null 2>&1; do
       sleep 0.3
   done
   ```

   See also §`pswarm send` lands as `[Pasted text +N lines]` if the
   prompt is being received but never submitting.

2. **Permission prompt fired and was missed** — `last_event` is
   `attention`, not `idle`. Either attach to the agent and answer,
   or run children in auto-mode so the prompt does not fire.

3. **The child loaded a stale plugin** — the `SessionStart` hook
   never reaches the daemon, so `pswarm ls` shows no `idle` event.
   Restart the daemon and re-spawn after confirming the plugin is
   v0.3.0+.

## Quick recovery (when no specific symptom matches)

```sh
pswarm clean                  # drop dead registry entries
pswarm daemon restart         # fresh daemon (covers internal-only rebuilds)
```

Most issues clear after `daemon restart` + re-running the failing
command, because the most common root cause is daemon ↔ CLI version
drift after a rebuild. See §First look for the rest of the
diagnostic surface.
