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
}
