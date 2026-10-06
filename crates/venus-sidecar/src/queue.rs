//! Observer inserts `jobs`; workers claim, cut, convert, then git + `last_flushed`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::collections::HashMap;
use std::future::Future;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

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
      AND state = 'pending'
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

const REOPEN_SQL: &str = "
UPDATE jobs AS j
SET state = 'pending',
    reason = 'idle',
    attempts = 0,
    last_error = NULL,
    last_error_at = NULL,
    failed_clock = NULL,
    owner = NULL,
    lease_until = NULL,
    not_before = w.first_dirty_at + ($1::bigint * interval '1 millisecond')
FROM dirty_wiki w
WHERE j.workspace_id = w.workspace_id
  AND j.workspace_id = $2::uuid
  AND j.state = 'failed'
  AND EXISTS (
    SELECT 1
    FROM dirty d
    WHERE d.workspace_id = j.workspace_id
      AND d.clock > COALESCE(j.failed_clock, 0)
  )
";

/// One observer tick for `workspace_id` only. `idle` is `SNAPSHOT_IDLE_MS`
/// (`0` is due immediately). Other workspaces are not enqueued into this wiki.
/// A `failed` job is reopened only when a dirty clock is newer than the one
/// recorded at the failure.
pub async fn observe_once(pool: &PgPool, idle: Duration, workspace_id: &str) -> Result<u64> {
    let idle_ms: i64 = idle.as_millis().try_into().context("SNAPSHOT_IDLE_MS")?;
    let reopened = sqlx::query(REOPEN_SQL)
        .bind(idle_ms)
        .bind(workspace_id)
        .execute(pool)
        .await
        .context("observer reopen failed job")?;
    let done = sqlx::query(OBSERVE_SQL)
        .bind(idle_ms)
        .bind(workspace_id)
        .execute(pool)
        .await
        .context("observer insert jobs")?;
    Ok(reopened.rows_affected() + done.rows_affected())
}

const FLUSH_SQL: &str = "
INSERT INTO jobs (workspace_id, reason, not_before)
VALUES ($1::uuid, 'flush', now())
ON CONFLICT (workspace_id) DO UPDATE
SET reason = 'flush',
    not_before = now()
";

/// `POST /flush` while the job is `failed`. The row stays failed until a newer
/// dirty clock reopens it.
#[derive(Debug)]
pub struct FlushBlocked {
    pub last_error: String,
    pub attempts: i32,
}

impl std::fmt::Display for FlushBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.last_error.is_empty() {
            "flush failed"
        } else {
            &self.last_error
        })
    }
}

impl std::error::Error for FlushBlocked {}

/// Last commit and the current job error, if any.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlushStatus {
    pub sha: Option<String>,
    pub last_error: Option<String>,
    pub attempts: i32,
    pub failed: bool,
}

/// Pull this wiki’s job to `reason=flush`, `not_before=now()`. Does not copy Yjs
/// or clear an inflight lease (a second Flush does not start a second convert).
/// A `failed` job is left alone and returned as [`FlushBlocked`].
pub async fn flush_now(pool: &PgPool, workspace_id: &str) -> Result<()> {
    let blocked: Option<(String, i32)> = sqlx::query_as(
        "SELECT COALESCE(last_error, ''), attempts
         FROM jobs
         WHERE workspace_id = $1::uuid AND state = 'failed'",
    )
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
    .context("flush failed check")?;
    if let Some((last_error, attempts)) = blocked {
        return Err(FlushBlocked {
            last_error,
            attempts,
        }
        .into());
    }
    sqlx::query(FLUSH_SQL)
        .bind(workspace_id)
        .execute(pool)
        .await
        .context("flush upsert jobs")?;
    Ok(())
}

