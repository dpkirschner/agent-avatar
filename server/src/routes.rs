//! HTTP routes: agent commands, the page's event stream, what the page
//! heard, and the kiosk page itself.
//!
//! State is small and in memory: the current screen, the sprite state
//! and the last line said. Every change goes to all connected pages as
//! an SSE event; a page that connects first gets the current screen and
//! sprite state, so a reload or a reconnect shows the right thing.

// Handlers return a ready `Response` as the error; boxing it buys nothing.
#![allow(clippy::result_large_err)]

use crate::config::Screen;
use axum::extract::{Json, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{Stream, StreamExt};

/// Longest line `say` accepts, in characters.
pub const MAX_SAY_CHARS: usize = 2000;
/// Longest utterance `heard` accepts, in characters.
pub const MAX_HEARD_CHARS: usize = 1000;
/// How soon a page's EventSource reconnects after a drop.
const SSE_RETRY: Duration = Duration::from_secs(2);

const INDEX_HTML: &str = include_str!("../../app/index.html");
const APP_JS: &str = include_str!("../../app/app.js");
const APP_CSS: &str = include_str!("../../app/app.css");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sprite {
    Idle,
    Listening,
    Thinking,
    Talking,
}

#[derive(Debug, Clone, Serialize)]
pub struct Said {
    pub id: u64,
    pub text: String,
    /// URL of pre-rendered speech audio; `None` means the page speaks
    /// the text with the browser's own voice.
    pub audio: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Shown {
    /// `None`: no screen, the avatar alone.
    pub screen: Option<String>,
    pub title: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Heard {
    pub text: String,
}

/// One event for the kiosk page.
#[derive(Debug, Clone)]
enum PageEvent {
    Show(Shown),
    Sprite(Sprite),
    Say(Said),
}

impl PageEvent {
    fn to_sse(&self) -> Event {
        let (name, data) = match self {
            PageEvent::Show(s) => ("show", serde_json::to_string(s)),
            PageEvent::Sprite(s) => ("sprite", serde_json::to_string(&SpriteBody { state: *s })),
            PageEvent::Say(s) => ("say", serde_json::to_string(s)),
        };
        Event::default()
            .event(name)
            .data(data.expect("event serializes"))
    }
}

#[derive(Debug)]
struct Current {
    screen: Option<String>,
    sprite: Sprite,
    last_say: Option<Said>,
    next_say_id: u64,
}

pub struct AppState {
    screens: Vec<Screen>,
    token: Option<String>,
    current: Mutex<Current>,
    page_tx: broadcast::Sender<PageEvent>,
    heard_tx: broadcast::Sender<Heard>,
    /// When the last page stream ended, for `/status`.
    page_left: Mutex<Option<Instant>>,
}

impl AppState {
    pub fn new(screens: Vec<Screen>, token: Option<String>) -> Arc<Self> {
        let (page_tx, _) = broadcast::channel(64);
        let (heard_tx, _) = broadcast::channel(64);
        Arc::new(AppState {
            screens,
            token,
            current: Mutex::new(Current {
                screen: None,
                sprite: Sprite::Idle,
                last_say: None,
                next_say_id: 1,
            }),
            page_tx,
            heard_tx,
            page_left: Mutex::new(None),
        })
    }

    fn shown(&self, screen: Option<&str>) -> Shown {
        let s = screen.and_then(|n| self.screens.iter().find(|s| s.name == n));
        Shown {
            screen: s.map(|s| s.name.clone()),
            title: s.map(|s| s.title.clone()),
            url: s.map(|s| s.url.clone()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Current> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Send to every page; no pages connected is fine.
    fn emit(&self, ev: PageEvent) {
        let _ = self.page_tx.send(ev);
    }

    /// Set the sprite state and tell the pages, if it changed.
    fn set_sprite(&self, cur: &mut Current, state: Sprite) {
        if cur.sprite != state {
            cur.sprite = state;
            self.emit(PageEvent::Sprite(state));
        }
    }
}

type Shared = Arc<AppState>;

pub fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(index))
        .route(
            "/app.js",
            get(|| async { asset(APP_JS, "text/javascript") }),
        )
        .route("/app.css", get(|| async { asset(APP_CSS, "text/css") }))
        .route("/healthz", get(|| async { "ok" }))
        .route("/version", get(version))
        .route("/status", get(status))
        .route("/state", get(get_state))
        .route("/screens", get(get_screens))
        .route("/events", get(events))
        .merge(commands(state.clone()))
        .with_state(state)
}

/// The routes that act: the agent's commands, and `heard`, whose text
/// reaches the agent as if the owner said it. The token layer runs before
/// any extractor, so a caller without the token gets 401 before its body is
/// read. The page sends the token too (set once in its settings).
fn commands(state: Shared) -> Router<Shared> {
    Router::new()
        .route("/show", post(show))
        .route("/sprite", post(sprite))
        .route("/say", post(say))
        .route("/heard", post(heard))
        .route("/heard/events", get(heard_events))
        .route_layer(middleware::from_fn_with_state(state, require_token))
}

async fn require_token(State(st): State<Shared>, req: Request, next: Next) -> Response {
    match authorized(&st, req.headers()) {
        Ok(()) => next.run(req).await,
        Err(r) => r,
    }
}

// ---------- page ----------

fn asset(body: &'static str, content_type: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        body,
    )
        .into_response()
}

/// The kiosk page, with a CSP that frames only the configured screens'
/// origins and lets nothing frame the page itself.
async fn index(State(st): State<Shared>) -> Response {
    let mut res = asset(INDEX_HTML, "text/html; charset=utf-8");
    let csp = page_csp(&st.screens);
    if let Ok(v) = HeaderValue::from_str(&csp) {
        res.headers_mut().insert(header::CONTENT_SECURITY_POLICY, v);
    }
    res
}

fn page_csp(screens: &[Screen]) -> String {
    let mut origins: Vec<String> = screens.iter().filter_map(|s| origin(&s.url)).collect();
    origins.sort();
    origins.dedup();
    let frames = if origins.is_empty() {
        "'none'".to_string()
    } else {
        origins.join(" ")
    };
    format!(
        "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
         media-src 'self' data: blob:; connect-src 'self'; frame-src {frames}; \
         frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
    )
}

/// `scheme://host[:port]` of an http(s) URL; `None` for anything that
/// would be unsafe to put in a CSP.
fn origin(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    let ok = !host.is_empty()
        && host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-:[]".contains(&b));
    ok.then(|| format!("{scheme}://{host}"))
}

// ---------- reads ----------

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "version": option_env!("AVATAR_VERSION").unwrap_or(env!("CARGO_PKG_VERSION")),
    }))
}

