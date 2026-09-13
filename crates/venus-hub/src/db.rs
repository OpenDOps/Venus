//! Postgres CRDT / blob store. Opaque Yjs bytes — do not stringify history.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::future::Future;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::postgres::{PgConnection, PgPoolOptions};
use sqlx::{PgPool, Postgres, Transaction};
use y_octo::Doc;

use crate::protocol::{apply_v1, encode_v1};
use crate::PAGE_DOC_ID;

/// Postgres `40001` / `40P01`: retry hydrate once (L1).
fn is_retryable_tx(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => matches!(db.code().as_deref(), Some("40001" | "40P01")),
        _ => false,
    }
}

/// Skip a trail bin that fails `apply_v1` (L13). Snapshot apply stays fail-closed.
/// Does not DELETE the row; compact may later merge the prefix.
fn apply_trail_skip_bad(
    doc: &mut Doc,
    workspace_id: &str,
    doc_id: &str,
    updates: &[(i64, Vec<u8>)],
) {
    let mut skipped = 0u64;
    for (seq, bin) in updates {
        if let Err(e) = apply_v1(doc, bin) {
            skipped += 1;
            tracing::warn!(
                workspace_id = %workspace_id,
                doc_id = %doc_id,
                seq,
                bytes = bin.len(),
                error = %e,
                "skipping trail bin that failed to apply"
            );
        }
    }
    if skipped > 0 {
        tracing::warn!(
            workspace_id = %workspace_id,
            doc_id = %doc_id,
            skipped,
            "hydrate/compact skipped corrupt trail bins"
        );
    }
}

/// Base doc for hydrate. A snapshot bin that fails `apply_v1` is skipped (L17)
/// and hydrate continues from the trail alone; a failed apply may already have
/// mutated the doc, so this returns a fresh one. `error!`, not `warn!` — unlike
/// one bad trail bin ([`apply_trail_skip_bad`]) this is real data loss.
///
/// `compact` deliberately stays fail-closed on the snapshot: skipping there
/// would rebuild a trail-only snapshot over the bad row and make the loss
/// permanent. The row is left for an operator; that workspace stops compacting.
fn hydrate_base_skip_bad_snapshot(workspace_id: &str, doc_id: &str, bin: &[u8]) -> Doc {
    let mut doc = Doc::default();
    match apply_v1(&mut doc, bin) {
        Ok(()) => doc,
        Err(e) => {
            tracing::error!(
                workspace_id = %workspace_id,
                doc_id = %doc_id,
                bytes = bin.len(),
                error = %e,
                "snapshot failed to apply; hydrating from the trail only"
            );
            Doc::default()
        }
    }
}

async fn lock_doc_shared(
    conn: &mut PgConnection,
    workspace_id: &str,
    doc_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared(venus_doc_lock_key($1::uuid, $2::uuid))")
        .bind(workspace_id)
        .bind(doc_id)
        .execute(conn)
        .await?;
    Ok(())
}

async fn lock_doc_exclusive(
    conn: &mut PgConnection,
    workspace_id: &str,
    doc_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(venus_doc_lock_key($1::uuid, $2::uuid))")
        .bind(workspace_id)
        .bind(doc_id)
        .execute(conn)
        .await?;
    Ok(())
}

/// Snapshot + trail under one RR snapshot. Apply happens after COMMIT (short tx).
async fn load_snapshot_and_trail(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
) -> Result<(Option<Vec<u8>>, Vec<(i64, Vec<u8>)>), sqlx::Error> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .execute(&mut *tx)
        .await?;
    lock_doc_shared(&mut *tx, workspace_id, doc_id).await?;
    let snap: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid FOR SHARE",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_optional(&mut *tx)
    .await?;
    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT seq, bin FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid ORDER BY seq",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((snap.map(|r| r.0), rows))
}

