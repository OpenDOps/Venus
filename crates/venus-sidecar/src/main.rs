//! Venus snapshotter binary. Compose service `sidecar`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use venus_sidecar::config::Config;
use venus_sidecar::http::router;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cfg = Config::from_env()?;
    tracing::info!(
        listen = %cfg.listen,
        convert_cli = %cfg.convert.cli.display(),
        convert_cwd = %cfg.convert.cwd.display(),
        "venus-sidecar starting"
    );

    let listener = TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;
    tracing::info!("listening on {}", cfg.listen);

    axum::serve(listener, router())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve")
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
    tracing::info!("signal received");
}