#[derive(Serialize)]
struct StateBody {
    screen: Shown,
    sprite: Sprite,
    last_say: Option<Said>,
}

async fn get_state(State(st): State<Shared>) -> Json<StateBody> {
    let cur = st.lock();
    Json(StateBody {
        screen: st.shown(cur.screen.as_deref()),
        sprite: cur.sprite,
        last_say: cur.last_say.clone(),
    })
}

#[derive(Serialize)]
struct ScreenInfo<'a> {
    name: &'a str,
    title: &'a str,
}

/// Names and titles only: an agent picks screens by name, so it never
/// needs (or sees) the URLs.
async fn get_screens(State(st): State<Shared>) -> Response {
    let list: Vec<ScreenInfo> = st
        .screens
        .iter()
        .map(|s| ScreenInfo {
            name: &s.name,
            title: &s.title,
        })
        .collect();
    Json(list).into_response()
}

/// What a monitor needs: pages and adapters connected now, and seconds
/// since a page was last connected (0 while one is; null if none ever
/// was). Counts only, no content, so it's open like `/healthz`.
async fn status(State(st): State<Shared>) -> Json<serde_json::Value> {
    let pages = st.page_tx.receiver_count();
    let page_seen_secs = if pages > 0 {
        Some(0)
    } else {
        st.page_left
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .map(|t| t.elapsed().as_secs())
    };
    Json(serde_json::json!({
        "pages": pages,
        "page_seen_secs": page_seen_secs,
        "adapters": st.heard_tx.receiver_count(),
    }))
}

/// Stamps `page_left` when a page's stream is dropped.
struct PageLeft(Shared);

impl Drop for PageLeft {
    fn drop(&mut self) {
        *self.0.page_left.lock().unwrap_or_else(|e| e.into_inner()) = Some(Instant::now());
    }
}