/// Product pool knobs (P17). `Config::from_env` requires the matching `HUB_DB_*` vars.
#[derive(Debug, Clone)]
pub struct PoolSettings {
    pub max_connections: u32,
    pub min_connections: u32,
    pub acquire_timeout: Duration,
    /// Session `work_mem` (P5). `None` leaves the server default, which is what
    /// tests want; the product path sets it from `HUB_DB_WORK_MEM`.
    pub work_mem: Option<String>,
}

/// Tests: sqlx-shaped defaults (max 10, min 0, 30s). Product path is [`connect_with`].
pub async fn connect(database_url: &str) -> Result<PgPool> {
    connect_with(
        database_url,
        &PoolSettings {
            max_connections: 10,
            min_connections: 0,
            acquire_timeout: Duration::from_secs(30),
            work_mem: None,
        },
    )
    .await
}

pub async fn connect_with(database_url: &str, pool: &PoolSettings) -> Result<PgPool> {
    let mut opts = PgPoolOptions::new()
        .max_connections(pool.max_connections)
        .min_connections(pool.min_connections)
        .acquire_timeout(pool.acquire_timeout);
    // P5: per session, not per statement — a flush is one statement and cannot
    // carry a `SET`. `set_config` because `SET work_mem` takes no bind
    // parameter and this value comes from the environment.
    if let Some(work_mem) = pool.work_mem.clone() {
        opts = opts.after_connect(move |conn, _meta| {
            let work_mem = work_mem.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('work_mem', $1, false)")
                    .bind(work_mem)
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        });
    }
    opts.connect(database_url).await.context("connect postgres")
}

pub async fn migrate(pool: &PgPool) -> Result<()> {
    let mut conn = pool.acquire().await.context("migrate acquire")?;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(SCHEMA_MIGRATE_LOCK)
        .execute(&mut *conn)
        .await
        .context("migrate lock")?;
    let schema = run_schema(&mut conn).await;
    let unlocked = sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SCHEMA_MIGRATE_LOCK)
        .execute(&mut *conn)
        .await;
    match schema {
        Ok(()) => unlocked.context("migrate unlock").map(|_| ()),
        Err(e) => {
            let _ = unlocked;
            Err(e)
        }
    }
}

/// Session lock for `migrate`. Hydrate/compact use `venus_doc_lock_key`
/// (one int8 from two UUIDs), not this bigint (L20).
const SCHEMA_MIGRATE_LOCK: i64 = 859_321_001;

const SCHEMA_SQL: &str = include_str!("schema.sql");
const SCHEMA_TRIGGER_MARK: &str = "-- venus:triggers\n";

/// Tables+function, then the dirty trigger batch if `crdt_update_dirty` is
/// missing or still `FOR EACH ROW` (`tgnewtable` unset — P13 cutover), or if an
/// older volume still has a `crdt_snapshot` dirty trigger (P1 removal).
///
/// Re-running `DROP TRIGGER` / `CREATE TRIGGER` on every start takes
/// AccessExclusive on `crdt_update` and `crdt_snapshot`. Hydrate grabs those
/// tables snapshot-then-update; compact does update-then-snapshot. Either order
/// of `LOCK TABLE` deadlocks one of those live txs (Compose `hub-b` restart).
/// The function is `CREATE OR REPLACE` in the first batch (ROW+STATEMENT body
/// so a live hub can persist during the one cutover). The trigger body is that
/// function; later boots skip the trigger batch.
async fn run_schema(conn: &mut PgConnection) -> Result<()> {
    let Some((tables, triggers)) = SCHEMA_SQL.split_once(SCHEMA_TRIGGER_MARK) else {
        anyhow::bail!("schema.sql missing `-- venus:triggers` batch marker");
    };
    sqlx::raw_sql(tables)
        .execute(&mut *conn)
        .await
        .context("hub schema")?;
    let (statement_update, leftover_snapshot): (i64, i64) = sqlx::query_as(
        "SELECT
           COUNT(*) FILTER (
             WHERE tgname = 'crdt_update_dirty' AND tgnewtable IS NOT NULL
           )::bigint,
           COUNT(*) FILTER (
             WHERE tgname IN (
               'crdt_snapshot_dirty',
               'crdt_snapshot_dirty_ins',
               'crdt_snapshot_dirty_upd'
             )
           )::bigint
         FROM pg_trigger
         WHERE NOT tgisinternal",
    )
    .fetch_one(&mut *conn)
    .await
    .context("migrate trigger probe")?;
    if statement_update >= 1 && leftover_snapshot == 0 {
        return Ok(());
    }
    sqlx::raw_sql(triggers)
        .execute(&mut *conn)
        .await
        .context("hub schema triggers")?;
    Ok(())
}

