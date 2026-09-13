//! Path B convert: spawn `from-pinned-cli.js` (Vite → `fromPinnedBytes`).
//! Dialect **oracle** for tests. Product Flush uses api-map Convert engine
//! ([`crate::from_doc`]; goldens matched). Not the RAM pane exporter.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug, Clone)]
pub struct ConvertConfig {
    pub node: PathBuf,
    pub cli: PathBuf,
    pub cwd: PathBuf,
    pub blob_origin: Option<String>,
    pub timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Converted {
    pub markdown: String,
    pub sidecar: Sidecar,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct Sidecar {
    #[serde(rename = "docId")]
    pub doc_id: String,
    pub clock: String,
    pub blocks: Vec<SidecarBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, serde::Serialize)]
pub struct SidecarBlock {
    pub id: String,
    pub start: usize,
    pub end: usize,
}

/// Run Path B on pin bytes. stdout of the CLI is JSON `{ markdown, sidecar }`.
pub async fn from_pinned_bytes(bytes: &[u8], cfg: &ConvertConfig) -> Result<Converted> {
    if !cfg.cli.is_file() {
        bail!(
            "CONVERT_CLI not found: {} (set CONVERT_CLI / CONVERT_CWD; cwd {})",
            cfg.cli.display(),
            cfg.cwd.display()
        );
    }
    if !cfg.cwd.is_dir() {
        bail!("CONVERT_CWD is not a directory: {}", cfg.cwd.display());
    }

    tracing::info!(
        cli = %cfg.cli.display(),
        cwd = %cfg.cwd.display(),
        pin_bytes = bytes.len(),
        "spawning Path B convert CLI"
    );

    // File argv, not stdin: Vite ESM loads before the CLI body would read `-`,
    // which EPIPE'd ~1MiB pins on a 64KiB pipe. api-map Actual is a `.yjs` path.
    let pin_path = write_tmp("yjs", bytes)?;
    let _pin_guard = TmpPath(pin_path.clone());
    let json_path = write_tmp("json", b"")?;
    let _json_guard = TmpPath(json_path.clone());
    let json_file = std::fs::File::create(&json_path)
        .with_context(|| format!("create convert stdout {}", json_path.display()))?;

    let mut cmd = Command::new(&cfg.node);
    cmd.arg(&cfg.cli)
        .arg(&pin_path)
        .current_dir(&cfg.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(json_file))
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    match &cfg.blob_origin {
        Some(origin) => {
            cmd.env("VENUS_BLOB_ORIGIN", origin);
        }
        None => {
            cmd.env_remove("VENUS_BLOB_ORIGIN");
        }
    }

    let child = cmd.spawn().with_context(|| {
        format!(
            "spawn {} {} {} (is Node on PATH?)",
            cfg.node.display(),
            cfg.cli.display(),
            pin_path.display()
        )
    })?;

    let output = timeout(cfg.timeout, child.wait_with_output())
        .await
        .map_err(|_| anyhow!("convert CLI timed out after {}ms", cfg.timeout.as_millis()))?
        .context("wait convert CLI")?;

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let stdout = std::fs::read_to_string(&json_path).with_context(|| {
        format!("read convert JSON stdout {}", json_path.display())
    })?;
    if !output.status.success() {
        bail!(
            "convert CLI exited {}: stderr={stderr} stdout={stdout}",
            output.status
        );
    }

    serde_json::from_str::<Converted>(stdout.trim())
        .with_context(|| format!("parse convert CLI JSON stdout; stderr={stderr} stdout={stdout}"))
}

struct TmpPath(PathBuf);

impl Drop for TmpPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn write_tmp(ext: &str, bytes: &[u8]) -> Result<PathBuf> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!(
        "venus-pin-{}-{nanos}.{ext}",
        std::process::id()
    ));
    std::fs::write(&path, bytes)
        .with_context(|| format!("write temp file {}", path.display()))?;
    Ok(path)
}
