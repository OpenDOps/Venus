//! ss-cluster step-recon.
//! Binary: search nodes are `surrealdb/surrealdb:v2` and do not create namespace `graph`.
//! HA default: exit 7 is three nodes and `replica_count = 1` (step 11).
//! Coverage: every default in scale.md has a step; no designed part is a non-goal.
//! Flush `jobs` stay the snapshotter queue.
//! Copies: every copy of a replica set is equal. No leader copy, no promotion.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use venus_graph::{
    remote_client_name, DEV_REPLICA_COUNT, DEV_SEARCH_NODES, DEV_SHARD_COUNT, FLUSH_JOBS_PK,
    FLUSH_JOBS_REASONS, HA_REPLICA_COUNT, HA_SEARCH_NODES, SURREAL_CLIENT, SURREAL_CLIENT_FEATURES,
    SURREAL_IMAGE,
};

const PLAN: &str = include_str!("../../../docs/design/SemanticGraph/search-scale/plan.md");
const README: &str = include_str!("../../../docs/design/SemanticGraph/search-scale/README.md");
const SCALE: &str = include_str!("../../../docs/design/SemanticGraph/scale.md");
const SCHEMA: &str = include_str!("../../venus-hub/src/schema.sql");
const CRATE: &str = include_str!("../Cargo.toml");
const LIB: &str = include_str!("../src/lib.rs");

fn between<'a>(doc: &'a str, start: &str, end: &str) -> &'a str {
    let rest = match doc.split_once(start) {
        Some((_, rest)) => rest,
        None => panic!("missing {start}"),
    };
    match rest.split_once(end) {
        Some((body, _)) => body,
        None => panic!("missing {end} after {start}"),
    }
}

/// Search nodes use the pinned v2 image and the client does not embed RocksDB.
/// The plan forbids namespace `graph` on a search volume.
#[test]
fn binary() {
    let recon = between(PLAN, "### 1. step-recon", "### 2. step-compose");
    assert!(
        recon.contains(SURREAL_IMAGE),
        "step-recon must pin {SURREAL_IMAGE}"
    );
    assert!(
        recon.contains("do not create namespace `graph`"),
        "step-recon must forbid namespace graph on a search node"
    );
    assert!(
        recon.contains("Do not enable `kv-rocksdb`"),
        "step-recon must keep the Rust client off RocksDB"
    );

    let constraints = between(PLAN, "## Constraints", "## Steps summary");
    assert!(
        constraints.contains(SURREAL_IMAGE) && constraints.contains("namespace `search` only"),
        "constraints must name the pinned image and the search schema"
    );

    let node = between(README, "## What runs on a search node", "## Cluster");
    assert!(
        node.contains(SURREAL_IMAGE),
        "search-node section must name {SURREAL_IMAGE}"
    );
    assert!(
        node.contains("Namespace `search`") && node.contains("Namespace `graph`"),
        "a search node loads namespace search and not namespace graph"
    );
    assert!(
        node.contains("SurrealDB does not ship a search-only build"),
        "a search node is the full server binary"
    );

    assert!(
        CRATE.contains(&format!("version = \"{SURREAL_CLIENT}\"")),
        "Cargo.toml must pin surrealdb {SURREAL_CLIENT}"
    );
    assert!(
        CRATE.contains("default-features = false"),
        "surrealdb must not take default features"
    );
    for feature in SURREAL_CLIENT_FEATURES {
        assert!(
            CRATE.contains(&format!("\"{feature}\"")),
            "Cargo.toml must enable {feature}"
        );
    }
    for forbidden in [
        "kv-rocksdb",
        "kv-mem",
        "kv-tikv",
        "kv-surrealkv",
        "kv-surrealcs",
    ] {
        assert!(
            !CRATE.contains(forbidden),
            "Cargo.toml must not enable {forbidden}"
        );
    }
    assert!(
        !LIB.contains("DEFINE INDEX") && !LIB.contains("BM25"),
        "step-recon must not add a full-text index to the graph schema"
    );
    assert!(
        remote_client_name().contains("Client"),
        "protocol-ws client type must compile"
    );
}

/// Exit 7 is three search nodes and `replica_count = 1`.
#[test]
fn ha_default() {
    let exit = between(PLAN, "## Exit", "## Non-goals");
    assert!(
        exit.contains("third search node")
            && exit.contains("`replica_count` still 1")
            && exit.contains("`shard_count` still 2"),
        "exit 7 must add a third node and keep replica_count 1"
    );
    assert!(
        !exit.contains("replica_count = 2") && !exit.contains("`replica_count` still 2"),
        "exit must not make replica_count 2 the layout"
    );

    let non_goals = between(PLAN, "## Non-goals", "## Constraints");
    assert!(
        non_goals.contains("`replica_count = 2`") && non_goals.contains("Not the HA default"),
        "replica_count 2 stays a non-goal"
    );

    let third = between(PLAN, "### 11. step-third-node", "### 12. step-graph-copies");
    assert!(
        third.contains("`shard_count` and `replica_count` stay 2 and 1"),
        "step 11 keeps shard_count 2 and replica_count 1"
    );
    assert!(
        third.contains("Set `replica_count` to 2."),
        "step 11 must refuse replica_count 2"
    );

    let ha = between(README, "## High availability", "## Rebuild");
    assert!(
        ha.contains("**3**") && ha.contains("**1**") && ha.contains("**HA default.**"),
        "README HA default is three nodes and replica_count 1"
    );
    assert!(
        ha.contains("Not the default."),
        "README must say replica_count 2 is not the default"
    );

    assert_eq!(DEV_SHARD_COUNT, 2);
    assert_eq!(DEV_REPLICA_COUNT, 1);
    assert_eq!(DEV_SEARCH_NODES, 2);
    assert_eq!(HA_SEARCH_NODES, 3);
    assert_eq!(HA_REPLICA_COUNT, 1);
    assert_ne!(HA_REPLICA_COUNT, 2);
}

