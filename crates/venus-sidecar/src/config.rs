//! Listen address and Path B CLI paths from the environment.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::convert::ConvertConfig;

#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub convert: ConvertConfig,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let listen = env::var("SIDECAR_LISTEN")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "0.0.0.0:3002".into())
            .parse()
            .context("SIDECAR_LISTEN")?;
        Ok(Self {
            listen,
            convert: ConvertConfig::from_env()?,
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
