//! agent-avatar server: the display server behind the kiosk page.
//!
//! Agents call show / sprite / say over HTTP (docs/API.md); the kiosk
//! page in `app/` (compiled into the binary) follows over SSE.
//!
//! Binds loopback by default; `tailscale serve` (or any reverse proxy)
//! is the network boundary. Environment: `AVATAR_ADDR` (address),
//! `AVATAR_SCREENS` (screens config), `AVATAR_TOKEN_FILE` or
//! `AVATAR_TOKEN` (command token).

mod config;
mod routes;

const DEFAULT_ADDR: &str = "127.0.0.1:8075";

fn die(msg: impl std::fmt::Display) -> ! {
    eprintln!("agent-avatar: {msg}");
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let screens = config::load_screens().unwrap_or_else(|e| die(e));
    let token = config::load_token().unwrap_or_else(|e| die(e));
    if token.is_none() {
        tracing::warn!(
            "no AVATAR_TOKEN_FILE: anything that can reach the port can drive the display"
        );
    }
    tracing::info!(screens = screens.len(), "screens loaded");
    let state = routes::AppState::new(screens, token);
    let addr = std::env::var("AVATAR_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => die(format!("bind {addr}: {e}")),
    };
    tracing::info!("server up: agent-avatar listening on http://{addr}");
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = axum::serve(listener, routes::router(state))
        .with_graceful_shutdown(shutdown)
        .await
    {
        die(e);
    }
}