/// Flush `jobs` is one row per wiki. Reasons are idle, flush, lease. No `graph_jobs` yet.
#[test]
fn flush_jobs() {
    let jobs = between(SCHEMA, "CREATE TABLE IF NOT EXISTS jobs (", ");");
    assert!(
        jobs.contains(&format!("{FLUSH_JOBS_PK} UUID PRIMARY KEY")),
        "jobs primary key must be {FLUSH_JOBS_PK}"
    );
    let reason = format!(
        "reason IN ({})",
        FLUSH_JOBS_REASONS
            .iter()
            .map(|r| format!("'{r}'"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert!(jobs.contains(&reason), "jobs.reason check must be {reason}");
    assert!(
        !SCHEMA.contains("graph_jobs"),
        "graph_jobs is step 7 and must not be in the flush schema"
    );

    let recon = between(PLAN, "### 1. step-recon", "### 2. step-compose");
    assert!(
        recon.contains("`graph_jobs` is step 7"),
        "step-recon must leave graph_jobs for step 7"
    );
}

/// `primary` may appear only as part of `primary key`.
fn names_leader(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.matches("primary").count() != lower.matches("primary key").count()
        || lower.contains("promot")
}

/// Copies of a replica set are equal. Writes carry a fence and a contiguous lsn.
/// Catch-up reads another copy's `_repl_log`. Search acks on one copy, the graph on two.
#[test]
fn copies() {
    let steps = between(PLAN, "### 4. step-repl-core", "## After this board");
    let cluster = between(README, "## Cluster", "## Memory");
    for (name, text) in [
        ("scale.md", SCALE),
        ("README cluster", cluster),
        ("steps 4–13", steps),
    ] {
        assert!(
            !names_leader(text),
            "{name} must not name a leader copy or a promotion"
        );
        assert!(
            !text.contains("`role`"),
            "{name} must not keep a role column"
        );
    }

    for needle in [
        "rendezvous",
        "`fence`",
        "`lsn`",
        "`applied_lsn`",
        "`_repl_log`",
        "hkey % shard_count",
    ] {
        assert!(SCALE.contains(needle), "scale.md must define {needle}");
    }
    assert!(
        SCALE.contains("A search write needs **one** copy"),
        "scale.md must ack a search write on one copy"
    );
    assert!(
        SCALE.contains("| `ack` | **2** (majority) | **1** |"),
        "scale.md must ack a graph write on a majority"
    );
    assert!(
        SCALE.contains("never from a sibling shard"),
        "scale.md must refill a search shard from the graph, not a sibling"
    );
    assert!(
        cluster.contains("Rendezvous") && cluster.contains("applied_lsn"),
        "README cluster must name rendezvous placement and applied_lsn catch-up"
    );

    let core = between(PLAN, "### 4. step-repl-core", "### 5. step-layout");
    for scenario in ["| Placement", "| Fenced", "| Gap", "| One copy down"] {
        assert!(core.contains(scenario), "step 4 must test {scenario}");
    }
    let third = between(PLAN, "### 11. step-third-node", "### 12. step-graph-copies");
    assert!(
        third.contains("rendezvous") && third.contains("REPL_REALLOCATE_DELAY_MS"),
        "step 11 joins by rendezvous and waits the reallocate delay"
    );
    let graph = between(PLAN, "### 12. step-graph-copies", "### 13. step-backup");
    for scenario in ["| Majority ack", "| No lost ack", "| Divergent"] {
        assert!(graph.contains(scenario), "step 11 must test {scenario}");
    }
}

/// Every default scale.md names has a step. No non-goal defers a designed part.
#[test]
fn coverage() {
    let defaults = between(SCALE, "## Defaults", "## Why this shape");
    let steps = between(PLAN, "### 4. step-repl-core", "## After this board");
    let names: Vec<&str> = defaults
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
        .collect();
    assert!(
        names.len() >= 12,
        "scale.md defaults must name its settings"
    );
    for name in names {
        assert!(steps.contains(name), "steps 4–13 must use {name}");
    }

    let non_goals = between(PLAN, "## Non-goals", "## Constraints");
    for designed in ["Hedged", "`min_lsn`", "shard_count", "backups", "archive"] {
        assert!(
            !non_goals.contains(designed),
            "non-goals must not defer {designed}; scale.md designs it"
        );
    }
    for (step, scenario) in [
        ("### 8. step-query", "| Hedge"),
        ("### 9. step-recover", "| Divergent"),
        ("### 10. step-monitor", "| Monitor"),
        ("### 13. step-backup", "| Point in time"),
        ("### 14. step-split", "| No early flip"),
    ] {
        let rest = between(PLAN, step, "## After this board");
        let body = rest.split("\n### ").next().unwrap_or(rest);
        assert!(body.contains(scenario), "{step} must test {scenario}");
    }
}
