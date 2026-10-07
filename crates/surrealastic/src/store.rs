//! Postgres map: leases, nodes, sets, copies. The log does not live here.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Context;
use sqlx::PgPool;

use crate::place::Health;

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS repl_lease (
    name TEXT PRIMARY KEY,
    holder TEXT NOT NULL,
    fence BIGINT NOT NULL,
    lease_until TIMESTAMPTZ NOT NULL
);

CREATE TABLE IF NOT EXISTS repl_node (
    node_id TEXT PRIMARY KEY,
    pool TEXT NOT NULL CHECK (pool IN ('graph', 'search')),
    url TEXT NOT NULL,
    zone TEXT NOT NULL,
    weight INT NOT NULL DEFAULT 1,
    state TEXT NOT NULL CHECK (state IN ('up', 'suspect', 'down', 'draining')),
    state_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS repl_set (
    set_id TEXT PRIMARY KEY,
    pool TEXT NOT NULL CHECK (pool IN ('graph', 'search')),
    namespace TEXT NOT NULL,
    database TEXT NOT NULL,
    copies INT NOT NULL,
    ack INT NOT NULL,
    lease TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS repl_copy (
    set_id TEXT NOT NULL REFERENCES repl_set (set_id),
    node_id TEXT NOT NULL REFERENCES repl_node (node_id),
    state TEXT NOT NULL CHECK (state IN ('joining', 'in_sync', 'lagging')),
    applied_lsn BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (set_id, node_id)
);

CREATE TABLE IF NOT EXISTS layout_db (
    db_id TEXT PRIMARY KEY,
    commit_set TEXT NOT NULL,
    epoch INT NOT NULL,
    shard_count INT NOT NULL,
    shard_count_next INT,
    replica_count INT NOT NULL,
    shard_ack INT NOT NULL
);

"#;

const FUNCTION: &str = r#"
CREATE OR REPLACE FUNCTION repl_map_notify() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    PERFORM pg_notify('repl_map', TG_TABLE_NAME);
    RETURN NULL;
END;
$$;
"#;

const TRIGGERS: &str = r#"
DROP TRIGGER IF EXISTS repl_node_notify ON repl_node;
CREATE TRIGGER repl_node_notify
    AFTER INSERT OR UPDATE OR DELETE ON repl_node
    FOR EACH STATEMENT EXECUTE FUNCTION repl_map_notify();

DROP TRIGGER IF EXISTS repl_set_notify ON repl_set;
CREATE TRIGGER repl_set_notify
    AFTER INSERT OR UPDATE OR DELETE ON repl_set
    FOR EACH STATEMENT EXECUTE FUNCTION repl_map_notify();

DROP TRIGGER IF EXISTS repl_copy_notify ON repl_copy;
CREATE TRIGGER repl_copy_notify
    AFTER INSERT OR UPDATE OR DELETE ON repl_copy
    FOR EACH STATEMENT EXECUTE FUNCTION repl_map_notify();

DROP TRIGGER IF EXISTS layout_db_notify ON layout_db;
CREATE TRIGGER layout_db_notify
    AFTER INSERT OR UPDATE OR DELETE ON layout_db
    FOR EACH STATEMENT EXECUTE FUNCTION repl_map_notify();
"#;

/// Create the layer tables. Idempotent. Does not touch Venus `search_*` tables.
pub async fn migrate(pool: &PgPool) -> anyhow::Result<()> {
    for statement in split_sql(DDL) {
        sqlx::raw_sql(&statement)
            .execute(pool)
            .await
            .with_context(|| format!("repl ddl: {statement}"))?;
    }
    sqlx::raw_sql(FUNCTION)
        .execute(pool)
        .await
        .context("repl notify function")?;
    for statement in split_sql(TRIGGERS) {
        sqlx::raw_sql(&statement)
            .execute(pool)
            .await
            .with_context(|| format!("repl trigger: {statement}"))?;
    }
    Ok(())
}

fn split_sql(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| format!("{s};"))
        .collect()
}

pub async fn ensure_lease(pool: &PgPool, name: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO repl_lease (name, holder, fence, lease_until)
         VALUES ($1, '', 0, '-infinity')
         ON CONFLICT (name) DO NOTHING",
    )
    .bind(name)
    .execute(pool)
    .await
    .context("ensure lease")?;
    Ok(())
}

/// One statement. No row: someone else holds the lease.
pub async fn claim_lease(
    pool: &PgPool,
    name: &str,
    holder: &str,
    lease_secs: f64,
) -> anyhow::Result<Option<(i64, f64)>> {
    let row = sqlx::query_as::<_, (i64, f64)>(
        "UPDATE repl_lease
            SET fence = fence + CASE WHEN holder = $2 THEN 0 ELSE 1 END,
                holder = $2,
                lease_until = now() + make_interval(secs => $3)
          WHERE name = $1 AND (holder = $2 OR lease_until < now())
          RETURNING fence, EXTRACT(EPOCH FROM lease_until)::float8",
    )
    .bind(name)
    .bind(holder)
    .bind(lease_secs)
    .fetch_optional(pool)
    .await
    .context("claim lease")?;
    Ok(row)
}

