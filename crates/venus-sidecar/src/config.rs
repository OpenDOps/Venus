//! Listen address, Path B CLI, and snapshotter queue from the environment.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::convert::ConvertConfig;
use crate::git::{WikiConfig, DEFAULT_AUTHOR_EMAIL, DEFAULT_AUTHOR_NAME, DEFAULT_WIKI_DIR};

pub const DEFAULT_SNAPSHOT_IDLE_MS: u64 = 60_000;
pub const DEFAULT_SNAPSHOT_OBSERVE_MS: u64 = 1_000;
pub const DEFAULT_SNAPSHOT_WORKERS: u32 = 2;

pub const DEFAULT_CORS_ORIGINS: [&str; 6] = [
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "http://localhost:5174",
    "http://127.0.0.1:5174",
    "http://localhost:8080",
    "http://127.0.0.1:8080",
];

#[derive(Debug, Clone)]
pub struct QueueConfig {
    pub idle: Duration,
    pub observe: Duration,
    pub workers: u32,
    /// Test hook: sleep after cut COMMIT, before `fromDoc`. Default 0.
    pub convert_sleep: Duration,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub convert: ConvertConfig,
    /// Postgres DSN. When set, the process runs observer + workers. HTTP health
    /// still serves without it (host `cargo run` recon).
    pub database_url: Option<String>,
    pub owner: String,
    pub queue: QueueConfig,
    pub wiki: WikiConfig,
    /// Unset → Vite/Compose localhost. Empty env → no CORS layer.
    pub cors_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen = env::var("SIDECAR_LISTEN")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "0.0.0.0:3002".into())
            .parse()
            .context("SIDECAR_LISTEN")?;
        let owner = env::var("SIDECAR_OWNER")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(default_owner);
        Ok(Self {
            listen,
            convert: ConvertConfig::from_env()?,
            database_url: database_url_from_env()?,
            owner,
            queue: QueueConfig::from_env()?,
            wiki: wiki_from_env()?,
            cors_origins: cors_origins_from_env()?,
        })
    }
}

fn wiki_from_env() -> Result<WikiConfig> {
    let dir = env::var("WIKI_DIR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_WIKI_DIR));
    let author_name = env::var("SNAPSHOT_GIT_AUTHOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AUTHOR_NAME.into());
    let author_email = env::var("SNAPSHOT_GIT_EMAIL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_AUTHOR_EMAIL.into());
    Ok(WikiConfig {
        dir,
        author_name,
        author_email,
    })
}

fn cors_origins_from_env() -> Result<Vec<String>> {
    match env::var("SIDECAR_CORS_ORIGINS") {
        Err(_) => Ok(DEFAULT_CORS_ORIGINS
            .iter()
            .map(|s| (*s).to_string())
            .collect()),
        Ok(s) if s.trim().is_empty() => Ok(Vec::new()),
        Ok(s) => Ok(s
            .split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(ToString::to_string)
            .collect()),
    }
}

impl QueueConfig {
    pub fn from_env() -> Result<Self> {
        let workers = u32_env("SNAPSHOT_WORKERS", DEFAULT_SNAPSHOT_WORKERS)?;
        if workers < 2 {
            bail!("SNAPSHOT_WORKERS must be >= 2 (got {workers})");
        }
        Ok(Self {
            idle: duration_ms_env("SNAPSHOT_IDLE_MS", DEFAULT_SNAPSHOT_IDLE_MS)?,
            observe: duration_ms_env("SNAPSHOT_OBSERVE_MS", DEFAULT_SNAPSHOT_OBSERVE_MS)?,
            workers,
            convert_sleep: duration_ms_env("SNAPSHOT_CONVERT_SLEEP_MS", 0)?,
        })
    }
}

impl ConvertConfig {
    pub fn from_env() -> Result<Self> {
        let cwd = env::var("CONVERT_CWD")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("apps/web")
            });
        let cli = env::var("CONVERT_CLI")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| cwd.join("src/host/mdgate/from-pinned-cli.js"));
        let node = env::var("NODE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "node".into());
        let blob_origin = env::var("VENUS_BLOB_ORIGIN")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let timeout = duration_ms_env("CONVERT_TIMEOUT_MS", 90_000)?;
        Ok(Self {
            node: PathBuf::from(node),
            cli,
            cwd,
            blob_origin,
            timeout,
        })
    }
}

fn duration_ms_env(name: &str, default_ms: u64) -> Result<Duration> {
    match env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => {
            let ms: u64 = raw.trim().parse().with_context(|| name.to_string())?;
            Ok(Duration::from_millis(ms))
        }
        _ => Ok(Duration::from_millis(default_ms)),
    }
}

fn u32_env(name: &str, default: u32) -> Result<u32> {
    match env::var(name) {
        Ok(raw) if !raw.trim().is_empty() => raw.trim().parse().with_context(|| name.to_string()),
        _ => Ok(default),
    }
}

fn default_owner() -> String {
    let host = hostname();
    host + "-" + &std::process::id().to_string()
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "sidecar".into())
}

/// `DATABASE_URL` wins. Else `POSTGRES_HOST` + `POSTGRES_USER` + `POSTGRES_PASSWORD`.
/// Unset host and URL → `None` (HTTP-only). SQLite is rejected.
pub fn database_url_from_env() -> Result<Option<String>> {
    if let Ok(url) = env::var("DATABASE_URL") {
        let url = url.trim().to_string();
        if !url.is_empty() {
            deny_sqlite(&url)?;
            return Ok(Some(url));
        }
    }
    match env::var("POSTGRES_HOST") {
        Err(_) => Ok(None),
        Ok(host) if host.trim().is_empty() => Ok(None),
        Ok(host) => {
            let user = required("POSTGRES_USER")?;
            let password = required("POSTGRES_PASSWORD")?;
            let db = env::var("POSTGRES_DB")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "venus".into());
            let port = env::var("POSTGRES_PORT")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| "5432".into());
            let url = [
                "postgres://",
                user.as_str(),
                ":",
                password.as_str(),
                "@",
                host.trim(),
                ":",
                port.as_str(),
                "/",
                db.as_str(),
                "?sslmode=disable",
            ]
            .concat();
            deny_sqlite(&url)?;
            Ok(Some(url))
        }
    }
}

fn required(name: &str) -> Result<String> {
    let v = env::var(name).with_context(|| {
        format!("{name} is required when POSTGRES_HOST is set and DATABASE_URL is unset")
    })?;
    let v = v.trim().to_string();
    if v.is_empty() {
        bail!("{name} is empty");
    }
    Ok(v)
}

fn deny_sqlite(url: &str) -> Result<()> {
    let lower = url.to_ascii_lowercase();
    if lower.starts_with("sqlite") || lower.contains("://sqlite") {
        bail!("SQLite is not a product store; set Postgres POSTGRES_* or DATABASE_URL");
    }
    Ok(())
}
