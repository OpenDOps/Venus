//! Every default in scale.md, read from an environment variable of that name.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;
use std::time::Duration;

/// Lease length and renew interval have no scale.md identifier.
/// Tests override them here. The named defaults below use their scale.md names.
#[derive(Debug, Clone)]
pub struct Config {
    pub lease: Duration,
    pub renew: Duration,
    pub entry_docs: u32,
    pub entry_bytes: u32,
    pub lag_entries: u64,
    pub lag: Duration,
    pub catchup_batch: u32,
    pub log_retain: Duration,
    pub log_retain_entries: u64,
    pub probe: Duration,
    pub down: Duration,
    pub reallocate_delay: Duration,
    pub max_builds_per_node: u32,
    pub hedge_floor: Duration,
    pub shard_timeout: Duration,
    pub hydrate: Duration,
    pub p95_target: Duration,
    pub graph_copies: u32,
    pub archive_url: String,
    pub backup_at: String,
    pub surreal_user: String,
    pub surreal_pass: String,
    /// PEM file for `wss://` nodes. Empty uses the default web PKI roots.
    pub tls_ca: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            lease: Duration::from_millis(env_u64("REPL_LEASE_MS", 10_000)),
            renew: Duration::from_millis(env_u64("REPL_RENEW_MS", 3_000)),
            entry_docs: env_u64("REPL_ENTRY_DOCS", 256) as u32,
            entry_bytes: env_u64("REPL_ENTRY_BYTES", 4 * 1024 * 1024) as u32,
            lag_entries: env_u64("REPL_LAG_MAX", 64),
            lag: Duration::from_millis(env_u64("REPL_LAG_MAX_MS", 1_000)),
            catchup_batch: env_u64("REPL_CATCHUP_BATCH", 500) as u32,
            log_retain: Duration::from_secs(env_u64("REPL_LOG_RETAIN_SECS", 24 * 60 * 60)),
            log_retain_entries: env_u64("REPL_LOG_RETAIN", 1_000_000),
            probe: Duration::from_millis(env_u64("REPL_PROBE_MS", 1_000)),
            down: Duration::from_millis(env_u64("REPL_DOWN_MS", 10_000)),
            reallocate_delay: Duration::from_millis(env_u64("REPL_REALLOCATE_DELAY_MS", 60_000)),
            max_builds_per_node: env_u64("REPL_MAX_BUILDS_PER_NODE", 2) as u32,
            hedge_floor: Duration::from_millis(env_u64("REPL_HEDGE_FLOOR_MS", 20)),
            shard_timeout: Duration::from_millis(env_u64("SEARCH_SHARD_TIMEOUT_MS", 2_000)),
            hydrate: Duration::from_millis(env_u64("SEARCH_HYDRATE_MS", 100)),
            p95_target: Duration::from_millis(env_u64("SEARCH_P95_TARGET_MS", 200)),
            graph_copies: env_u64("GRAPH_COPIES", 1) as u32,
            archive_url: env_string("REPL_ARCHIVE_URL", "file://"),
            backup_at: env_string("REPL_BACKUP_AT", "03:00"),
            surreal_user: env_string("SURREAL_USER", "venus"),
            surreal_pass: env_string("SURREAL_PASS", "venus"),
            tls_ca: env_string("REPL_TLS_CA", ""),
        }
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_string(name: &str, default: &str) -> String {
    env::var(name).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| default.to_string())
}
