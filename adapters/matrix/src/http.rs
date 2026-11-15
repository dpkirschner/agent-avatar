//! The two HTTP peers: the Matrix homeserver (client-server API, just
//! the endpoints the adapter needs) and the display server.

use crate::config::Config;
use reqwest::{Client, RequestBuilder, Response, StatusCode};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

/// A failed call: `Fatal` stops the adapter (bad token, not in the
/// room, encrypted room); `Retry` is worth trying again later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Fatal(String),
    Retry(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Fatal(m) | Error::Retry(m) => f.write_str(m),
        }
    }
}

/// Send a request; a 2xx comes back, 401/403 are fatal, anything else
/// (including the network) is retryable.
async fn call(what: &str, req: RequestBuilder) -> Result<Response, Error> {
    let resp = req
        .send()
        .await
        .map_err(|e| Error::Retry(format!("{what}: {e}")))?;
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    let body = resp.text().await.unwrap_or_default();
    let msg = format!("{what}: HTTP {status} {}", body.trim());
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(Error::Fatal(msg)),
        _ => Err(Error::Retry(msg)),
    }
}

fn base_url(s: &str) -> Url {
    Url::parse(s).expect("config checked the scheme")
}

/// `base` with `segments` appended, each percent-encoded as one segment.
fn join(base: &Url, segments: &[&str]) -> Url {
    let mut u = base.clone();
    u.path_segments_mut()
        .expect("http(s) URLs have a path")
        .pop_if_empty()
        .extend(segments);
    u
}

/// How long a `/sync` long-poll waits for news, in milliseconds.
pub const SYNC_TIMEOUT_MS: u64 = 30_000;

pub struct Homeserver {
    http: Client,
    base: Url,
    token: String,
    room: String,
    txn_prefix: String,
    txn: AtomicU64,
}

impl Homeserver {
    pub fn new(http: Client, cfg: &Config) -> Self {
        let started = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        Homeserver {
            http,
            base: base_url(&cfg.homeserver),
            token: cfg.matrix_token.clone(),
            room: cfg.room.clone(),
            txn_prefix: format!("avatar{started}"),
            txn: AtomicU64::new(0),
        }
    }

    fn api(&self, segments: &[&str]) -> Url {
        let mut all = vec!["_matrix", "client", "v3"];
        all.extend_from_slice(segments);
        join(&self.base, &all)
    }

    /// Refuse an end-to-end encrypted room: this adapter speaks the
    /// plain client-server API and would see only ciphertext.
    pub async fn check_unencrypted(&self) -> Result<(), Error> {
        let url = self.api(&["rooms", &self.room, "state", "m.room.encryption", ""]);
        let resp = self.http.get(url).bearer_auth(&self.token).send().await;
        let resp = resp.map_err(|e| Error::Retry(format!("room state: {e}")))?;
        match resp.status() {
            StatusCode::NOT_FOUND => Ok(()),
            s if s.is_success() => Err(Error::Fatal(format!(
                "room {} is end-to-end encrypted; this adapter needs an unencrypted room",
                self.room
            ))),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(Error::Fatal(format!(
                "room state: HTTP {} (bad token, or not joined to {})",
                resp.status(),
                self.room
            ))),
            s => Err(Error::Retry(format!("room state: HTTP {s}"))),
        }
    }

    /// One `/sync`, filtered to the room's messages and typing.
    pub async fn sync(&self, since: Option<&str>, timeout_ms: u64) -> Result<Value, Error> {
        let filter = json!({
            "presence": {"types": []},
            "account_data": {"types": []},
            "room": {
                "rooms": [self.room],
                "timeline": {"types": ["m.room.message"], "limit": 20},
                "ephemeral": {"types": ["m.typing"]},
                "state": {"types": []},
                "account_data": {"types": []}
            }
        })
        .to_string();
        let mut url = self.api(&["sync"]);
        url.query_pairs_mut()
            .append_pair("filter", &filter)
            .append_pair("timeout", &timeout_ms.to_string());
        if let Some(s) = since {
            url.query_pairs_mut().append_pair("since", s);
        }
        let req = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .timeout(Duration::from_millis(timeout_ms) + Duration::from_secs(30));
        call("sync", req)
            .await?
            .json()
            .await
            .map_err(|e| Error::Retry(format!("sync: {e}")))
    }

    /// Post `text` to the room as the adapter's user.
    pub async fn send_text(&self, text: &str) -> Result<(), Error> {
        let n = self.txn.fetch_add(1, Ordering::Relaxed);
        let txn = format!("{}-{n}", self.txn_prefix);
        let url = self.api(&["rooms", &self.room, "send", "m.room.message", &txn]);
        let body = json!({"msgtype": "m.text", "body": text});
        let req = self.http.put(url).bearer_auth(&self.token).json(&body);
        call("send", req).await.map(|_| ())
    }
}

pub struct Avatar {
    http: Client,
    base: Url,
    token: Option<String>,
}

impl Avatar {
    pub fn new(http: Client, cfg: &Config) -> Self {
        Avatar {
            http,
            base: base_url(&cfg.avatar_url),
            token: cfg.avatar_token.clone(),
        }
    }

    fn auth(&self, req: RequestBuilder) -> RequestBuilder {
        match &self.token {
            Some(t) => req.bearer_auth(t),
            None => req,
        }
    }

    pub async fn say(&self, text: &str) -> Result<(), Error> {
        let req = self.http.post(join(&self.base, &["say"]));
        call("say", self.auth(req).json(&json!({"text": text})))
            .await
            .map(|_| ())
    }

    pub async fn sprite(&self, state: &str) -> Result<(), Error> {
        let req = self.http.post(join(&self.base, &["sprite"]));
        call("sprite", self.auth(req).json(&json!({"state": state})))
            .await
            .map(|_| ())
    }

    /// Open the `/heard/events` stream.
    pub async fn heard(&self) -> Result<Response, Error> {
        let req = self.http.get(join(&self.base, &["heard", "events"]));
        call("heard stream", self.auth(req)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_encodes_each_segment() {
        let base = base_url("http://hs.example:8008/prefix/");
        let u = join(
            &base,
            &["rooms", "!a/b:example.org", "send", "m.room.message", "t 1"],
        );
        assert_eq!(
            u.as_str(),
            "http://hs.example:8008/prefix/rooms/!a%2Fb:example.org/send/m.room.message/t%201"
        );
        let state = join(
            &base_url("http://hs.example"),
            &["state", "m.room.encryption", ""],
        );
        assert_eq!(state.as_str(), "http://hs.example/state/m.room.encryption/");
    }
}
