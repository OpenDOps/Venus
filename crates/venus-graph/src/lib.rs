//! Venus graph worker. Step-recon pins. Step-schema migrates SurrealDB and the
//! Postgres allocation map. The client speaks WebSocket. It does not embed RocksDB.
//! SPDX-License-Identifier: MIT OR Apache-2.0

mod allocation;
mod migrate;
mod schema;

pub use allocation::{list_allocation, migrate_allocation, AllocationRow};
pub use migrate::migrate_surreal;
pub use schema::{accept_graph_schema, accept_search_schema, graph_surql, search_surql};

/// Docker image for the graph process and every search node.
/// Channel `v2`, patch pinned. Search nodes load the search schema only.
pub const SURREAL_IMAGE: &str = "surrealdb/surrealdb:v2.7.0";

/// crates.io `surrealdb` client. Same 2.x line as [`SURREAL_IMAGE`].
pub const SURREAL_CLIENT: &str = "2.7.0";

/// Client features. `protocol-ws` plus `rustls`. No embedded storage engine.
pub const SURREAL_CLIENT_FEATURES: &[&str] = &["protocol-ws", "rustls"];

/// This board: two shards, one extra copy, two search nodes.
pub const DEV_SHARD_COUNT: u32 = 2;
pub const DEV_REPLICA_COUNT: u32 = 1;
pub const DEV_SEARCH_NODES: u32 = 2;

/// HA default, step 7: a third search node. `replica_count` stays 1.
pub const HA_SEARCH_NODES: u32 = 3;
pub const HA_REPLICA_COUNT: u32 = 1;

/// Flush `jobs` primary key. One row per wiki. `graph_jobs` is step 6.
pub const FLUSH_JOBS_PK: &str = "workspace_id";

/// Flush `jobs.reason` check. Snapshotter reasons only.
pub const FLUSH_JOBS_REASONS: &[&str] = &["idle", "flush", "lease"];

/// M0 wiki. Same UUID the hub uses for `/collaboration/:workspace_id`.
pub const WORKSPACE_ID: &str = "77e4a2b1-8b40-5979-a73c-fd4477216d00";

/// Search node ids for this board, and the host URL of each.
pub const SEARCH_NODES: &[(&str, &str)] = &[
    ("0", "http://127.0.0.1:8001"),
    ("1", "http://127.0.0.1:8002"),
];

/// Remote WebSocket client. Present only when `protocol-ws` is enabled.
pub type RemoteClient = surrealdb::Surreal<surrealdb::engine::remote::ws::Client>;

/// Name of [`RemoteClient`]'s connection type. Recon asserts this compiles.
pub fn remote_client_name() -> &'static str {
    std::any::type_name::<surrealdb::engine::remote::ws::Client>()
}