#[derive(Debug, Clone)]
pub struct SetRow {
    pub set_id: String,
    pub pool: String,
    pub namespace: String,
    pub database: String,
    pub copies: i32,
    pub ack: i32,
    pub lease: String,
}

pub async fn load_set(pool: &PgPool, set_id: &str) -> anyhow::Result<SetRow> {
    sqlx::query_as::<_, (String, String, String, String, i32, i32, String)>(
        "SELECT set_id, pool, namespace, database, copies, ack, lease
         FROM repl_set WHERE set_id = $1",
    )
    .bind(set_id)
    .fetch_optional(pool)
    .await
    .context("load set")?
    .map(|(set_id, pool, namespace, database, copies, ack, lease)| SetRow {
        set_id,
        pool,
        namespace,
        database,
        copies,
        ack,
        lease,
    })
    .context("unknown set")
}

#[derive(Debug, Clone)]
pub struct CopyRow {
    pub node_id: String,
    pub url: String,
    pub node_state: String,
    pub state: String,
    pub applied_lsn: i64,
}

pub async fn load_copies(pool: &PgPool, set_id: &str) -> anyhow::Result<Vec<CopyRow>> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64)>(
        "SELECT c.node_id, n.url, n.state, c.state, c.applied_lsn
         FROM repl_copy c
         JOIN repl_node n ON n.node_id = c.node_id
         WHERE c.set_id = $1
         ORDER BY c.node_id",
    )
    .bind(set_id)
    .fetch_all(pool)
    .await
    .context("load copies")?;
    Ok(rows
        .into_iter()
        .map(|(node_id, url, node_state, state, applied_lsn)| CopyRow {
            node_id,
            url,
            node_state,
            state,
            applied_lsn,
        })
        .collect())
}

/// Writes `repl_copy` only when `state` changes. Positions stay on the copy.
pub async fn set_copy_state(
    pool: &PgPool,
    set_id: &str,
    node_id: &str,
    state: &str,
) -> anyhow::Result<bool> {
    let changed = sqlx::query(
        "UPDATE repl_copy SET state = $3
          WHERE set_id = $1 AND node_id = $2 AND state <> $3",
    )
    .bind(set_id)
    .bind(node_id)
    .bind(state)
    .execute(pool)
    .await
    .context("set copy state")?
    .rows_affected();
    Ok(changed > 0)
}

pub async fn copy_fingerprint(pool: &PgPool, set_id: &str) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT node_id, state, applied_lsn, xmin::text
         FROM repl_copy WHERE set_id = $1 ORDER BY node_id",
    )
    .bind(set_id)
    .fetch_all(pool)
    .await
    .context("copy fingerprint")?;
    Ok(rows
        .into_iter()
        .map(|(n, s, l, x)| format!("{n}:{s}:{l}:{x}"))
        .collect())
}

#[derive(Debug, Clone)]
pub struct LayoutRow {
    pub db_id: String,
    pub commit_set: String,
    pub epoch: i32,
    pub shard_count: i32,
    pub shard_count_next: Option<i32>,
    pub replica_count: i32,
    pub shard_ack: i32,
}

pub async fn upsert_layout(pool: &PgPool, row: &LayoutRow) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO layout_db
            (db_id, commit_set, epoch, shard_count, shard_count_next, replica_count, shard_ack)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (db_id) DO UPDATE SET
            commit_set = EXCLUDED.commit_set,
            epoch = EXCLUDED.epoch,
            shard_count = EXCLUDED.shard_count,
            shard_count_next = EXCLUDED.shard_count_next,
            replica_count = EXCLUDED.replica_count,
            shard_ack = EXCLUDED.shard_ack",
    )
    .bind(&row.db_id)
    .bind(&row.commit_set)
    .bind(row.epoch)
    .bind(row.shard_count)
    .bind(row.shard_count_next)
    .bind(row.replica_count)
    .bind(row.shard_ack)
    .execute(pool)
    .await
    .context("upsert layout")?;
    Ok(())
}

pub async fn load_layout(pool: &PgPool, db_id: &str) -> anyhow::Result<LayoutRow> {
    sqlx::query_as::<_, (String, String, i32, i32, Option<i32>, i32, i32)>(
        "SELECT db_id, commit_set, epoch, shard_count, shard_count_next, replica_count, shard_ack
         FROM layout_db WHERE db_id = $1",
    )
    .bind(db_id)
    .fetch_optional(pool)
    .await
    .context("load layout")?
    .map(
        |(db_id, commit_set, epoch, shard_count, shard_count_next, replica_count, shard_ack)| {
            LayoutRow {
                db_id,
                commit_set,
                epoch,
                shard_count,
                shard_count_next,
                replica_count,
                shard_ack,
            }
        },
    )
    .context("unknown layout")
}

pub fn health_of(set: &SetRow, copies: &[CopyRow]) -> Health {
    let in_sync_on_up = copies
        .iter()
        .filter(|c| c.state == "in_sync" && c.node_state == "up")
        .count() as u32;
    crate::place::set_health(set.copies as u32, set.ack as u32, in_sync_on_up)
}

