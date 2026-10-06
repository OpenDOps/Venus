//! Observer inserts `jobs`; workers claim, cut, convert, then git + `last_flushed`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::PgPool;

use crate::catalog::{self, is_catalog_sql_id, sidecar_doc_id};
use crate::convert::Converted;
use crate::cut::cut_workspace;
use crate::from_doc;
use crate::git::{self, SnapshotWrite, WikiConfig};
use crate::pin::PinMap;

/// Wiki job leased by one worker. Yjs bytes live on [`PinMap`] after the cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub workspace_id: String,
    pub owner: String,
}

/// One flush after a claim: git SHA (if any files) and whether a commit was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushOutcome {
    pub claim: Claim,
    pub sha: Option<String>,
    pub committed: bool,
    /// SQL uuids that ran `fromDoc` this Flush (not catalog, not git-mv-only).
    pub converted_ids: Vec<String>,
}

const OBSERVE_SQL: &str = "
INSERT INTO jobs (workspace_id, reason, not_before)
SELECT w.workspace_id, 'idle', w.first_dirty_at + ($1::bigint * interval '1 millisecond')
FROM dirty_wiki w
WHERE NOT EXISTS (
    SELECT 1 FROM jobs j WHERE j.workspace_id = w.workspace_id
)
AND w.workspace_id = $2::uuid
AND EXISTS (
    SELECT 1
    FROM dirty d
    LEFT JOIN last_flushed lf
      ON lf.workspace_id = d.workspace_id AND lf.doc_id = d.doc_id
    WHERE d.workspace_id = w.workspace_id
      AND (lf.clock IS NULL OR d.clock > lf.clock)
)
ON CONFLICT (workspace_id) DO NOTHING
";

const CLAIM_SQL: &str = "
WITH picked AS (
    SELECT workspace_id
    FROM jobs
    WHERE workspace_id = $2::uuid
      AND not_before <= now()
      AND (lease_until IS NULL OR lease_until < now())
    ORDER BY not_before ASC
    FOR UPDATE SKIP LOCKED
    LIMIT 1
)
UPDATE jobs AS j
SET owner = $1,
    lease_until = now() + interval '2 minutes'
FROM picked
WHERE j.workspace_id = picked.workspace_id
RETURNING j.workspace_id::text
";

/// One observer tick for `workspace_id` only. `idle` is `SNAPSHOT_IDLE_MS`
/// (`0` is due immediately). Other workspaces are not enqueued into this wiki.
pub async fn observe_once(pool: &PgPool, idle: Duration, workspace_id: &str) -> Result<u64> {
    let idle_ms: i64 = idle.as_millis().try_into().context("SNAPSHOT_IDLE_MS")?;
    let done = sqlx::query(OBSERVE_SQL)
        .bind(idle_ms)
        .bind(workspace_id)
        .execute(pool)
        .await
        .context("observer insert jobs")?;
    Ok(done.rows_affected())
}

const FLUSH_SQL: &str = "
INSERT INTO jobs (workspace_id, reason, not_before)
VALUES ($1::uuid, 'flush', now())
ON CONFLICT (workspace_id) DO UPDATE
SET reason = 'flush',
    not_before = now()
";

/// Pull this wiki’s job to `reason=flush`, `not_before=now()`. Does not copy Yjs
/// or clear an inflight lease (a second Flush does not start a second convert).
pub async fn flush_now(pool: &PgPool, workspace_id: &str) -> Result<()> {
    sqlx::query(FLUSH_SQL)
        .bind(workspace_id)
        .execute(pool)
        .await
        .context("flush upsert jobs")?;
    Ok(())
}

/// Lease one due job for `workspace_id` and **COMMIT**. Does not copy Yjs / fill [`PinMap`].
pub async fn claim_one(pool: &PgPool, owner: &str, workspace_id: &str) -> Result<Option<Claim>> {
    let mut tx = pool.begin().await.context("claim begin")?;
    let claimed: Option<String> = sqlx::query_scalar(CLAIM_SQL)
        .bind(owner)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await
        .context("claim skip locked")?;
    tx.commit().await.context("claim commit")?;
    Ok(claimed.map(|workspace_id| Claim {
        workspace_id,
        owner: owner.to_string(),
    }))
}

/// Claim, cut S into the Map, COMMIT, then `fromDoc` the Map (no git).
pub async fn worker_turn(
    pool: &PgPool,
    owner: &str,
    workspace_id: &str,
    pins: &mut PinMap,
) -> Result<Option<Claim>> {
    worker_turn_with(pool, owner, workspace_id, pins, Duration::ZERO).await
}

/// Same as [`worker_turn`], with an optional sleep after cut (before convert).
pub async fn worker_turn_with(
    pool: &PgPool,
    owner: &str,
    workspace_id: &str,
    pins: &mut PinMap,
    convert_sleep: Duration,
) -> Result<Option<Claim>> {
    let claimed = claim_and_cut(pool, owner, workspace_id, pins, convert_sleep).await?;
    if let Some(ref claim) = claimed {
        convert_pins(&claim.workspace_id, pins)?;
    }
    Ok(claimed)
}

