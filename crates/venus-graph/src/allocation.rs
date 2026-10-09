//! Wiki layout in Postgres. Step 3 `search_*` rows are copied into `layout_db`
//! and those tables are dropped. A second call does not change the map.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Context;
use sqlx::PgPool;
use surrealastic::{homes, shard_set_id, Member, NodeState};

use crate::{DEV_REPLICA_COUNT, DEV_SHARD_COUNT, WORKSPACE_ID};

/// Graph node, then the two search nodes. Zones differ so placement can use both.
const NODES: &[(&str, &str, &str, &str)] = &[
    ("0", "graph", "http://127.0.0.1:28730", "0"),
    ("1", "search", "http://127.0.0.1:28731", "1"),
    ("2", "search", "http://127.0.0.1:28732", "2"),
];

pub const LAYOUT_EPOCH: i32 = 1;

pub fn db_id(ws: &str) -> String {
    format!("search:{ws}")
}

pub fn commit_set_id(ws: &str) -> String {
    format!("graph:{ws}")
}

pub fn lease_name(ws: &str) -> String {
    format!("writer:{ws}")
}

pub fn graph_database(ws: &str) -> String {
    ws.to_string()
}

pub fn shard_database(ws: &str, epoch: i32, shard: i32) -> String {
    format!("{ws}_{epoch}_{shard}")
}

/// One wiki's layout row and how many sets it has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiLayout {
    pub db_id: String,
    pub commit_set: String,
    pub epoch: i32,
    pub shard_count: i32,
    pub replica_count: i32,
    pub shard_ack: i32,
    pub search_sets: i64,
    pub commit_copies: i64,
}

/// Run the surrealastic migration, carry `search_cluster` into `layout_db`, drop
/// the step 3 tables, and seed the fixture wiki when nothing was carried.
pub async fn migrate_allocation(database_url: &str) -> anyhow::Result<()> {
    surrealastic::migrate(database_url)
        .await
        .context("repl map")?;
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    let carried = read_legacy(&pool).await?;
    drop_legacy(&pool).await?;
    seed_nodes(&pool).await?;
    if carried.is_empty() {
        ensure_wiki_pool(
            &pool,
            WORKSPACE_ID,
            i32::try_from(DEV_SHARD_COUNT).expect("shard_count"),
            i32::try_from(DEV_REPLICA_COUNT).expect("replica_count"),
        )
        .await?;
    } else {
        for (ws, shard_count, replica_count) in carried {
            ensure_wiki_pool(&pool, &ws, shard_count, replica_count).await?;
        }
    }
    crate::jobs::ensure_graph_jobs(&pool).await?;
    Ok(())
}

/// Insert a wiki that is not already in `layout_db`. Existing rows stay as they are.
pub async fn ensure_wiki(
    database_url: &str,
    ws: &str,
    shard_count: i32,
    replica_count: i32,
) -> anyhow::Result<()> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    seed_nodes(&pool).await?;
    ensure_wiki_pool(&pool, ws, shard_count, replica_count).await
}

/// Point a node at a different SurrealDB URL. Used when the graph port is not :28730.
pub async fn set_node_url(database_url: &str, node_id: &str, url: &str) -> anyhow::Result<()> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    sqlx::query("UPDATE repl_node SET url = $2 WHERE node_id = $1")
        .bind(node_id)
        .bind(url)
        .execute(&pool)
        .await
        .context("set node url")?;
    Ok(())
}

pub async fn wiki_layout(database_url: &str, ws: &str) -> anyhow::Result<WikiLayout> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    let id = db_id(ws);
    let row = sqlx::query_as::<_, (String, String, i32, i32, i32, i32)>(
        "SELECT db_id, commit_set, epoch, shard_count, replica_count, shard_ack
         FROM layout_db WHERE db_id = $1",
    )
    .bind(&id)
    .fetch_optional(&pool)
    .await
    .context("load layout_db")?
    .context("layout_db row missing")?;
    let search_sets = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM repl_set WHERE set_id LIKE $1 AND pool = 'search'",
    )
    .bind(format!("{id}:%"))
    .fetch_one(&pool)
    .await
    .context("count search sets")?;
    let commit_copies =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM repl_copy WHERE set_id = $1")
            .bind(&row.1)
            .fetch_one(&pool)
            .await
            .context("count commit copies")?;
    Ok(WikiLayout {
        db_id: row.0,
        commit_set: row.1,
        epoch: row.2,
        shard_count: row.3,
        replica_count: row.4,
        shard_ack: row.5,
        search_sets,
        commit_copies,
    })
}

