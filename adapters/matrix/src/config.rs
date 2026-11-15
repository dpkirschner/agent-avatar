//! Startup configuration, all from the environment, read once.
//!
//! Secrets (the Matrix access token, the avatar's command token) come
//! from 0600 files named by `*_FILE` variables; the Matrix token has no
//! plain-environment form.

/// Everything the adapter needs to run.
#[derive(Debug, Clone)]
pub struct Config {
    /// Base URL of the display server, e.g. `http://127.0.0.1:8075`.
    pub avatar_url: String,
    /// The display server's command token, if it has one.
    pub avatar_token: Option<String>,
    /// Base URL of the Matrix homeserver (client-server API).
    pub homeserver: String,
    /// The adapter's own Matrix user, e.g. `@tablet:example.org`.
    pub user: String,
    /// The room speech goes into and replies come from.
    pub room: String,
    /// The agent's Matrix user: only its messages are spoken.
    pub agent_user: String,
    /// Access token for `user`.
    pub matrix_token: String,
}

pub const DEFAULT_AVATAR_URL: &str = "http://127.0.0.1:8075";

impl Config {
    /// Read the configuration from the process environment.
    pub fn from_env() -> Result<Config, String> {
        Config::from_lookup(|k| std::env::var(k).ok(), read_secret_file)
    }

    /// Read the configuration through `var` (variables) and `file`
    /// (secret files), so tests don't touch the real environment.
    pub fn from_lookup(
        var: impl Fn(&str) -> Option<String>,
        file: impl Fn(&str, &str) -> Result<String, String>,
    ) -> Result<Config, String> {
        let need = |k: &str| {
            var(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
                .ok_or_else(|| format!("{k} is not set"))
        };
        let avatar_url = var("AVATAR_URL").unwrap_or_else(|| DEFAULT_AVATAR_URL.to_string());
        let avatar_token = match var("AVATAR_TOKEN_FILE") {
            Some(path) => Some(file("AVATAR_TOKEN_FILE", &path)?),
            None => None,
        };
        let token_path = need("MATRIX_TOKEN_FILE")?;
        let matrix_token = file("MATRIX_TOKEN_FILE", &token_path)?;
        let cfg = Config {
            avatar_url: trim_slash(&avatar_url),
            avatar_token,
            homeserver: trim_slash(&need("MATRIX_HOMESERVER")?),
            user: need("MATRIX_USER")?,
            room: need("MATRIX_ROOM")?,
            agent_user: need("MATRIX_AGENT_USER")?,
            matrix_token,
        };
        for (k, v) in [
            ("AVATAR_URL", &cfg.avatar_url),
            ("MATRIX_HOMESERVER", &cfg.homeserver),
        ] {
            if !(v.starts_with("http://") || v.starts_with("https://")) {
                return Err(format!("{k} must start with http:// or https://"));
            }
        }
        for (k, v, sigil) in [
            ("MATRIX_USER", &cfg.user, '@'),
            ("MATRIX_AGENT_USER", &cfg.agent_user, '@'),
            ("MATRIX_ROOM", &cfg.room, '!'),
        ] {
            if !v.starts_with(sigil) || !v.contains(':') {
                return Err(format!("{k} {v:?}: expected {sigil}name:server"));
            }
        }
        if cfg.user == cfg.agent_user {
            return Err("MATRIX_USER and MATRIX_AGENT_USER are the same user".into());
        }
        Ok(cfg)
    }
}

fn trim_slash(s: &str) -> String {
    s.trim().trim_end_matches('/').to_string()
}

fn read_secret_file(var: &str, path: &str) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{var} {path}: {e}"))?;
    let t = text.trim();
    if t.is_empty() {
        return Err(format!("{var} {path}: empty"));
    }
    Ok(t.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn base() -> HashMap<&'static str, &'static str> {
        HashMap::from([
            ("MATRIX_HOMESERVER", "http://127.0.0.1:8008/"),
            ("MATRIX_USER", "@tablet:example.org"),
            ("MATRIX_ROOM", "!room:example.org"),
            ("MATRIX_AGENT_USER", "@agent:example.org"),
            ("MATRIX_TOKEN_FILE", "/secrets/matrix"),
        ])
    }

    fn load(env: &HashMap<&str, &str>) -> Result<Config, String> {
        Config::from_lookup(
            |k| env.get(k).map(|v| v.to_string()),
            |_, path| Ok(format!("secret-from-{path}")),
        )
    }

    #[test]
    fn defaults_and_trims() {
        let cfg = load(&base()).unwrap();
        assert_eq!(cfg.homeserver, "http://127.0.0.1:8008");
        assert_eq!(cfg.avatar_url, DEFAULT_AVATAR_URL);
        assert_eq!(cfg.avatar_token, None);
        assert_eq!(cfg.matrix_token, "secret-from-/secrets/matrix");
    }

    #[test]
    fn reads_the_avatar_token_file() {
        let mut env = base();
        env.insert("AVATAR_TOKEN_FILE", "/secrets/avatar");
        assert_eq!(
            load(&env).unwrap().avatar_token.as_deref(),
            Some("secret-from-/secrets/avatar")
        );
    }

    #[test]
    fn rejects_missing_and_malformed() {
        for (k, v) in [
            ("MATRIX_ROOM", ""),
            ("MATRIX_ROOM", "#alias:example.org"),
            ("MATRIX_USER", "tablet"),
            ("MATRIX_HOMESERVER", "127.0.0.1:8008"),
            ("MATRIX_AGENT_USER", "@tablet:example.org"),
        ] {
            let mut env = base();
            env.insert(k, v);
            assert!(load(&env).is_err(), "{k}={v} should be rejected");
        }
        let mut env = base();
        env.remove("MATRIX_TOKEN_FILE");
        assert!(load(&env).unwrap_err().contains("MATRIX_TOKEN_FILE"));
    }
}