pub async fn hydrate_doc(pool: &PgPool, workspace_id: &str, doc_id: &str) -> Result<Doc> {
    let (doc, _) = hydrate_with_trail_len(pool, workspace_id, doc_id).await?;
    Ok(doc)
}

/// Same as `hydrate_doc`, plus how many `crdt_update` rows were in that snapshot
/// (so a room can skip compact `COUNT(*)` until the trail is long enough).
pub async fn hydrate_with_trail_len(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
) -> Result<(Doc, u64)> {
    let (snap, updates) = match load_snapshot_and_trail(pool, workspace_id, doc_id).await {
        Ok(pair) => pair,
        Err(e) if is_retryable_tx(&e) => {
            tracing::warn!(error = %e, "hydrate serialization/deadlock; retry once");
            load_snapshot_and_trail(pool, workspace_id, doc_id)
                .await
                .context("hydrate retry")?
        }
        Err(e) => return Err(e).context("hydrate"),
    };
    let trail_len = updates.len() as u64;
    let mut doc = match snap {
        Some(bin) => hydrate_base_skip_bad_snapshot(workspace_id, doc_id, &bin),
        None => Doc::default(),
    };
    apply_trail_skip_bad(&mut doc, workspace_id, doc_id, &updates);
    Ok((doc, trail_len))
}

/// Snapshot + pending updates, encoded as Yjs update v1. Does not write the snapshot.
pub async fn get_doc(pool: &PgPool, workspace_id: &str, doc_id: &str) -> Result<Vec<u8>> {
    let doc = hydrate_doc(pool, workspace_id, doc_id).await?;
    encode_v1(&doc)
}

pub async fn encode_doc(pool: &PgPool, workspace_id: &str, doc_id: &str) -> Result<Vec<u8>> {
    get_doc(pool, workspace_id, doc_id).await
}

