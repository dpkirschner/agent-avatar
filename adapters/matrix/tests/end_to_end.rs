//! The adapter against a fake homeserver and a fake display server
//! (synthetic data): speech goes into the room, the agent's settled
//! reply comes back as one `say`, history and other senders are ignored.

use agent_avatar_matrix::config::Config;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use futures_util::stream::{self, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const ROOM: &str = "!room:example.org";
const AGENT: &str = "@agent:example.org";
const AVATAR_TOKEN: &str = "avatar-test-token";

#[derive(Default)]
struct Seen {
    sent: Vec<String>,
    said: Vec<String>,
    avatars: Vec<String>,
}

type Shared = Arc<Mutex<Seen>>;

async fn serve(app: Router) -> String {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
    format!("http://{addr}")
}

fn msg(id: &str, sender: &str, content: Value) -> Value {
    json!({"type": "m.room.message", "event_id": id, "sender": sender, "content": content})
}

fn room_page(next: &str, timeline: Value, typing: Value) -> Value {
    json!({"next_batch": next, "rooms": {"join": {ROOM: {
        "timeline": {"events": timeline},
        "ephemeral": {"events": [{"type": "m.typing", "content": {"user_ids": typing}}]}
    }}}})
}

async fn sync(State(seen): State<Shared>, Query(q): Query<HashMap<String, String>>) -> Json<Value> {
    match q.get("since").map(String::as_str) {
        // The starting point: history, which must not be spoken.
        None => Json(room_page(
            "s1",
            json!([msg(
                "$old",
                AGENT,
                json!({"msgtype": "m.text", "body": "old news"})
            )]),
            json!([]),
        )),
        Some("s1") => {
            // Answer once the adapter has posted what the user said.
            for _ in 0..100 {
                if !seen.lock().unwrap().sent.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Json(room_page(
                "s2",
                json!([
                    msg(
                        "$me",
                        "@tablet:example.org",
                        json!({"msgtype": "m.text", "body": "what's new"})
                    ),
                    msg("$1", AGENT, json!({"msgtype": "m.text", "body": "Two"})),
                    msg(
                        "$2",
                        AGENT,
                        json!({"msgtype": "m.text", "body": "* Two new **mail**.",
                        "m.new_content": {"msgtype": "m.text", "body": "Two new **mail**."},
                        "m.relates_to": {"rel_type": "m.replace", "event_id": "$1"}})
                    )
                ]),
                json!([AGENT]),
            ))
        }
        Some(_) => {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Json(json!({"next_batch": "s2"}))
        }
    }
}

async fn send(
    State(seen): State<Shared>,
    Path((_room, _txn)): Path<(String, String)>,
    Json(b): Json<Value>,
) -> Json<Value> {
    seen.lock()
        .unwrap()
        .sent
        .push(b["body"].as_str().unwrap().to_string());
    Json(json!({"event_id": "$sent"}))
}

fn homeserver(seen: Shared, encrypted: bool) -> Router {
    let state_route = move || async move {
        if encrypted {
            (
                StatusCode::OK,
                Json(json!({"algorithm": "m.megolm.v1.aes-sha2"})),
            )
        } else {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"errcode": "M_NOT_FOUND"})),
            )
        }
    };
    Router::new()
        .route(
            "/_matrix/client/v3/rooms/{room}/state/m.room.encryption/",
            get(state_route),
        )
        .route("/_matrix/client/v3/sync", get(sync))
        .route(
            "/_matrix/client/v3/rooms/{room}/send/m.room.message/{txn}",
            put(send),
        )
        .with_state(seen)
}

fn authed(h: &HeaderMap) -> bool {
    h.get("authorization").and_then(|v| v.to_str().ok()) == Some(&format!("Bearer {AVATAR_TOKEN}"))
}

fn display(seen: Shared) -> Router {
    let heard = |h: HeaderMap| async move {
        if !authed(&h) {
            return StatusCode::UNAUTHORIZED.into_response();
        }
        let first = stream::once(async {
            Ok::<_, Infallible>(
                Event::default()
                    .event("heard")
                    .data(r#"{"text":"what's new"}"#),
            )
        });
        Sse::new(first.chain(stream::pending())).into_response()
    };
    let say = |State(s): State<Shared>, h: HeaderMap, Json(b): Json<Value>| async move {
        assert!(authed(&h));
        s.lock()
            .unwrap()
            .said
            .push(b["text"].as_str().unwrap().to_string());
        StatusCode::OK
    };
    let display = |State(s): State<Shared>, h: HeaderMap, Json(b): Json<Value>| async move {
        assert!(authed(&h));
        s.lock()
            .unwrap()
            .avatars
            .push(b["avatar"].as_str().unwrap().to_string());
        StatusCode::OK
    };
    Router::new()
        .route("/heard/events", get(heard))
        .route("/say", post(say))
        .route("/display", post(display))
        .with_state(seen)
}

fn config(homeserver: String, avatar_url: String) -> Config {
    Config {
        avatar_url,
        avatar_token: Some(AVATAR_TOKEN.into()),
        homeserver,
        user: "@tablet:example.org".into(),
        room: ROOM.into(),
        agent_user: AGENT.into(),
        matrix_token: "matrix-test-token".into(),
        settle: Duration::from_millis(100),
    }
}

#[tokio::test]
async fn speech_goes_in_and_the_settled_reply_comes_out_once() {
    let seen = Shared::default();
    let hs = serve(homeserver(seen.clone(), false)).await;
    let avatar = serve(display(seen.clone())).await;
    let adapter = tokio::spawn(agent_avatar_matrix::run(config(hs, avatar)));

    for _ in 0..150 {
        if !seen.lock().unwrap().said.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    // Long enough for a duplicate to show up if there were one.
    tokio::time::sleep(Duration::from_millis(400)).await;
    adapter.abort();

    let seen = seen.lock().unwrap();
    assert_eq!(seen.sent, vec!["what's new"]);
    assert_eq!(seen.said, vec!["Two new mail."]);
    assert_eq!(seen.avatars, vec!["thinking"]);
}

#[tokio::test]
async fn refuses_an_encrypted_room() {
    let seen = Shared::default();
    let hs = serve(homeserver(seen.clone(), true)).await;
    let avatar = serve(display(seen)).await;
    let err = agent_avatar_matrix::run(config(hs, avatar))
        .await
        .unwrap_err();
    assert!(err.contains("encrypted"), "{err}");
}