/// The page's event stream: the current screen and sprite state first,
/// then every change. A `say` is not replayed, so a reconnect never
/// repeats a line.
async fn events(State(st): State<Shared>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (rx, first) = {
        let cur = st.lock();
        // Subscribe under the lock so no change slips between the
        // snapshot and the stream.
        let rx = st.page_tx.subscribe();
        let first = vec![
            PageEvent::Show(st.shown(cur.screen.as_deref())).to_sse(),
            PageEvent::Sprite(cur.sprite).to_sse().retry(SSE_RETRY),
        ];
        (rx, first)
    };
    let left = PageLeft(st.clone());
    let live = BroadcastStream::new(rx).filter_map(|r| r.ok().map(|e| e.to_sse()));
    let stream = tokio_stream::iter(first).chain(live).map(move |e| {
        let _ = &left;
        Ok(e)
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
}

// ---------- commands ----------

fn err(status: StatusCode, msg: impl Into<String>) -> Response {
    (status, Json(serde_json::json!({ "error": msg.into() }))).into_response()
}

/// Commands need `Authorization: Bearer <token>` when a token is set.
fn authorized(st: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let Some(want) = &st.token else {
        return Ok(());
    };
    let got = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    if constant_time_eq(got.as_bytes(), want.as_bytes()) {
        Ok(())
    } else {
        let mut res = err(StatusCode::UNAUTHORIZED, "missing or wrong bearer token");
        res.headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        Err(res)
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShowBody {
    screen: Option<String>,
}

async fn show(State(st): State<Shared>, Json(b): Json<ShowBody>) -> Response {
    if let Some(name) = &b.screen {
        if !st.screens.iter().any(|s| &s.name == name) {
            return err(StatusCode::NOT_FOUND, format!("no screen named {name:?}"));
        }
    }
    let mut cur = st.lock();
    cur.screen = b.screen;
    let shown = st.shown(cur.screen.as_deref());
    st.emit(PageEvent::Show(shown.clone()));
    Json(shown).into_response()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpriteBody {
    state: Sprite,
}

async fn sprite(State(st): State<Shared>, Json(b): Json<SpriteBody>) -> Response {
    let mut cur = st.lock();
    st.set_sprite(&mut cur, b.state);
    Json(SpriteBody { state: cur.sprite }).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TextBody {
    text: String,
}

fn check_text(text: &str, max: usize) -> Result<String, Response> {
    let t = text.trim();
    if t.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "text is empty"));
    }
    if t.chars().count() > max {
        return Err(err(
            StatusCode::BAD_REQUEST,
            format!("text is longer than {max} characters"),
        ));
    }
    Ok(t.to_string())
}

/// Speak a line. The page shows `talking` while it plays; afterwards the
/// avatar is `idle`, so the sprite state is set to idle here.
async fn say(State(st): State<Shared>, Json(b): Json<TextBody>) -> Response {
    let text = match check_text(&b.text, MAX_SAY_CHARS) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let mut cur = st.lock();
    let said = Said {
        id: cur.next_say_id,
        text,
        audio: None,
    };
    cur.next_say_id += 1;
    cur.last_say = Some(said.clone());
    st.emit(PageEvent::Say(said.clone()));
    st.set_sprite(&mut cur, Sprite::Idle);
    Json(said).into_response()
}

/// What the user said on the page. Needs the token: it goes to adapters
/// on `/heard/events`, and the avatar turns `thinking`.
async fn heard(State(st): State<Shared>, Json(b): Json<TextBody>) -> Response {
    let text = match check_text(&b.text, MAX_HEARD_CHARS) {
        Ok(t) => t,
        Err(r) => return r,
    };
    let listeners = st.heard_tx.send(Heard { text }).unwrap_or(0);
    let mut cur = st.lock();
    if listeners > 0 {
        st.set_sprite(&mut cur, Sprite::Thinking);
    }
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "delivered_to": listeners })),
    )
        .into_response()
}