pub async fn push_update(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    bin: &[u8],
) -> Result<i64> {
    let (seq,): (i64,) = sqlx::query_as(
        "INSERT INTO crdt_update (workspace_id, doc_id, bin) VALUES ($1::uuid, $2::uuid, $3) RETURNING seq",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .bind(bin)
    .fetch_one(pool)
    .await?;
    Ok(seq)
}

/// The whole batch in one statement (P4). A lone `INSERT` is its own
/// transaction, so a failure commits nothing and `Room::flush` can put the
/// bins back in order. `WITH ORDINALITY` + `ORDER BY` keep `seq` ascending in
/// array order; the dirty trigger upserts `max(seq)` once per statement (P13).
///
/// Bins are `&[u8]` so the caller can pass views of `Bytes` without copying
/// the payload (P15).
pub async fn flush_updates(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    bins: &[&[u8]],
) -> Result<Vec<i64>> {
    if bins.is_empty() {
        return Ok(Vec::new());
    }
    let seqs: Vec<i64> = sqlx::query_scalar(
        "INSERT INTO crdt_update (workspace_id, doc_id, bin)
         SELECT $1::uuid, $2::uuid, bin FROM unnest($3::bytea[]) WITH ORDINALITY AS t(bin, ord)
         ORDER BY ord
         RETURNING seq",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .bind(bins)
    .fetch_all(pool)
    .await
    .context("flush batch insert")?;
    Ok(seqs)
}

/// Merge trail into snapshot off the insert path. Always rebuilds from SQL
/// (snapshot + trail) so a foreign owner's flushed rows are not deleted
/// unseen. New `crdt_update` rows with seq > `max_seq` are left alone.
///
/// P11: the CPU merge runs **after** the read transaction commits, so hydrate is
/// not blocked on `encode_v1`. The write transaction re-checks `MAX(seq)` and
/// skips if the trail moved.
pub struct CompactOutcome {
    pub merged: bool,
    pub trail_len: i64,
}

enum CompactRead {
    Skip {
        trail_len: i64,
    },
    Ready {
        snap: Option<Vec<u8>>,
        trail: Vec<(i64, Vec<u8>)>,
        max_seq: i64,
    },
}

pub async fn compact(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    threshold: i64,
) -> Result<CompactOutcome> {
    compact_with_hooks(
        pool,
        workspace_id,
        doc_id,
        threshold,
        |_| async {},
        |_| async {},
    )
    .await
}

/// Same as [`compact`], with a hook after the read tx commits (P11 tests).
#[doc(hidden)]
pub async fn compact_after_load<F, Fut>(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    threshold: i64,
    after_load: F,
) -> Result<CompactOutcome>
where
    F: FnOnce(i64) -> Fut,
    Fut: Future<Output = ()>,
{
    compact_with_hooks(
        pool,
        workspace_id,
        doc_id,
        threshold,
        after_load,
        |_| async {},
    )
    .await
}

/// Same as [`compact`], with a hook after `now_max` matches and before the
/// snapshot UPSERT (D1: concurrent flush must not rewind `dirty.clock`).
#[doc(hidden)]
pub async fn compact_after_now_max<F, Fut>(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    threshold: i64,
    after_now_max: F,
) -> Result<CompactOutcome>
where
    F: FnOnce(i64) -> Fut,
    Fut: Future<Output = ()>,
{
    compact_with_hooks(
        pool,
        workspace_id,
        doc_id,
        threshold,
        |_| async {},
        after_now_max,
    )
    .await
}

async fn compact_with_hooks<FL, FutL, FN, FutN>(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    threshold: i64,
    after_load: FL,
    after_now_max: FN,
) -> Result<CompactOutcome>
where
    FL: FnOnce(i64) -> FutL,
    FutL: Future<Output = ()>,
    FN: FnOnce(i64) -> FutN,
    FutN: Future<Output = ()>,
{
    match load_compact(pool, workspace_id, doc_id, threshold).await? {
        CompactRead::Skip { trail_len } => Ok(CompactOutcome {
            merged: false,
            trail_len,
        }),
        CompactRead::Ready {
            snap,
            trail,
            max_seq,
        } => {
            after_load(max_seq).await;
            let merged = merge_compact_bin(workspace_id, doc_id, snap.as_deref(), &trail)?;
            commit_compact(pool, workspace_id, doc_id, max_seq, &merged, after_now_max).await
        }
    }
}

async fn load_compact(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    threshold: i64,
) -> Result<CompactRead> {
    let mut tx: Transaction<'_, Postgres> = pool.begin().await.context("compact begin")?;
    lock_doc_exclusive(&mut *tx, workspace_id, doc_id)
        .await
        .context("compact lock")?;

    let (count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_one(&mut *tx)
    .await?;
    if count < threshold {
        tx.commit().await?;
        return Ok(CompactRead::Skip { trail_len: count });
    }

    let (max_seq,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(seq), 0)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_one(&mut *tx)
    .await?;
    if max_seq == 0 {
        tx.commit().await?;
        return Ok(CompactRead::Skip { trail_len: count });
    }

    let snap: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_optional(&mut *tx)
    .await?;

    let trail: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT seq, bin FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid AND seq <= $3 ORDER BY seq",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .bind(max_seq)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(CompactRead::Ready {
        snap: snap.map(|r| r.0),
        trail,
        max_seq,
    })
}

fn merge_compact_bin(
    workspace_id: &str,
    doc_id: &str,
    snap: Option<&[u8]>,
    trail: &[(i64, Vec<u8>)],
) -> Result<Vec<u8>> {
    let mut doc = Doc::default();
    if let Some(bin) = snap {
        // Fail-closed on purpose (L17): a merge that skipped the snapshot would
        // write a trail-only rebuild over it. Hydrate tolerates this; compact
        // must not, so the workspace stops compacting until an operator acts.
        apply_v1(&mut doc, bin).context("compact snapshot apply")?;
    }
    apply_trail_skip_bad(&mut doc, workspace_id, doc_id, trail);
    encode_v1(&doc)
}

async fn commit_compact<F, Fut>(
    pool: &PgPool,
    workspace_id: &str,
    doc_id: &str,
    max_seq: i64,
    merged: &[u8],
    after_now_max: F,
) -> Result<CompactOutcome>
where
    F: FnOnce(i64) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut tx: Transaction<'_, Postgres> = pool.begin().await.context("compact write begin")?;
    lock_doc_exclusive(&mut *tx, workspace_id, doc_id)
        .await
        .context("compact write lock")?;

    let (now_max,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(seq), 0)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_one(&mut *tx)
    .await?;
    if now_max != max_seq {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
        )
        .bind(workspace_id)
        .bind(doc_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::debug!(
            workspace_id = %workspace_id,
            doc_id = %doc_id,
            expected = max_seq,
            actual = now_max,
            "compact skipped; trail moved during merge"
        );
        return Ok(CompactOutcome {
            merged: false,
            trail_len: count,
        });
    }

    after_now_max(max_seq).await;

    let _snap: Option<(Vec<u8>,)> = sqlx::query_as(
        "SELECT bin FROM crdt_snapshot WHERE workspace_id = $1::uuid AND doc_id = $2::uuid FOR UPDATE",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_optional(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO crdt_snapshot (workspace_id, doc_id, bin, clock, updated_at)
         VALUES ($1::uuid, $2::uuid, $3, $4, now())
         ON CONFLICT (workspace_id, doc_id) DO UPDATE
           SET bin = EXCLUDED.bin, clock = EXCLUDED.clock, updated_at = now()",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .bind(merged)
    .bind(max_seq)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        "DELETE FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid AND seq <= $3",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .bind(max_seq)
    .execute(&mut *tx)
    .await?;

    let (remaining,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM crdt_update WHERE workspace_id = $1::uuid AND doc_id = $2::uuid",
    )
    .bind(workspace_id)
    .bind(doc_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(CompactOutcome {
        merged: true,
        trail_len: remaining,
    })
}

pub async fn put_blob(pool: &PgPool, workspace_id: &str, hash: &str, bytes: &[u8]) -> Result<bool> {
    let res = sqlx::query(
        "INSERT INTO blob (workspace_id, hash, bytes) VALUES ($1::uuid, $2, $3)
         ON CONFLICT (workspace_id, hash) DO NOTHING",
    )
    .bind(workspace_id)
    .bind(hash)
    .bind(bytes)
    .execute(pool)
    .await?;
    Ok(res.rows_affected() == 0)
}

pub async fn get_blob(pool: &PgPool, workspace_id: &str, hash: &str) -> Result<Option<Vec<u8>>> {
    let row: Option<(Vec<u8>,)> =
        sqlx::query_as("SELECT bytes FROM blob WHERE workspace_id = $1::uuid AND hash = $2")
            .bind(workspace_id)
            .bind(hash)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// Length only. HEAD must not pull `BYTEA` into the hub.
pub async fn blob_len(pool: &PgPool, workspace_id: &str, hash: &str) -> Result<Option<i64>> {
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT octet_length(bytes)::bigint FROM blob WHERE workspace_id = $1::uuid AND hash = $2",
    )
    .bind(workspace_id)
    .bind(hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.0))
}

pub async fn delete_blob(pool: &PgPool, workspace_id: &str, hash: &str) -> Result<bool> {
    let res = sqlx::query("DELETE FROM blob WHERE workspace_id = $1::uuid AND hash = $2")
        .bind(workspace_id)
        .bind(hash)
        .execute(pool)
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn default_page_export(pool: &PgPool, workspace_id: &str) -> Result<Vec<u8>> {
    encode_doc(pool, workspace_id, PAGE_DOC_ID).await
}
