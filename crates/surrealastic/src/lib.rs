//! Surrealastic. Replication and layout for any SurrealDB database.
//! No page, doc, or field hash.
//! SPDX-License-Identifier: MIT OR Apache-2.0

mod body;
mod cluster;
mod config;
mod guard;
mod hooks;
mod layout;
mod link;
mod place;
mod store;
mod writer;

pub use body::{accept_sql, Body};
pub use cluster::{migrate, Cluster};
pub use config::Config;
pub use guard::{ApplyReply, ApplyStatus, Entry};
pub use hooks::{BoxFut, Owner};
pub use layout::{
    log_tail_start, pack_items, remove_database, shard_for, shard_set_id, shards_for, Item, Layout,
    PartialShard, ReadOut, ShardLag,
};
pub use link::dial_wss;
pub use place::{homes, set_health, Health, Member, NodeState};
pub use store::{LayoutRow, SetRow};
pub use writer::Writer;

/// Guarded write against one copy. The writer fans this out. Tests use it for
/// the replies a single copy gives: `already`, `gap`, `divergent`.
pub async fn apply(
    cluster: &Cluster,
    node_id: &str,
    url: &str,
    namespace: &str,
    database: &str,
    entry: &Entry,
) -> anyhow::Result<ApplyReply> {
    let conn = cluster.conn(node_id, url).await?;
    link::apply(&conn, namespace, database, entry).await
}

pub async fn log_ids(
    cluster: &Cluster,
    node_id: &str,
    url: &str,
    namespace: &str,
    database: &str,
    from: i64,
    to: i64,
) -> anyhow::Result<Vec<i64>> {
    let conn = cluster.conn(node_id, url).await?;
    link::log_ids(&conn, namespace, database, from, to).await
}

pub async fn explain_log(
    cluster: &Cluster,
    node_id: &str,
    url: &str,
    namespace: &str,
    database: &str,
    from: i64,
    to: i64,
) -> anyhow::Result<String> {
    let conn = cluster.conn(node_id, url).await?;
    link::explain_log(&conn, namespace, database, from, to).await
}

pub async fn raw(
    cluster: &Cluster,
    node_id: &str,
    url: &str,
    namespace: &str,
    database: &str,
    sql: &str,
) -> anyhow::Result<String> {
    let conn = cluster.conn(node_id, url).await?;
    link::raw(&conn, namespace, database, sql).await
}
