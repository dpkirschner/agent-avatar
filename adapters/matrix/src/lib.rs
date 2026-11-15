//! agent-avatar's Matrix adapter.
//!
//! What the user says on the kiosk page arrives on the display server's
//! `/heard/events` stream and goes into a Matrix room as the adapter's
//! user. The agent's replies in that room (settled across edits) go back
//! to the display server as `say`; its typing turns the avatar
//! `thinking`. See docs/ARCHITECTURE.md § Adapters.

pub mod config;
pub mod http;
pub mod replies;
pub mod speech;
pub mod sse;
pub mod sync;

use config::Config;
use futures_util::StreamExt;
use http::{Avatar, Error, Homeserver, SYNC_TIMEOUT_MS};
use replies::Replies;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync::Incoming;
use tokio::sync::mpsc;

/// Replies older than this when they reach the adapter are not spoken.
const MAX_REPLY_AGE: Duration = Duration::from_secs(5 * 60);

/// What the tablet says when the agent's room can't be reached.
pub const UNREACHABLE: &str = "I couldn't reach the agent.";

/// Run the adapter until a fatal error (bad token, not in the room, an
/// encrypted room). Network trouble on either side is retried.
pub async fn run(cfg: Config) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        // The heard stream sends a keep-alive every 20 s; a /sync answers
        // within SYNC_TIMEOUT_MS.
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("http client: {e}"))?;
    let hs = Arc::new(Homeserver::new(http.clone(), &cfg));
    let avatar = Arc::new(Avatar::new(http, &cfg));

    let mut backoff = Backoff::new();
    loop {
        match hs.check_unencrypted().await {
            Ok(()) => break,
            Err(Error::Fatal(m)) => return Err(m),
            Err(Error::Retry(m)) => tracing::warn!("{m}; retrying"),
        }
        tokio::time::sleep(backoff.next()).await;
    }
    tracing::info!(room = %cfg.room, agent = %cfg.agent_user, "adapter up");

    let (tx, rx) = mpsc::channel(64);
    tokio::select! {
        r = heard_loop(avatar.clone(), hs.clone()) => r,
        r = sync_loop(hs, cfg.room.clone(), cfg.agent_user.clone(), tx) => r,
        r = speak_loop(avatar, rx, cfg.settle) => r,
    }
}

/// Carry each utterance from the display server into the room.
async fn heard_loop(avatar: Arc<Avatar>, hs: Arc<Homeserver>) -> Result<(), String> {
    let mut backoff = Backoff::new();
    loop {
        match avatar.heard().await {
            Ok(resp) => {
                tracing::info!("listening for speech");
                backoff.reset();
                let mut parser = sse::SseParser::new();
                let mut body = resp.bytes_stream();
                while let Some(chunk) = body.next().await {
                    let chunk = match chunk {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!("heard stream: {e}");
                            break;
                        }
                    };
                    for ev in parser.feed(&chunk) {
                        if ev.event == "heard" {
                            deliver(&avatar, &hs, &ev.data).await?;
                        }
                    }
                }
                tracing::warn!("heard stream ended; reconnecting");
            }
            Err(Error::Fatal(m)) => return Err(m),
            Err(Error::Retry(m)) => tracing::warn!("{m}; retrying"),
        }
        tokio::time::sleep(backoff.next()).await;
    }
}

async fn deliver(avatar: &Avatar, hs: &Homeserver, data: &str) -> Result<(), String> {
    let text = match serde_json::from_str::<serde_json::Value>(data) {
        Ok(v) => v["text"].as_str().unwrap_or_default().trim().to_string(),
        Err(e) => {
            tracing::warn!("heard event isn't JSON: {e}");
            return Ok(());
        }
    };
    if text.is_empty() {
        return Ok(());
    }
    match hs.send_text(&text).await {
        Ok(()) => Ok(()),
        Err(Error::Fatal(m)) => Err(m),
        Err(Error::Retry(m)) => {
            tracing::warn!("{m}; utterance dropped");
            // Tell the user rather than leave the avatar thinking.
            if let Err(e) = avatar.say(UNREACHABLE).await {
                tracing::warn!("{e}");
            }
            Ok(())
        }
    }
}

/// Long-poll the room and pass the agent's activity to the speaker. The
/// first sync only finds the starting point: history is never spoken.
async fn sync_loop(
    hs: Arc<Homeserver>,
    room: String,
    agent: String,
    tx: mpsc::Sender<Incoming>,
) -> Result<(), String> {
    let mut since: Option<String> = None;
    let mut backoff = Backoff::new();
    loop {
        let timeout = if since.is_some() { SYNC_TIMEOUT_MS } else { 0 };
        let failed = match hs.sync(since.as_deref(), timeout).await {
            Ok(resp) => {
                let min_ts = now_ms().saturating_sub(MAX_REPLY_AGE.as_millis() as u64);
                match sync::parse_sync(&resp, &room, &agent, min_ts) {
                    Ok(page) => {
                        if since.is_none() {
                            tracing::info!("caught up with the room");
                        } else {
                            for item in page.items {
                                tx.send(item).await.map_err(|_| "speaker stopped")?;
                            }
                        }
                        since = Some(page.next_batch);
                        backoff.reset();
                        None
                    }
                    Err(e) => Some(e),
                }
            }
            Err(Error::Fatal(m)) => return Err(m),
            Err(Error::Retry(m)) => Some(m),
        };
        if let Some(m) = failed {
            tracing::warn!("{m}; retrying");
            tokio::time::sleep(backoff.next()).await;
        }
    }
}

/// Settle replies and say them; follow the agent's typing on the avatar.
async fn speak_loop(
    avatar: Arc<Avatar>,
    mut rx: mpsc::Receiver<Incoming>,
    settle: Duration,
) -> Result<(), String> {
    let mut replies = Replies::new(settle);
    let mut typing = false;
    loop {
        let due = replies.next_due();
        let wait = async {
            match due {
                Some(d) => tokio::time::sleep_until(d.into()).await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            item = rx.recv() => {
                let Some(item) = item else { return Err("sync stopped".into()) };
                let now = Instant::now();
                match item {
                    Incoming::Message { event_id, body } => replies.message(&event_id, &body, now),
                    Incoming::Edit { target, body } => replies.edit(&target, &body, now),
                    Incoming::Typing(t) => {
                        let state = match (typing, t) {
                            (false, true) => Some("thinking"),
                            (true, false) if !replies.is_pending() => Some("idle"),
                            _ => None,
                        };
                        typing = t;
                        if let Some(s) = state {
                            report(avatar.avatar(s).await)?;
                        }
                    }
                }
            }
            () = wait => {
                for text in replies.take_due(Instant::now()) {
                    if let Some(line) = speech::for_speech(&text) {
                        report(avatar.say(&line).await)?;
                    }
                }
            }
        }
    }
}

/// A display-server call's result: fatal stops the adapter, anything
/// else is logged and the line or state change is dropped.
fn report(r: Result<(), Error>) -> Result<(), String> {
    match r {
        Ok(()) => Ok(()),
        Err(Error::Fatal(m)) => Err(m),
        Err(Error::Retry(m)) => {
            tracing::warn!("{m}");
            Ok(())
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Exponential backoff from 1 s up to 30 s.
struct Backoff(Duration);

impl Backoff {
    fn new() -> Self {
        Backoff(Duration::from_secs(1))
    }
    fn reset(&mut self) {
        self.0 = Duration::from_secs(1);
    }
    fn next(&mut self) -> Duration {
        let d = self.0;
        self.0 = (self.0 * 2).min(Duration::from_secs(30));
        d
    }
}