/// The adapters' stream of what the user said. Needs the token when one
/// is set: it carries the user's words.
async fn heard_events(State(st): State<Shared>) -> Response {
    let rx = st.heard_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|r| {
        r.ok().map(|h| {
            Ok::<_, Infallible>(
                Event::default()
                    .event("heard")
                    .data(serde_json::to_string(&h).expect("heard serializes")),
            )
        })
    });
    Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(20)))
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn screens() -> Vec<Screen> {
        crate::config::parse_screens(include_str!("../../examples/screens.toml")).unwrap()
    }

    fn app(token: Option<&str>) -> (Router, Shared) {
        let st = AppState::new(screens(), token.map(String::from));
        (router(st.clone()), st)
    }

    fn post_json(path: &str, body: &str, token: Option<&str>) -> Request<Body> {
        let mut r = Request::post(path).header("content-type", "application/json");
        if let Some(t) = token {
            r = r.header("authorization", format!("Bearer {t}"));
        }
        r.body(Body::from(body.to_string())).unwrap()
    }

    async fn send(app: &Router, req: Request<Body>) -> (StatusCode, String) {
        let res = app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    async fn get(app: &Router, path: &str) -> (StatusCode, String) {
        send(app, Request::get(path).body(Body::empty()).unwrap()).await
    }

    #[tokio::test]
    async fn healthz_answers_ok() {
        let (app, _) = app(None);
        assert_eq!(get(&app, "/healthz").await, (StatusCode::OK, "ok".into()));
    }

    #[tokio::test]
    async fn status_counts_pages_and_adapters() {
        let (app, st) = app(None);
        let v = |s: String| serde_json::from_str::<serde_json::Value>(&s).unwrap();
        let (code, body) = get(&app, "/status").await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(
            v(body),
            serde_json::json!({"pages": 0, "page_seen_secs": null, "adapters": 0})
        );

        let page = events(State(st.clone())).await;
        let adapter = st.heard_tx.subscribe();
        let (_, body) = get(&app, "/status").await;
        assert_eq!(
            v(body),
            serde_json::json!({"pages": 1, "page_seen_secs": 0, "adapters": 1})
        );

        drop(page);
        drop(adapter);
        let (_, body) = get(&app, "/status").await;
        let body = v(body);
        assert_eq!(body["pages"], 0);
        assert_eq!(body["adapters"], 0);
        assert_eq!(body["page_seen_secs"], 0, "a page just left");
    }

    #[tokio::test]
    async fn serves_the_page_with_a_csp_for_screen_origins() {
        let (app, _) = app(None);
        let res = app
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let csp = res.headers()[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .to_string();
        assert!(csp.contains("frame-src https://status.example.com https://weather.example.com;"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }

    #[test]
    fn origin_keeps_scheme_host_port_only() {
        assert_eq!(
            origin("https://a.example:8443/x?y#z").as_deref(),
            Some("https://a.example:8443")
        );
        assert_eq!(origin("https://a b/"), None);
        assert_eq!(origin("https://a;evil/"), None);
    }

    #[tokio::test]
    async fn show_known_screen_updates_state() {
        let (app, _) = app(None);
        let (s, body) = send(&app, post_json("/show", r#"{"screen":"status"}"#, None)).await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains(r#""url":"https://status.example.com/""#));
        let (_, state) = get(&app, "/state").await;
        assert!(state.contains(r#""screen":"status""#));
    }

    #[tokio::test]
    async fn show_unknown_screen_is_404_and_null_clears() {
        let (app, _) = app(None);
        let (s, _) = send(&app, post_json("/show", r#"{"screen":"nope"}"#, None)).await;
        assert_eq!(s, StatusCode::NOT_FOUND);
        let (s, body) = send(&app, post_json("/show", r#"{"screen":null}"#, None)).await;
        assert_eq!(s, StatusCode::OK);
        assert!(body.contains(r#""screen":null"#));
    }

    #[tokio::test]
    async fn sprite_rejects_unknown_states() {
        let (app, _) = app(None);
        let (s, _) = send(&app, post_json("/sprite", r#"{"state":"thinking"}"#, None)).await;
        assert_eq!(s, StatusCode::OK);
        let (s, _) = send(&app, post_json("/sprite", r#"{"state":"dancing"}"#, None)).await;
        assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn say_numbers_lines_and_validates_text() {
        let (app, _) = app(None);
        let (_, a) = send(&app, post_json("/say", r#"{"text":" Hello. "}"#, None)).await;
        assert!(a.contains(r#""id":1"#) && a.contains(r#""text":"Hello.""#));
        let (_, b) = send(&app, post_json("/say", r#"{"text":"Again."}"#, None)).await;
        assert!(b.contains(r#""id":2"#));
        let (s, _) = send(&app, post_json("/say", r#"{"text":"  "}"#, None)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
        let long = format!(r#"{{"text":"{}"}}"#, "a".repeat(MAX_SAY_CHARS + 1));
        let (s, _) = send(&app, post_json("/say", &long, None)).await;
        assert_eq!(s, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn token_guards_commands_and_heard_not_page() {
        let (app, _) = app(Some("s3cret"));
        for (path, body, ok) in [
            ("/show", r#"{"screen":"status"}"#, StatusCode::OK),
            ("/sprite", r#"{"state":"idle"}"#, StatusCode::OK),
            ("/say", r#"{"text":"hi"}"#, StatusCode::OK),
            ("/heard", r#"{"text":"hi"}"#, StatusCode::ACCEPTED),
        ] {
            let (s, _) = send(&app, post_json(path, body, None)).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{path} without token");
            let (s, _) = send(&app, post_json(path, body, Some("wrong"))).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{path} wrong token");
            let (s, _) = send(&app, post_json(path, body, Some("s3cret"))).await;
            assert_eq!(s, ok, "{path} right token");
        }
        let (s, _) = get(&app, "/heard/events").await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
        // The token is checked before the body is read: a bare POST
        // with no content type or body is 401, not 415.
        for path in ["/show", "/sprite", "/say", "/heard"] {
            let req = Request::post(path).body(Body::empty()).unwrap();
            let (s, _) = send(&app, req).await;
            assert_eq!(s, StatusCode::UNAUTHORIZED, "{path} bare POST");
        }
        // The page's own routes stay open.
        for path in ["/", "/app.js", "/state", "/screens", "/healthz", "/status"] {
            assert_eq!(get(&app, path).await.0, StatusCode::OK, "{path}");
        }
    }

    #[tokio::test]
    async fn screens_lists_names_and_titles_without_urls() {
        let (app, _) = app(None);
        let (_, body) = get(&app, "/screens").await;
        assert!(body.contains(r#""name":"status""#));
        assert!(!body.contains("https://"));
    }

    #[tokio::test]
    async fn heard_reaches_adapters_and_sets_thinking() {
        let (app, st) = app(None);
        let mut rx = st.heard_tx.subscribe();
        let mut pages = st.page_tx.subscribe();
        let (s, body) = send(&app, post_json("/heard", r#"{"text":"show status"}"#, None)).await;
        assert_eq!(s, StatusCode::ACCEPTED);
        assert!(body.contains(r#""delivered_to":1"#));
        assert_eq!(rx.recv().await.unwrap().text, "show status");
        assert!(matches!(
            pages.recv().await.unwrap(),
            PageEvent::Sprite(Sprite::Thinking)
        ));
    }

    #[tokio::test]
    async fn heard_with_no_adapter_leaves_sprite_alone() {
        let (app, st) = app(None);
        let (_, body) = send(&app, post_json("/heard", r#"{"text":"hello"}"#, None)).await;
        assert!(body.contains(r#""delivered_to":0"#));
        assert_eq!(st.lock().sprite, Sprite::Idle);
    }

    #[tokio::test]
    async fn say_goes_to_pages_then_sprite_idle() {
        let (app, st) = app(None);
        st.lock().sprite = Sprite::Thinking;
        let mut pages = st.page_tx.subscribe();
        send(&app, post_json("/say", r#"{"text":"Done."}"#, None)).await;
        assert!(matches!(pages.recv().await.unwrap(), PageEvent::Say(s) if s.text == "Done."));
        assert!(matches!(
            pages.recv().await.unwrap(),
            PageEvent::Sprite(Sprite::Idle)
        ));
    }

    #[tokio::test]
    async fn event_stream_starts_with_current_state() {
        let (app, _) = app(None);
        send(&app, post_json("/show", r#"{"screen":"weather"}"#, None)).await;
        let res = app
            .oneshot(Request::get("/events").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/event-stream");
        let mut body = res.into_body();
        let mut text = String::new();
        while !text.contains("event: sprite") {
            let frame = body.frame().await.unwrap().unwrap();
            text.push_str(std::str::from_utf8(frame.data_ref().unwrap()).unwrap());
        }
        assert!(text.contains("event: show\ndata: {\"screen\":\"weather\""));
        assert!(text.contains("retry: 2000"));
    }
}