/// `(set_id, node_id)` pairs for one wiki, ordered.
pub async fn copy_pairs(database_url: &str, ws: &str) -> anyhow::Result<Vec<(String, String)>> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    let like = format!("{}%", db_id(ws));
    let commit = commit_set_id(ws);
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT set_id, node_id FROM repl_copy
         WHERE set_id = $1 OR set_id LIKE $2
         ORDER BY set_id, node_id",
    )
    .bind(commit)
    .bind(like)
    .fetch_all(&pool)
    .await
    .context("list copies")?;
    Ok(rows)
}

pub async fn legacy_table_names(database_url: &str) -> anyhow::Result<Vec<String>> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public'
           AND table_name IN ('search_node', 'search_allocation', 'search_cluster')
         ORDER BY table_name",
    )
    .fetch_all(&pool)
    .await
    .context("legacy tables")?;
    Ok(rows)
}

/// Stable text of the map. A second migrate must produce the same text.
pub async fn map_snapshot(database_url: &str) -> anyhow::Result<Vec<String>> {
    let pool = PgPool::connect(database_url)
        .await
        .context("connect allocation postgres")?;
    let mut lines = Vec::new();
    let layouts = sqlx::query_as::<_, (String, String, i32, i32, i32, i32)>(
        "SELECT db_id, commit_set, epoch, shard_count, replica_count, shard_ack
         FROM layout_db ORDER BY db_id",
    )
    .fetch_all(&pool)
    .await
    .context("snapshot layout")?;
    for row in layouts {
        lines.push(format!(
            "layout {} {} epoch {} shards {} replica {} ack {}",
            row.0, row.1, row.2, row.3, row.4, row.5
        ));
    }
    let sets = sqlx::query_as::<_, (String, String, String, String, i32, i32, String)>(
        "SELECT set_id, pool, namespace, database, copies, ack, lease
         FROM repl_set ORDER BY set_id",
    )
    .fetch_all(&pool)
    .await
    .context("snapshot sets")?;
    for row in sets {
        lines.push(format!(
            "set {} {} {} {} copies {} ack {} lease {}",
            row.0, row.1, row.2, row.3, row.4, row.5, row.6
        ));
    }
    let copies = sqlx::query_as::<_, (String, String, String, i64)>(
        "SELECT set_id, node_id, state, applied_lsn FROM repl_copy ORDER BY set_id, node_id",
    )
    .fetch_all(&pool)
    .await
    .context("snapshot copies")?;
    for row in copies {
        lines.push(format!("copy {} {} {} {}", row.0, row.1, row.2, row.3));
    }
    let nodes = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT node_id, pool, url, zone FROM repl_node ORDER BY node_id",
    )
    .fetch_all(&pool)
    .await
    .context("snapshot nodes")?;
    for row in nodes {
        lines.push(format!("node {} {} {} {}", row.0, row.1, row.2, row.3));
    }
    Ok(lines)
}

async fn read_legacy(pool: &PgPool) -> anyhow::Result<Vec<(String, i32, i32)>> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = 'search_cluster'
         )",
    )
    .fetch_one(pool)
    .await
    .context("search_cluster exists")?;
    if !exists {
        return Ok(Vec::new());
    }
    let rows = sqlx::query_as::<_, (String, i32, i32)>(
        "SELECT workspace_id::text, shard_count, replica_count FROM search_cluster",
    )
    .fetch_all(pool)
    .await
    .context("read search_cluster")?;
    Ok(rows)
}

async fn drop_legacy(pool: &PgPool) -> anyhow::Result<()> {
    for table in ["search_allocation", "search_node", "search_cluster"] {
        sqlx::raw_sql(&format!("DROP TABLE IF EXISTS {table}"))
            .execute(pool)
            .await
            .with_context(|| format!("drop {table}"))?;
    }
    Ok(())
}