pub async fn flush_status(pool: &PgPool, workspace_id: &str) -> Result<FlushStatus> {
    let row: (Option<String>, Option<String>, i32, bool) = sqlx::query_as(
        "SELECT
            lf.git_sha,
            j.last_error,
            COALESCE(j.attempts, 0)::int4,
            COALESCE(j.state = 'failed', false)
         FROM (SELECT $1::uuid AS workspace_id) AS s
         LEFT JOIN jobs j ON j.workspace_id = s.workspace_id
         LEFT JOIN LATERAL (
            SELECT git_sha
            FROM last_flushed
            WHERE workspace_id = s.workspace_id
            ORDER BY clock DESC
            LIMIT 1
         ) lf ON true",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await
    .context("flush status")?;
    Ok(FlushStatus {
        sha: row.0,
        last_error: row.1,
        attempts: row.2,
        failed: row.3,
    })
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
/// A failure releases the lease and records the error, but only while this
/// worker still owns the row. Deterministic errors fail the job immediately.
/// Transient errors back off, then fail after [`MAX_FLUSH_ATTEMPTS`].
/// The work itself stops at [`FLUSH_LEASE`] (the same two minutes as the claim).
pub async fn flush_claimed(
    pool: &PgPool,
    claim: Claim,
    pins: &mut PinMap,
    wiki: &WikiConfig,
    convert_sleep: Duration,
) -> Result<FlushOutcome> {
    let workspace_id = claim.workspace_id.clone();
    let owner = claim.owner.clone();
    match flush_claimed_inner(pool, claim, pins, wiki, convert_sleep, FlushBudget::start()).await {
        Ok(outcome) => Ok(outcome),
        Err(e) => {
            if let Err(record) = record_flush_failure(pool, &workspace_id, &owner, &e).await {
                tracing::warn!(
                    workspace_id = %workspace_id,
                    error = %record,
                    "could not record flush failure"
                );
            }
            Err(e)
        }
    }
}

async fn flush_claimed_inner(
    pool: &PgPool,
    claim: Claim,
    pins: &mut PinMap,
    wiki: &WikiConfig,
    convert_sleep: Duration,
    budget: FlushBudget,
) -> Result<FlushOutcome> {
    budget
        .within(cut_workspace(
            pool,
            &claim.workspace_id,
            pins,
            Some(&wiki.dir),
        ))
        .await?;
    if !convert_sleep.is_zero() {
        let wait = convert_sleep.min(budget.remaining());
        if wait.is_zero() {
            return Err(flush_timeout_error());
        }
        tokio::time::sleep(wait).await;
        if wait < convert_sleep || budget.exceeded() {
            return Err(flush_timeout_error());
        }
    }
    // The cut stored the walk with the pins. Walk here only when a catalog pin
    // is present and the cut did not (no wiki directory, so no convert set).
    if pins.catalog_walk().is_none() {
        if let Some(bytes) = pins
            .get(crate::CATALOG_DOC_ID)
            .map(|entry| entry.bytes.clone())
        {
            pins.set_catalog_walk(Some(catalog::walk_pin(&bytes).context("walk catalog pin")?));
        }
    }
    let walk = pins.catalog_walk().cloned();
    if let Some(yaml) = git::head_pages_yaml(&wiki.dir)? {
        budget
            .within(catalog::reconcile_page_identity(
                pool,
                &claim.workspace_id,
                &yaml,
            ))
            .await?;
    }
    let old_pages = budget
        .within(catalog::load_old_pages(pool, &claim.workspace_id))
        .await?;
    let missing = crate::links::missing_link_names(walk.as_ref(), &old_pages);
    let converted =
        convert_pins_stopping(&claim.workspace_id, pins, walk.as_ref(), &missing, || {
            budget.ensure()
        })?;
    let converted_ids: Vec<String> = converted.iter().map(|(id, _)| id.clone()).collect();
    // Do not start the git commit once the lease window is over. The commit
    // itself is not cancelled mid-write; a second worker waits until it returns.
    let write = {
        let _git = wiki_commit_lock().lock().await;
        budget.ensure()?;
        git::commit_pins(
            wiki,
            &claim.workspace_id,
            pins,
            &converted,
            walk.as_ref(),
            &old_pages,
        )?
    };
    if let Some(ref write) = write {
        budget
            .within(record_published_state(
                pool,
                &claim.workspace_id,
                walk.as_ref(),
                pins,
                write,
            ))
            .await?;
    }
    delete_job(pool, &claim.workspace_id, &claim.owner).await?;
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
    convert_pins_catalog(workspace_id, pins, None, &HashMap::new())
}

pub fn convert_pins_catalog(
    workspace_id: &str,
    pins: &PinMap,
    walk: Option<&catalog::CatalogWalk>,
    missing: &HashMap<String, String>,
) -> Result<Vec<(String, Converted)>> {
    convert_pins_stopping(workspace_id, pins, walk, missing, || Ok(()))
}

fn convert_pins_stopping(
    workspace_id: &str,
    pins: &PinMap,
    walk: Option<&catalog::CatalogWalk>,
    missing: &HashMap<String, String>,
    mut stop: impl FnMut() -> Result<()>,
) -> Result<Vec<(String, Converted)>> {
    let pages = walk
        .map(from_doc::catalog_page_index)
        .map(std::sync::Arc::new);
    let missing = std::sync::Arc::new(missing.clone());
    let mut out = Vec::with_capacity(pins.len());
    for (doc_id, entry) in pins.iter() {
        stop()?;
        if is_catalog_sql_id(doc_id) {
            continue;
        }
        anyhow::ensure!(
            !entry.bytes.is_empty(),
            "pin {doc_id} has empty bytes (not a CRDT pin)"
        );
        let sidecar_id = sidecar_doc_id(doc_id);
        let converted = from_doc::from_pinned_bytes_catalog_shared(
            &entry.bytes,
            workspace_id,
            sidecar_id,
            walk,
            pages.as_ref(),
            Some(&missing),
        )
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
            tokio::time::sleep(Duration::from_millis(60_000)).await;
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
                let class = if is_transient_flush_error(&e) {
                    "transient"
                } else {
                    "deterministic"
                };
                tracing::warn!(owner = %owner, class, error = %e, "flush turn failed");
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

async fn record_published_state(
    pool: &PgPool,
    workspace_id: &str,
    walk: Option<&catalog::CatalogWalk>,
    pins: &PinMap,
    write: &SnapshotWrite,
) -> Result<()> {
    let mut tx = pool.begin().await.context("publish begin")?;
    if let Some(walk) = walk {
        catalog::replace_page_identity_tx(
            &mut tx,
            workspace_id,
            &walk.pages,
            catalog::Leaving::Tombstone,
        )
        .await?;
    }
    for (doc_id, entry) in pins.iter() {
        sqlx::query(LAST_FLUSHED_SQL)
            .bind(workspace_id)
            .bind(doc_id)
            .bind(entry.clock)
            .bind(&write.sha)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("last_flushed {doc_id}"))?;
    }
    tx.commit().await.context("publish commit")?;
    Ok(())
}

async fn delete_job(pool: &PgPool, workspace_id: &str, owner: &str) -> Result<()> {
    sqlx::query("DELETE FROM jobs WHERE workspace_id = $1::uuid AND owner = $2")
        .bind(workspace_id)
        .bind(owner)
        .execute(pool)
        .await
        .context("delete jobs")?;
    Ok(())
}

/// Same length as `CLAIM_SQL`'s `lease_until`. A Flush does not run past this.
pub const FLUSH_LEASE: Duration = Duration::from_secs(120);

fn flush_timeout_error() -> anyhow::Error {
    anyhow::anyhow!("flush timed out after 2 minutes")
}

#[derive(Clone, Copy)]
struct FlushBudget {
    deadline: Instant,
}

impl FlushBudget {
    fn start() -> Self {
        Self::limit(FLUSH_LEASE)
    }

    fn limit(limit: Duration) -> Self {
        Self {
            deadline: Instant::now() + limit,
        }
    }

    fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    fn exceeded(&self) -> bool {
        Instant::now() >= self.deadline
    }

    fn ensure(&self) -> Result<()> {
        if self.exceeded() {
            Err(flush_timeout_error())
        } else {
            Ok(())
        }
    }

    async fn within<T>(&self, fut: impl Future<Output = Result<T>>) -> Result<T> {
        let remaining = self.remaining();
        if remaining.is_zero() {
            return Err(flush_timeout_error());
        }
        match tokio::time::timeout(remaining, fut).await {
            Ok(value) => value,
            Err(_) => Err(flush_timeout_error()),
        }
    }
}

fn wiki_commit_lock() -> &'static tokio::sync::Mutex<()> {
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
}

/// Transient failures retry this many times, then the job is `failed`.
pub const MAX_FLUSH_ATTEMPTS: i32 = 5;

/// `1 → 2s`, then doubles, capped at 2 minutes.
pub fn flush_backoff(attempts: i32) -> Duration {
    let step = attempts.clamp(1, 7) as u32;
    Duration::from_secs(2u64.saturating_pow(step)).min(Duration::from_secs(120))
}

/// Database errors and I/O timeouts retry. Validation and collision errors do not.
pub fn is_transient_flush_error(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if cause.downcast_ref::<sqlx::Error>().is_some() {
            return true;
        }
        if let Some(io) = cause.downcast_ref::<std::io::Error>() {
            return matches!(
                io.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::Interrupted
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
                    | std::io::ErrorKind::UnexpectedEof
                    | std::io::ErrorKind::NotConnected
                    | std::io::ErrorKind::ConnectionRefused
            );
        }
    }
    let msg = err.to_string().to_ascii_lowercase();
    const DETERMINISTIC: &[&str] = &[
        "would overwrite",
        "no home doc",
        "pin clock missing",
        "empty bytes",
        "not a crdt pin",
        "file name too long",
        "enametoolong",
        "refusing workspace",
        "must have no origin",
        "flush timed out",
    ];
    if DETERMINISTIC.iter().any(|needle| msg.contains(needle)) {
        return false;
    }
    const TRANSIENT: &[&str] = &[
        "timed out",
        "timeout",
        "connection refused",
        "connection reset",
        "connection aborted",
        "broken pipe",
        "deadlock",
        "serialization failure",
        "too many connections",
        "could not connect",
    ];
    if TRANSIENT.iter().any(|needle| msg.contains(needle)) {
        return true;
    }
    // Unknown failures still retry, and stop after MAX_FLUSH_ATTEMPTS.
    true
}

/// Release the lease. Deterministic errors, and the Nth transient error, set
/// `state = failed` until a newer dirty clock appears.
pub async fn record_flush_failure(
    pool: &PgPool,
    workspace_id: &str,
    owner: &str,
    err: &anyhow::Error,
) -> Result<()> {
    let deterministic = !is_transient_flush_error(err);
    let text: String = err.to_string().chars().take(2000).collect();
    let attempts: Option<i32> =
        sqlx::query_scalar("SELECT attempts FROM jobs WHERE workspace_id = $1::uuid")
            .bind(workspace_id)
            .fetch_optional(pool)
            .await
            .context("flush attempts")?;
    let Some(attempts) = attempts else {
        return Ok(());
    };
    let next = attempts.saturating_add(1);
    let delay_ms: i64 = flush_backoff(next)
        .as_millis()
        .try_into()
        .context("flush backoff")?;
    sqlx::query(
        "UPDATE jobs AS j
         SET attempts = j.attempts + 1,
             last_error = $2,
             last_error_at = now(),
             failed_clock = COALESCE(
                 (SELECT MAX(d.clock) FROM dirty d WHERE d.workspace_id = j.workspace_id),
                 0
             ),
             lease_until = NULL,
             owner = NULL,
             state = CASE
                 WHEN $3::bool OR j.attempts + 1 >= $4 THEN 'failed'
                 ELSE 'pending'
             END,
             not_before = CASE
                 WHEN $3::bool OR j.attempts + 1 >= $4 THEN j.not_before
                 ELSE now() + ($5::bigint * interval '1 millisecond')
             END
         WHERE j.workspace_id = $1::uuid
           AND (j.owner IS NULL OR j.owner = $6)",
    )
    .bind(workspace_id)
    .bind(&text)
    .bind(deterministic)
    .bind(MAX_FLUSH_ATTEMPTS)
    .bind(delay_ms)
    .bind(owner)
    .execute(pool)
    .await
    .context("record flush failure")?;
    Ok(())
}

#[cfg(test)]
mod failure_tests {
    use super::*;

    #[test]
    fn a_flush_that_reaches_the_lease_fails_instead_of_retrying() {
        let err = anyhow::anyhow!("flush timed out after 2 minutes");
        assert!(!is_transient_flush_error(&err));
        assert!(is_transient_flush_error(&anyhow::anyhow!(
            "connection timed out"
        )));
        assert_eq!(FLUSH_LEASE, Duration::from_secs(120));
        let src = include_str!("queue.rs");
        assert!(src.contains("lease_until = now() + interval '2 minutes'"));
        assert!(src.contains("AND owner = $2"));
        assert!(src.contains("j.owner IS NULL OR j.owner = $6"));
        let body = src
            .split("async fn flush_claimed_inner")
            .nth(1)
            .expect("flush_claimed_inner")
            .split("async fn claim_and_cut")
            .next()
            .expect("body");
        let lock_at = body.find("wiki_commit_lock").expect("lock");
        let git_at = body.find("commit_pins").expect("commit");
        assert!(lock_at < git_at);
        assert!(body[lock_at..git_at].contains("ensure()"));
    }

    #[tokio::test]
    async fn expired_budget_does_not_start_the_next_step() {
        let budget = FlushBudget::limit(Duration::ZERO);
        let mut started = false;
        let err = budget
            .within(async {
                started = true;
                Ok(())
            })
            .await
            .expect_err("expired");
        assert!(!started);
        assert!(err.to_string().contains("flush timed out"));
    }

    #[tokio::test]
    async fn budget_interrupts_a_step_that_runs_past_the_lease() {
        let budget = FlushBudget::limit(Duration::from_millis(40));
        let started = Instant::now();
        let err = budget
            .within(async {
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(())
            })
            .await
            .expect_err("interrupted");
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(err.to_string().contains("flush timed out"));
        assert!(!is_transient_flush_error(&err));
    }

    #[test]
    fn convert_stops_before_the_next_page_once_the_budget_is_spent() {
        use crate::pin::PinEntry;

        let mut pins = PinMap::new();
        pins.insert(
            "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee".into(),
            PinEntry {
                bytes: vec![1],
                clock: 1,
            },
        );
        let mut checks = 0;
        let err = convert_pins_stopping("", &pins, None, &HashMap::new(), || {
            checks += 1;
            Err(flush_timeout_error())
        })
        .expect_err("stop");
        assert_eq!(checks, 1);
        assert!(err.to_string().contains("flush timed out"));
    }

    #[test]
    fn validation_errors_are_deterministic() {
        assert!(!is_transient_flush_error(&anyhow::anyhow!(
            "catalog pin has no home doc"
        )));
        assert!(!is_transient_flush_error(&anyhow::anyhow!(
            "git mv would overwrite spec/notes.md"
        )));
        assert!(!is_transient_flush_error(&anyhow::anyhow!(
            "pin clock missing for converted page"
        )));
    }

    #[test]
    fn timeouts_and_database_text_are_transient() {
        assert!(is_transient_flush_error(&anyhow::anyhow!(
            "connection refused"
        )));
        let io = std::io::Error::new(std::io::ErrorKind::TimedOut, "read");
        assert!(is_transient_flush_error(&anyhow::Error::new(io)));
        assert!(is_transient_flush_error(&anyhow::anyhow!("disk full")));
    }

    #[test]
    fn page_identity_and_last_flushed_share_one_transaction() {
        let src = include_str!("queue.rs");
        let start = src
            .find("async fn record_published_state")
            .expect("record_published_state");
        let rest = &src[start..];
        let end = rest[1..]
            .find("\nasync fn")
            .map(|index| index + 1)
            .unwrap_or(rest.len());
        let body = &rest[..end];
        assert_eq!(body.matches(".begin()").count(), 1, "{body}");
        assert!(body.contains("replace_page_identity_tx"));
        assert!(body.contains("LAST_FLUSHED_SQL"));
        assert!(body.contains(".commit()"));
        let flush_at = src
            .find("async fn flush_claimed_inner")
            .expect("flush_claimed_inner");
        let flush_rest = &src[flush_at..];
        let flush_end = flush_rest[1..]
            .find("\nasync fn")
            .map(|index| index + 1)
            .unwrap_or(flush_rest.len());
        let flush_body = &flush_rest[..flush_end];
        let reconcile = flush_body
            .find("reconcile_page_identity")
            .expect("reconcile");
        let load = flush_body.find("load_old_pages").expect("load_old_pages");
        assert!(reconcile < load);
        assert!(flush_body.contains("head_pages_yaml"));
        assert!(flush_body.contains("record_published_state"));
        assert!(!flush_body.contains("upsert_last_flushed"));
        let stored = flush_body.find("catalog_walk").expect("stored walk");
        let fallback = flush_body.find("walk_pin").expect("fallback walk");
        assert!(stored < fallback);
        assert_eq!(flush_body.matches("walk_pin").count(), 1);
    }

    #[test]
    fn convert_shares_one_page_index_and_does_not_walk() {
        let src = include_str!("queue.rs");
        let start = src.find("fn convert_pins_stopping").expect("convert");
        let rest = &src[start..];
        let end = rest[1..]
            .find("\npub async fn")
            .map(|index| index + 1)
            .unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(!body.contains("walk_pin"), "{body}");
        let loop_at = body.find("for (doc_id").expect("loop");
        assert!(body[..loop_at].contains("catalog_page_index"));
        assert!(!body[loop_at..].contains("catalog_page_index"));
    }

    #[test]
    fn backoff_doubles_and_caps_at_two_minutes() {
        assert_eq!(flush_backoff(1), Duration::from_secs(2));
        assert_eq!(flush_backoff(2), Duration::from_secs(4));
        assert_eq!(flush_backoff(3), Duration::from_secs(8));
        assert_eq!(flush_backoff(7), Duration::from_secs(120));
        assert_eq!(flush_backoff(20), Duration::from_secs(120));
    }
}
