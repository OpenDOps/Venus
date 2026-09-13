//! Venus hub binary. Compose service `hub`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{Context, Result};
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;
use venus_hub::config::Config;
use venus_hub::db;
use venus_hub::http::{router, AppState};
use venus_hub::lease::Lease;
use venus_hub::room::Hub;

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
        owner = %cfg.owner,
        pg_sslmode = %cfg.pg_sslmode,
        lease_ttl_secs = cfg.lease_ttl.as_secs(),
        heartbeat_interval_secs = cfg.heartbeat_interval.as_secs(),
        persist_interval_ms = cfg.persist_interval.as_millis() as u64,
        compact_after = cfg.compact_after,
        db_max_connections = cfg.db_max_connections,
        db_min_connections = cfg.db_min_connections,
        db_acquire_timeout_secs = cfg.db_acquire_timeout.as_secs(),
        cors_origins = cfg.cors_origins.len(),
        "venus-hub starting"
    );

    let pool = db::connect_with(
        &cfg.database_url,
        &db::PoolSettings {
            max_connections: cfg.db_max_connections,
            min_connections: cfg.db_min_connections,
            acquire_timeout: cfg.db_acquire_timeout,
        },
    )
    .await?;
    db::migrate(&pool).await?;

    let lease = Lease::new(pool.clone(), cfg.owner.clone(), cfg.lease_ttl);
    let hub = Hub::new(pool, lease.clone(), cfg.persist_interval, cfg.compact_after);

    let listener = TcpListener::bind(cfg.listen)
        .await
        .with_context(|| format!("bind {}", cfg.listen))?;
    tracing::info!("listening on {}", cfg.listen);

    let hb_hub = hub.clone();
    let hb_lease = lease.clone();
    let heartbeat_interval = cfg.heartbeat_interval;
    let heartbeat = tokio::spawn(async move {
        let mut tick = tokio::time::interval(heartbeat_interval);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let ids = hb_hub.workspace_ids().await;
            let missed = hb_lease.heartbeat_many(&ids).await;
            if !missed.is_empty() {
                tracing::warn!(n = missed.len(), "lease heartbeat misses");
            }
        }
    });

    let app = router(AppState::with_cors(hub.clone(), cfg.cors_origins));
    let serve_result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("serve");
    hub.shutdown_with_heartbeat(heartbeat).await;
    tracing::info!("hub drained");
    serve_result
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
    tracing::info!("signal received; flushing persist then dropping leases");
}
