//! Observer inserts `jobs`; workers claim, cut, convert, then git + `last_flushed`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::PgPool;

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
}

const OBSERVE_SQL: &str = "
INSERT INTO jobs (workspace_id, reason, not_before)
SELECT w.workspace_id, 'idle', w.first_dirty_at + ($1::bigint * interval '1 millisecond')
FROM dirty_wiki w
WHERE NOT EXISTS (
    SELECT 1 FROM jobs j WHERE j.workspace_id = w.workspace_id
)
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
    WHERE not_before <= now()
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

/// One observer tick. `idle` is `SNAPSHOT_IDLE_MS` (`0` is due immediately).
pub async fn observe_once(pool: &PgPool, idle: Duration) -> Result<u64> {
    let idle_ms: i64 = idle.as_millis().try_into().context("SNAPSHOT_IDLE_MS")?;
    let done = sqlx::query(OBSERVE_SQL)
        .bind(idle_ms)
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

/// Lease one due job and **COMMIT**. Does not copy Yjs / fill [`PinMap`].
pub async fn claim_one(pool: &PgPool, owner: &str) -> Result<Option<Claim>> {
    let mut tx = pool.begin().await.context("claim begin")?;
    let workspace_id: Option<String> = sqlx::query_scalar(CLAIM_SQL)
        .bind(owner)
        .fetch_optional(&mut *tx)
        .await
        .context("claim skip locked")?;
    tx.commit().await.context("claim commit")?;
    Ok(workspace_id.map(|workspace_id| Claim {
        workspace_id,
        owner: owner.to_string(),
    }))
}

/// Claim, cut S into the Map, COMMIT, then `fromDoc` the Map (no git).
pub async fn worker_turn(pool: &PgPool, owner: &str, pins: &mut PinMap) -> Result<Option<Claim>> {
    worker_turn_with(pool, owner, pins, Duration::ZERO).await
}

/// Same as [`worker_turn`], with an optional sleep after cut (before convert).
pub async fn worker_turn_with(
    pool: &PgPool,
    owner: &str,
    pins: &mut PinMap,
    convert_sleep: Duration,
) -> Result<Option<Claim>> {
    let claimed = claim_and_cut(pool, owner, pins, convert_sleep).await?;
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
    cut_workspace(pool, &claim.workspace_id, pins).await?;
    if !convert_sleep.is_zero() {
        tokio::time::sleep(convert_sleep).await;
    }
    let converted = convert_pins(&claim.workspace_id, pins)?;
    let write = git::commit_pins(wiki, pins, &converted)?;
    if let Some(ref write) = write {
        upsert_last_flushed(pool, &claim.workspace_id, pins, write).await?;
    }
    delete_job(pool, &claim.workspace_id).await?;
    pins.clear();
    Ok(FlushOutcome {
        claim,
        sha: write.as_ref().map(|w| w.sha.clone()),
        committed: write.map(|w| w.committed).unwrap_or(false),
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
    let Some(claim) = claim_one(pool, owner).await? else {
        return Ok(None);
    };
    Ok(Some(
        flush_claimed(pool, claim, pins, wiki, convert_sleep).await?,
    ))
}

async fn claim_and_cut(
    pool: &PgPool,
    owner: &str,
    pins: &mut PinMap,
    convert_sleep: Duration,
) -> Result<Option<Claim>> {
    let claimed = claim_one(pool, owner).await?;
    if let Some(ref claim) = claimed {
        cut_workspace(pool, &claim.workspace_id, pins).await?;
        if !convert_sleep.is_zero() {
            tokio::time::sleep(convert_sleep).await;
        }
    }
    Ok(claimed)
}

/// Path B on every pin. Call only after the cut txn has committed and S is full.
pub fn convert_pins(workspace_id: &str, pins: &PinMap) -> Result<Vec<(String, Converted)>> {
    let mut out = Vec::with_capacity(pins.len());
    for (doc_id, entry) in pins.iter() {
        anyhow::ensure!(
            !entry.bytes.is_empty(),
            "pin {doc_id} has empty bytes (not a CRDT pin)"
        );
        let converted = from_doc::from_pinned_bytes_in(&entry.bytes, workspace_id)
            .with_context(|| format!("fromDoc pin {doc_id}"))?;
        out.push((doc_id.to_string(), converted));
    }
    Ok(out)
}

pub async fn run_observer(pool: PgPool, idle: Duration, period: Duration) {
    loop {
        if let Err(e) = observe_once(&pool, idle).await {
            tracing::warn!(error = %e, "observer tick failed");
        }
        tokio::time::sleep(period).await;
    }
}

pub async fn run_worker(pool: PgPool, owner: String, convert_sleep: Duration, wiki: WikiConfig) {
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
