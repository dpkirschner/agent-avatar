//! agent-avatar server: the display server behind the kiosk page.
//!
//! Scaffold only: `/healthz` answers so the gate and a health check have
//! something to hit. The show / sprite / say API and the SSE stream are
//! described in docs/API.md and land next.
//!
//! Binds loopback by default; `tailscale serve` (or any reverse proxy)
//! is the network boundary. `AVATAR_ADDR` overrides the address.

use axum::routing::get;
use axum::Router;

const DEFAULT_ADDR: &str = "127.0.0.1:8070";

fn app() -> Router {
    Router::new().route("/healthz", get(|| async { "ok" }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let addr = std::env::var("AVATAR_ADDR").unwrap_or_else(|_| DEFAULT_ADDR.to_string());
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("agent-avatar: bind {addr}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("agent-avatar listening on http://{addr}");
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    if let Err(e) = axum::serve(listener, app())
        .with_graceful_shutdown(shutdown)
        .await
    {
        eprintln!("agent-avatar: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_answers_ok() {
        let res = app()
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
