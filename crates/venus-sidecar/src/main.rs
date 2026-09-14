//! Venus snapshotter binary. Compose service `sidecar`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use venus_sidecar::config::Config;

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
        workers = cfg.queue.workers,
        has_dsn = cfg.database_url.is_some(),
        "venus-sidecar starting"
    );

    let app = if let Some(url) = cfg.database_url.clone() {
        let pool = venus_sidecar::db::connect(&url).await?;
        let idle = cfg.queue.idle;
        let period = cfg.queue.observe;
        let pool_obs = pool.clone();
        tokio::spawn(async move {
            venus_sidecar::queue::run_observer(pool_obs, idle, period).await;
        });
        for i in 0..cfg.queue.workers {
            let pool_w = pool.clone();
            let owner = format!("{}-{}", cfg.owner, i);
            let convert_sleep = cfg.queue.convert_sleep;
            let wiki = cfg.wiki.clone();
            tokio::spawn(async move {
                venus_sidecar::queue::run_worker(pool_w, owner, convert_sleep, wiki).await;
            });
        }
        tracing::info!(
            workers = cfg.queue.workers,
            idle_ms = cfg.queue.idle.as_millis() as u64,
            observe_ms = cfg.queue.observe.as_millis() as u64,
            wiki_dir = %cfg.wiki.dir.display(),
            "observer + workers running"
        );
        venus_sidecar::http::router_with_wiki_and_cors(
            Some(pool),
            cfg.wiki.clone(),
            cfg.cors_origins.clone(),
        )
    } else {
        tracing::warn!("no DATABASE_URL / POSTGRES_HOST; observer off (HTTP only)");
        venus_sidecar::http::router_with_wiki_and_cors(
            None,
            cfg.wiki.clone(),
            cfg.cors_origins.clone(),
        )
    };

    let listener = TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;
    tracing::info!("listening on {}", cfg.listen);

    axum::serve(listener, app)
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
