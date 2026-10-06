//! Postgres map for the search cluster. Not flush `jobs`.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Context;
use sqlx::PgPool;

use crate::{DEV_REPLICA_COUNT, DEV_SHARD_COUNT, SEARCH_NODES, WORKSPACE_ID};

const DDL: &str = r#"
CREATE TABLE IF NOT EXISTS search_cluster (
    workspace_id UUID PRIMARY KEY,
    shard_count INT NOT NULL,
    replica_count INT NOT NULL
);

CREATE TABLE IF NOT EXISTS search_node (
    node_id TEXT PRIMARY KEY,
    url TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS search_allocation (
    workspace_id UUID NOT NULL,
    shard INT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('primary', 'replica')),
    node_id TEXT NOT NULL REFERENCES search_node (node_id),
    state TEXT NOT NULL CHECK (state IN ('in_sync', 'stale', 'down')),
    synced_sha TEXT,
    PRIMARY KEY (workspace_id, shard, role, node_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS search_allocation_one_primary
    ON search_allocation (workspace_id, shard)
    WHERE role = 'primary';
"#;

/// One allocation row. `role` is `primary` or `replica`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllocationRow {
    pub shard: i32,
    pub role: String,
    pub node_id: String,
}

/// Create the map if needed and seed this board's two nodes, crossed.
/// A second call does not insert another primary.
pub async fn migrate_allocation(database_url: &str) -> anyhow::Result<()> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    sqlx::raw_sql(DDL)
        .execute(&pool)
        .await
        .context("search allocation ddl")?;

    for (node_id, url) in SEARCH_NODES {
        sqlx::query(
            "INSERT INTO search_node (node_id, url) VALUES ($1, $2)
             ON CONFLICT (node_id) DO NOTHING",
        )
        .bind(*node_id)
        .bind(*url)
        .execute(&pool)
        .await
        .context("seed search_node")?;
    }

    sqlx::query(
        "INSERT INTO search_cluster (workspace_id, shard_count, replica_count)
         VALUES ($1::uuid, $2, $3)
         ON CONFLICT (workspace_id) DO NOTHING",
    )
    .bind(WORKSPACE_ID)
    .bind(i32::try_from(DEV_SHARD_COUNT).expect("shard_count"))
    .bind(i32::try_from(DEV_REPLICA_COUNT).expect("replica_count"))
    .execute(&pool)
    .await
    .context("seed search_cluster")?;

    for (shard, role, node_id) in [
        (0_i32, "primary", "0"),
        (0, "replica", "1"),
        (1, "primary", "1"),
        (1, "replica", "0"),
    ] {
        sqlx::query(
            "INSERT INTO search_allocation
                (workspace_id, shard, role, node_id, state, synced_sha)
             VALUES ($1::uuid, $2, $3, $4, 'in_sync', NULL)
             ON CONFLICT (workspace_id, shard, role, node_id) DO NOTHING",
        )
        .bind(WORKSPACE_ID)
        .bind(shard)
        .bind(role)
        .bind(node_id)
        .execute(&pool)
        .await
        .context("seed search_allocation")?;
    }
    Ok(())
}

/// Allocation rows for [`WORKSPACE_ID`], ordered by shard then role.
pub async fn list_allocation(database_url: &str) -> anyhow::Result<Vec<AllocationRow>> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    let rows = sqlx::query_as::<_, (i32, String, String)>(
        "SELECT shard, role, node_id
         FROM search_allocation
         WHERE workspace_id = $1::uuid
         ORDER BY shard, role",
    )
    .bind(WORKSPACE_ID)
    .fetch_all(&pool)
    .await
    .context("list search_allocation")?;
    Ok(rows
        .into_iter()
        .map(|(shard, role, node_id)| AllocationRow {
            shard,
            role,
            node_id,
        })
        .collect())
}