async fn seed_nodes(pool: &PgPool) -> anyhow::Result<()> {
    for (node_id, pool_name, url, zone) in NODES {
        sqlx::query(
            "INSERT INTO repl_node (node_id, pool, url, zone, weight, state)
             VALUES ($1, $2, $3, $4, 1, 'up')
             ON CONFLICT (node_id) DO NOTHING",
        )
        .bind(*node_id)
        .bind(*pool_name)
        .bind(*url)
        .bind(*zone)
        .execute(pool)
        .await
        .context("seed repl_node")?;
    }
    Ok(())
}

async fn ensure_wiki_pool(
    pool: &PgPool,
    ws: &str,
    shard_count: i32,
    replica_count: i32,
) -> anyhow::Result<()> {
    if shard_count < 1 {
        anyhow::bail!("shard_count {shard_count}");
    }
    let id = db_id(ws);
    let commit = commit_set_id(ws);
    let lease = lease_name(ws);
    sqlx::query(
        "INSERT INTO repl_lease (name, holder, fence, lease_until)
         VALUES ($1, '', 0, '-infinity')
         ON CONFLICT (name) DO NOTHING",
    )
    .bind(&lease)
    .execute(pool)
    .await
    .context("seed lease")?;
    sqlx::query(
        "INSERT INTO layout_db
            (db_id, commit_set, epoch, shard_count, shard_count_next, replica_count, shard_ack)
         VALUES ($1, $2, $3, $4, NULL, $5, 1)
         ON CONFLICT (db_id) DO NOTHING",
    )
    .bind(&id)
    .bind(&commit)
    .bind(LAYOUT_EPOCH)
    .bind(shard_count)
    .bind(replica_count)
    .execute(pool)
    .await
    .context("seed layout_db")?;
    insert_set(
        pool,
        &commit,
        "graph",
        "graph",
        &graph_database(ws),
        1,
        1,
        &lease,
    )
    .await?;
    for node in homes(&commit, 1, &members("graph")) {
        insert_copy(pool, &commit, &node).await?;
    }
    let copies = u32::try_from(replica_count + 1).unwrap_or(1);
    for shard in 0..shard_count {
        let set = shard_set_id(&id, LAYOUT_EPOCH, shard);
        insert_set(
            pool,
            &set,
            "search",
            "search",
            &shard_database(ws, LAYOUT_EPOCH, shard),
            i32::try_from(copies).unwrap_or(1),
            1,
            &lease,
        )
        .await?;
        for node in homes(&set, copies, &members("search")) {
            insert_copy(pool, &set, &node).await?;
        }
    }
    Ok(())
}

fn members(pool_name: &str) -> Vec<Member> {
    NODES
        .iter()
        .filter(|(_, pool, _, _)| *pool == pool_name)
        .map(|(node_id, _, _, zone)| Member {
            node_id: (*node_id).to_string(),
            zone: (*zone).to_string(),
            weight: 1,
            state: NodeState::Up,
        })
        .collect()
}

async fn insert_set(
    pool: &PgPool,
    set_id: &str,
    pool_name: &str,
    namespace: &str,
    database: &str,
    copies: i32,
    ack: i32,
    lease: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO repl_set (set_id, pool, namespace, database, copies, ack, lease)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         ON CONFLICT (set_id) DO NOTHING",
    )
    .bind(set_id)
    .bind(pool_name)
    .bind(namespace)
    .bind(database)
    .bind(copies)
    .bind(ack)
    .bind(lease)
    .execute(pool)
    .await
    .context("seed repl_set")?;
    Ok(())
}

async fn insert_copy(pool: &PgPool, set_id: &str, node_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO repl_copy (set_id, node_id, state, applied_lsn)
         VALUES ($1, $2, 'in_sync', 0)
         ON CONFLICT (set_id, node_id) DO NOTHING",
    )
    .bind(set_id)
    .bind(node_id)
    .execute(pool)
    .await
    .context("seed repl_copy")?;
    Ok(())
}
