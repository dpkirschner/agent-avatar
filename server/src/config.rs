//! Startup configuration: the screens config file, the command token and
//! the avatar to draw.
//!
//! Both are read once at startup. A bad screens file or an unreadable
//! token file stops the server rather than starting it half-configured.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

/// One screen an agent may show: a name it puts in a `screen` card, a
/// title for the page, the URL the page frames, and the query params
/// the agent may add to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Screen {
    pub name: String,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, ParamRule>,
}

/// What a screen param accepts: one of a list of values, or any value of
/// a kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamRule {
    OneOf(Vec<String>),
    Kind(ParamKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParamKind {
    /// `YYYY-MM-DD`.
    Date,
    /// A whole number.
    Int,
}

/// The avatars built into the page, by name (`app/avatars/<name>/`).
pub const FACES: &[&str] = &["blob"];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ScreensFile {
    #[serde(default)]
    screen: Vec<Screen>,
}

/// Parse and validate a screens config (TOML, `[[screen]]` tables).
///
/// Names are lowercase `[a-z0-9_-]`, 1-40 characters, unique. URLs are
/// absolute `http://` or `https://`. Titles are non-empty. Param names
/// follow the screen-name rule; a value list is non-empty, each value
/// 1-40 characters.
pub fn parse_screens(text: &str) -> Result<Vec<Screen>, String> {
    let file: ScreensFile = toml::from_str(text).map_err(|e| e.to_string())?;
    let mut seen = HashSet::new();
    for s in &file.screen {
        if !valid_name(&s.name) {
            return Err(format!(
                "screen name {:?}: use 1-40 characters from a-z 0-9 _ -",
                s.name
            ));
        }
        if !seen.insert(s.name.clone()) {
            return Err(format!("screen name {:?} is listed twice", s.name));
        }
        if s.title.trim().is_empty() {
            return Err(format!("screen {:?}: empty title", s.name));
        }
        if !(s.url.starts_with("https://") || s.url.starts_with("http://")) {
            return Err(format!(
                "screen {:?}: url must start with https:// or http://",
                s.name
            ));
        }
        for (key, rule) in &s.params {
            if !valid_name(key) {
                return Err(format!(
                    "screen {:?}: param name {key:?}: use a-z 0-9 _ -",
                    s.name
                ));
            }
            if let ParamRule::OneOf(vals) = rule {
                if vals.is_empty() || vals.iter().any(|v| v.is_empty() || v.chars().count() > 40) {
                    return Err(format!(
                        "screen {:?}: param {key:?}: list 1-40 character values",
                        s.name
                    ));
                }
            }
        }
    }
    Ok(file.screen)
}

fn valid_name(name: &str) -> bool {
    (1..=40).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Load the screens config from `AVATAR_SCREENS`. Unset means no screens
/// (the avatar alone); set but unreadable or invalid is an error.
pub fn load_screens() -> Result<Vec<Screen>, String> {
    let Ok(path) = std::env::var("AVATAR_SCREENS") else {
        return Ok(Vec::new());
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("AVATAR_SCREENS {path}: {e}"))?;
    parse_screens(&text).map_err(|e| format!("AVATAR_SCREENS {path}: {e}"))
}

/// The avatar to draw, from `AVATAR_FACE` (default `blob`). An unknown
/// name stops the server.
pub fn load_face() -> Result<String, String> {
    let face = std::env::var("AVATAR_FACE").unwrap_or_else(|_| FACES[0].to_string());
    if FACES.contains(&face.as_str()) {
        Ok(face)
    } else {
        Err(format!("AVATAR_FACE {face:?}: have {}", FACES.join(", ")))
    }
}

/// The command token, from `AVATAR_TOKEN_FILE` (a 0600 file, preferred)
/// or `AVATAR_TOKEN`. `None` leaves the commands open.
pub fn load_token() -> Result<Option<String>, String> {
    if let Ok(path) = std::env::var("AVATAR_TOKEN_FILE") {
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("AVATAR_TOKEN_FILE {path}: {e}"))?;
        return non_empty(text.trim(), "AVATAR_TOKEN_FILE").map(Some);
    }
    match std::env::var("AVATAR_TOKEN") {
        Ok(t) => non_empty(t.trim(), "AVATAR_TOKEN").map(Some),
        Err(_) => Ok(None),
    }
}

fn non_empty(t: &str, what: &str) -> Result<String, String> {
    if t.is_empty() {
        Err(format!("{what} is set but empty"))
    } else {
        Ok(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_example_config() {
        let text = include_str!("../../examples/screens.toml");
        let screens = parse_screens(text).unwrap();
        assert_eq!(screens.len(), 2);
        assert_eq!(screens[0].name, "status");
    }

    #[test]
    fn empty_file_means_no_screens() {
        assert!(parse_screens("").unwrap().is_empty());
    }

    #[test]
    fn rejects_bad_names_duplicates_and_urls() {
        let one = |name: &str, url: &str| {
            format!("[[screen]]\nname = {name:?}\ntitle = \"T\"\nurl = {url:?}\n")
        };
        assert!(parse_screens(&one("Status", "https://a.example/")).is_err());
        assert!(parse_screens(&one("", "https://a.example/")).is_err());
        assert!(parse_screens(&one("a b", "https://a.example/")).is_err());
        assert!(parse_screens(&one("ok", "javascript:alert(1)")).is_err());
        assert!(parse_screens(&one("ok", "/relative")).is_err());
        let dup = one("x", "https://a.example/") + &one("x", "https://b.example/");
        assert!(parse_screens(&dup).unwrap_err().contains("twice"));
    }

    #[test]
    fn parses_param_rules() {
        let screens = parse_screens(include_str!("../../examples/screens.toml")).unwrap();
        let w = &screens[1].params;
        assert!(matches!(&w["range"], ParamRule::OneOf(v) if v.contains(&"7d".to_string())));
        assert_eq!(w["day"], ParamRule::Kind(ParamKind::Date));
        let base = "[[screen]]\nname = \"a\"\ntitle = \"A\"\nurl = \"https://a.example/\"\n";
        assert!(parse_screens(&format!("{base}[screen.params]\nx = \"float\"\n")).is_err());
        assert!(parse_screens(&format!("{base}[screen.params]\nx = []\n")).is_err());
        assert!(parse_screens(&format!("{base}[screen.params]\nBad = [\"a\"]\n")).is_err());
    }

    #[test]
    fn rejects_unknown_keys() {
        let text =
            "[[screen]]\nname = \"a\"\ntitle = \"A\"\nurl = \"https://a.example/\"\nsrc = \"x\"\n";
        assert!(parse_screens(text).is_err());
    }
}