/// Cut + convert + git + `last_flushed` + delete `jobs`. Caller already leased
/// this wiki (or [`claim_one`] just did). Drops the pin Map on success.
pub async fn flush_claimed(
    pool: &PgPool,
    claim: Claim,
    pins: &mut PinMap,
    wiki: &WikiConfig,
    convert_sleep: Duration,
) -> Result<FlushOutcome> {
    cut_workspace(pool, &claim.workspace_id, pins, Some(&wiki.dir)).await?;
    if !convert_sleep.is_zero() {
        tokio::time::sleep(convert_sleep).await;
    }
    let walk = match pins.get(crate::CATALOG_DOC_ID) {
        Some(entry) => Some(catalog::walk_pin(&entry.bytes).context("walk catalog pin")?),
        None => None,
    };
    let converted = convert_pins_catalog(&claim.workspace_id, pins, walk.as_ref())?;
    let converted_ids: Vec<String> = converted.iter().map(|(id, _)| id.clone()).collect();
    let old_pages = catalog::load_old_pages(pool, &claim.workspace_id).await?;
    let write = git::commit_pins(
        wiki,
        &claim.workspace_id,
        pins,
        &converted,
        walk.as_ref(),
        &old_pages,
    )?;
    if let Some(ref write) = write {
        if let Some(ref walk) = walk {
            catalog::replace_page_identity(pool, &claim.workspace_id, walk).await?;
        }
        upsert_last_flushed(pool, &claim.workspace_id, pins, write).await?;
    }
    delete_job(pool, &claim.workspace_id).await?;
    pins.clear();
    Ok(FlushOutcome {
        claim,
        sha: write.as_ref().map(|w| w.sha.clone()),
        committed: write.map(|w| w.committed).unwrap_or(false),
        converted_ids,
    })
}

/// Claim one due job, then [`flush_claimed`].
pub async fn flush_turn(
    pool: &PgPool,
    owner: &str,
    pins: &mut PinMap,
    wiki: &WikiConfig,
    convert_sleep: Duration,
) -> Result<Option<FlushOutcome>> {
    let Some(workspace_id) = wiki.workspace_id.as_deref() else {
        return Ok(None);
    };
    let Some(claim) = claim_one(pool, owner, workspace_id).await? else {
        return Ok(None);
    };
    Ok(Some(
        flush_claimed(pool, claim, pins, wiki, convert_sleep).await?,
    ))
}

async fn claim_and_cut(
    pool: &PgPool,
    owner: &str,
    workspace_id: &str,
    pins: &mut PinMap,
    convert_sleep: Duration,
) -> Result<Option<Claim>> {
    let claimed = claim_one(pool, owner, workspace_id).await?;
    if let Some(ref claim) = claimed {
        cut_workspace(pool, &claim.workspace_id, pins, None).await?;
        if !convert_sleep.is_zero() {
            tokio::time::sleep(convert_sleep).await;
        }
    }
    Ok(claimed)
}

/// Path B on every **page** pin (skip catalog). Call only after the cut txn
/// has committed and S is full. Catalog walk rewrites linked-doc hrefs.
pub fn convert_pins(workspace_id: &str, pins: &PinMap) -> Result<Vec<(String, Converted)>> {
    convert_pins_catalog(workspace_id, pins, None)
}

pub fn convert_pins_catalog(
    workspace_id: &str,
    pins: &PinMap,
    walk: Option<&catalog::CatalogWalk>,
) -> Result<Vec<(String, Converted)>> {
    let mut out = Vec::with_capacity(pins.len());
    for (doc_id, entry) in pins.iter() {
        if is_catalog_sql_id(doc_id) {
            continue;
        }
        anyhow::ensure!(
            !entry.bytes.is_empty(),
            "pin {doc_id} has empty bytes (not a CRDT pin)"
        );
        let sidecar_id = sidecar_doc_id(doc_id);
        let converted =
            from_doc::from_pinned_bytes_catalog(&entry.bytes, workspace_id, sidecar_id, walk)
                .with_context(|| format!("fromDoc pin {doc_id}"))?;
        out.push((doc_id.to_string(), converted));
    }
    Ok(out)
}

pub async fn run_observer(pool: PgPool, idle: Duration, period: Duration, workspace_id: String) {
    loop {
        if let Err(e) = observe_once(&pool, idle, &workspace_id).await {
            tracing::warn!(error = %e, workspace_id = %workspace_id, "observer tick failed");
        }
        tokio::time::sleep(period).await;
    }
}

pub async fn run_worker(pool: PgPool, owner: String, convert_sleep: Duration, wiki: WikiConfig) {
    if wiki.workspace_id.is_none() {
        tracing::error!("WIKI_WORKSPACE_ID unset; worker will not claim jobs");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
    let mut pins = PinMap::new();
    loop {
        match flush_turn(&pool, &owner, &mut pins, &wiki, convert_sleep).await {
            Ok(Some(outcome)) => {
                tracing::info!(
                    workspace_id = %outcome.claim.workspace_id,
                    owner = %outcome.claim.owner,
                    sha = ?outcome.sha,
                    committed = outcome.committed,
                    "flush committed"
                );
            }
            Ok(None) => {
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(e) => {
                tracing::warn!(owner = %owner, error = %e, "flush turn failed");
                pins.clear();
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }
}

const LAST_FLUSHED_SQL: &str = "
INSERT INTO last_flushed (workspace_id, doc_id, clock, git_sha)
VALUES ($1::uuid, $2::uuid, $3, $4)
ON CONFLICT (workspace_id, doc_id) DO UPDATE
SET clock = EXCLUDED.clock,
    git_sha = EXCLUDED.git_sha
";

async fn upsert_last_flushed(
    pool: &PgPool,
    workspace_id: &str,
    pins: &PinMap,
    write: &SnapshotWrite,
) -> Result<()> {
    for (doc_id, entry) in pins.iter() {
        sqlx::query(LAST_FLUSHED_SQL)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(entry.clock)
            .bind(&write.sha)
            .execute(pool)
            .await
            .with_context(|| format!("last_flushed {doc_id}"))?;
    }
    Ok(())
}

async fn delete_job(pool: &PgPool, workspace_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM jobs WHERE workspace_id = $1::uuid")
        .bind(workspace_id)
        .execute(pool)
        .await
        .context("delete jobs")?;
    Ok(())
}
