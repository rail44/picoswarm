# Config file (`config.toml`)

- **Priority:** Medium
- **Status:** Deferred — held until a downstream feature actually
  needs persistent configuration.

### Triggers to revisit

Reconsider when one of these lands:

- A future window-management hook (see `docs/integration.md`
  "Future hook contract") that wants its script path stored
  somewhere other than env. (Note: #01 and #20 — original triggers —
  have since been dropped, so this is the remaining concrete path.)
- Any feature that produces "settings that don't fit in a CLI flag
  or env var."

Designing the schema alongside the first concrete consumer makes
it less likely to land in the wrong shape than building an empty
shell first. The config layer itself is a one-time read at
startup and not invasive, so the cost of deferring is low.

### Description

- **Summary:** `docs/plan.md` Open Decisions item 2 reserves the
  `$XDG_CONFIG_HOME/picoswarm/config.toml` path but doesn't ship
  anything. The need was originally framed around adapter selection
  (#01, dropped) and preferred PTY size (#14, resolved without
  config). The current open need would be a future hook contract
  (per `docs/decision-log.md` 13) where a config field is the
  documented escape hatch for hook scripts that should not be
  configured via env.
- **Impact:** Currently zero immediate friction. Becomes a blocker
  only if a feature lands that genuinely cannot fit in a CLI flag
  or env var.

### Proposed Solutions

1. **Minimal schema with `toml` + `serde`** (small, half-day): read
   once at startup into `Arc<Config>`. Empty/missing file allowed,
   defaults fill in. Tradeoff: none — extend per concrete need.
2. **`figment` for env/file/CLI layering** (medium, 1–2 days): one
   unified source-of-truth for settings, at the cost of a heavier
   dependency and more conceptual surface. Tradeoff: probably
   over-scoped for a solo CLI.
3. **Skip entirely (CLI flags only)** (none): falls apart the moment
   a feature wants multi-valued or per-agent settings. Tradeoff:
   pain later.

### References

- `docs/plan.md` Open Decisions item 2
- `src/paths.rs` — XDG path helpers (a `config_path()` would slot in)
- Related: `docs/decision-log.md` 13 (future hook contract names
  config-file as the safe escape hatch — no env-based hooks)
