//! agent-avatar-matrix: the Matrix adapter binary. Configuration is all
//! environment (see docs/ARCHITECTURE.md § Matrix adapter).

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let cfg = match agent_avatar_matrix::config::Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("agent-avatar-matrix: {e}");
            std::process::exit(1);
        }
    };
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let result = rt.block_on(async {
        tokio::select! {
            r = agent_avatar_matrix::run(cfg) => r,
            _ = tokio::signal::ctrl_c() => Ok(()),
        }
    });
    if let Err(e) = result {
        eprintln!("agent-avatar-matrix: {e}");
        std::process::exit(1);
    }
}
