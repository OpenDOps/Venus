# Search cluster


|               |                                                                                     |
| ------------- | ----------------------------------------------------------------------------------- |
| **planId**    | `ss-cluster`                                                                        |
| **Milestone** | [M5](../../M5/README.md). Board 1 of the [semantic graph](../README.md#boards). Search design: [README](./README.md). |
| **Duration**  | About 6 weeks. The replication layer and the layout (steps 4, 5, 9, and 10) are about half of it. |
| **Board**     | [SS.state.yaml](./SS.state.yaml)                                                    |


Graph role, replication, and write order: [scale.md](../scale.md). Graph schema: [store.md](../store.md). Flush queue this must not join: [LiveSnapshot HA](../../LiveSnapshot/high-availability.md).

This board builds everything [scale.md](../scale.md) designs: **surrealastic** (`crates/surrealastic`, replication and layout), the search cluster on it, three graph copies, recovery, failure detection, backups, and shard splits. Each search node is a full SurrealDB server with the **search schema only**. Every search shard and every wiki graph is a replica set of equal copies, written with a fence, a contiguous `lsn`, and a per-copy `_repl_log`. HA default is three search nodes and `replica_count = 1` ([README — high availability](./README.md#high-availability)).

Steps 4–10 run on two search nodes and one graph node, so a search copy can be lost while writes and reads continue. Step 11 adds the third search node, so a replacement copy can land while the dead node is still gone. Step 12 gives the graph three copies acked on two. Step 13 adds backups and the log archive. Step 14 splits shards online.

## Where the board is

Steps 1–7 are done (step 7 on 2026-10-08). Steps 8–14 are not started. Next is [step 8](#8-step-query). The [M4](../../M4/README.md) person pass does not block the board. Detail per step is the [summary](#steps-summary) below.

**Shipped.** Compose profile `graph` runs SurrealDB `v2.7.0`: one graph node (`:28730`, namespace `graph`, no `SEARCH` index) and two search nodes (`:28731`, `:28732`, namespace `search` only), one volume and a 1 GB cap each. Both search nodes have the BM25 indexes. Postgres holds the fixture wiki in `layout_db` (`shard_count` 2, `replica_count` 1); `search_cluster`, `search_node`, and `search_allocation` are gone. `crates/surrealastic` replicates any SurrealDB database and lays one out: the `repl_*` map, `layout_db`, a fenced writer lease, a guarded `_repl_log` in the same transaction as the data, fan-out with ack, rendezvous placement, and TLS. A layout write commits on one set, records each item in `_layout_item` and the commit’s item list in `_layout_commit`, then applies by `key % shard_count`, including a cursor entry on a shard with no items. A shard that cannot ack stays queued and is rebuilt from `_layout_commit` after its cursor. Refill reads the item table, not a sibling shard. The crate has no page, doc, or field hash. `venus-graph` projects one job with one `layout.write`: `hkey` is the item key, `doc_id` is the item, and `serve` holds `writer:{ws}`. After `last_flushed` commits, the sidecar upserts `graph_jobs`. `serve` claims that row with `SKIP LOCKED` and, with no fixture, records the sha.

**Layers.** The same six as [scale.md](../scale.md#layers). Replication and the layout are shipped.

| Layer | Shipped | Still to build |
|---|---|---|
| **Venus** (`venus-graph`) | [6](#6-step-project) one `layout.write` per job, `hkey` as the item key, migrate into `layout_db`, `serve` holds the writer lease. [7](#7-step-enqueue) `graph_jobs` after flush; `serve` claims with `SKIP LOCKED` while it holds the writer lease. | [8](#8-step-query) the router. |
| **Layout** (surrealastic) | [5](#5-step-layout) commit set, `_layout_item`, `_layout_commit`, shard cursor, queued apply, refill, `schema` hook. | [9](#9-step-recover) refill after a trimmed log, queue rebuild on takeover, an ahead shard copy refilled from the commit set. [14](#14-step-split) split and rebuild. |
| **Replication** (surrealastic) | [4](#4-step-repl-core) log, fence, fan-out, placement. | [9](#9-step-recover) catch-up, snapshot, takeover fence, occupied lsn, hole fill, divergence, retention. [12](#12-step-graph-copies) graph ack 2 of 3. |
| **Graph commit set** | One node, the graph schema, and one commit per job (`page.hkey`, headings, mentions, edges, `_layout_item`). | [12](#12-step-graph-copies) three copies. [13](#13-step-backup) archive and restore. |
| **Search shards** | Two nodes. One database per shard copy. Items land by `hkey % shard_count`, each row carrying `indexed_sha`. | [11](#11-step-third-node) the third node, replacement, `removed`. |
| **Postgres** | `repl_lease`, `repl_node`, `repl_set`, `repl_copy`, `layout_db`. Step 3’s `search_cluster` has been carried into `layout_db`. | [10](#10-step-monitor) node state. [11](#11-step-third-node) `repl_node_seq`. |

**Further, by step.**

| Step | Adds |
|---|---|
| [7](#7-step-enqueue) | `graph_jobs` after `last_flushed`. A down cluster does not fail a flush. |
| [8](#8-step-query) | Word search across shards: two choices, one hedge, failover, `min_commit`, hydrate from the graph. |
| [9](#9-step-recover) | A copy that missed entries catches up; a wiped copy returns by snapshot; a new writer keeps every acked entry and rebuilds the shard queue. |
| [10](#10-step-monitor) | Nodes go `suspect`, `down`, and `up`. Each set is green, yellow, or red. |
| [11](#11-step-third-node) | A third search node. A node down past the delay gets a replacement. `replica_count` stays 1. A removed node keeps its id. |
| [12](#12-step-graph-copies) | Three graph copies, acked on two. Losing one does not stop graph writes. |
| [13](#13-step-backup) | The graph log is archived before trim. A graph restores to any archived `lsn`. |
| [14](#14-step-split) | The fixture wiki goes from 2 shards to 4 while writes and reads continue. A shrink is a new `epoch`. |

## Shape

End state of this board:

```text
surreal-graph    :28730  node 0  pool graph    zone 0   a copy of each wiki graph
surreal-search-0 :28731  node 1  pool search   zone 1   copies by rendezvous
surreal-search-1 :28732  node 2  pool search   zone 2   copies by rendezvous
surreal-search-2 :28733  node 3  pool search   zone 3   step 11
surreal-graph-1  :28734  node 4  pool graph    zone 4   step 12
surreal-graph-2  :28735  node 5  pool graph    zone 5   step 12
venus-graph serve        writer, router, monitor; crates/surrealastic linked in
Postgres                 repl_lease, repl_node, repl_set, repl_copy, layout_db
REPL_ARCHIVE_URL         file:// in dev and tests, s3:// in production   step 13
```

In dev, `node_id` is the port minus 28730, and the zone is the `node_id`. `node_id`s are never reused. Tests read homes from `repl_copy`. They do not hardcode which nodes a set lands on.

Venus passes `key = hkey`, `hkey` = first 8 bytes of `sha256(doc_id)`, big-endian, shifted right by 1. The layout applies `shard = key % shard_count` ([scale.md](../scale.md#shards)). The board’s fixture wiki uses `shard_count = 2`; a new wiki gets 1. With two copies per shard on two search nodes, both shards are on both nodes. Step 14 doubles the fixture wiki to 4 shards.

## Gate


| Steps    | When                                                                                          |
| -------- | --------------------------------------------------------------------------------------------- |
| **1–14** | When `dependsOn` is done. Step 7 hooks after `last_flushed`. The M4 person pass does not block it. |




## Exit

1. Profile `graph` starts three graph copies (`127.0.0.1:28730`, `:28734`, `:28735`) and three search nodes (`:28731`, `:28732`, `:28733`). One volume each. Search nodes have no graph namespace.
2. `crates/surrealastic` has no Venus types. Every replicated write is a guarded log entry with `lsn`, `prev`, `prev_fence`, and `fence`, in one transaction with its data. Map in the `repl_*` tables and `layout_db`, no role column. Leases carry a `fence`.
3. Fixture docs: one `layout.write`. The commit set acks with the items in `_layout_item`, then the layout applies each item by `key` and moves that shard’s `_layout:cursor`. A stale fence is refused. A copy that missed an entry refuses the next one with `gap`. Entries stay within 256 items and 4 MiB. A shard with no acking copy stays queued and is applied when a copy returns, without a second write; a new writer rebuilds that queue from the cursors. Every new copy gets its schema from `ensure`.
4. `@@` merges every shard and never runs on a graph node. Two random choices, one hedge after p95, failover in the same request, `min_lsn`, `partial` after 2 s, hydrate within 100 ms, `unverified` when the graph does not answer. Reads continue while Postgres is down.
5. A lagging copy catches up from another copy’s `_repl_log` when that log is the history just chosen. A wiped or too-far-behind copy returns by snapshot plus catch-up. A writer takeover stores the new fence before it chooses a reference or appends. On a graph set it propagates the reference tip before any new lsn, and it keeps every acked entry. An entry the majority can ack without is left divergent. A write that committed on fewer copies than `ack` keeps that lsn until it is acked or restored away. An lsn nobody committed, sitting under a newer lsn, is an empty log row. A divergent commit-set copy hands its tags to `lost`. A divergent shard copy is refilled from the commit set, not from its sibling. The log is trimmed. A search shard with no copy, or with a cursor behind the trimmed commit log, is refilled from `_layout_item` on the commit set, never from a sibling shard.
6. Flush upserts `graph_jobs` after `last_flushed` and still commits if every SurrealDB process is down.
7. A third search node takes a new copy after one node is stopped past the delay, `replica_count` still 1, `shard_count` still 2.
8. Each wiki graph has three copies acked on two. Losing one graph copy loses no acked write and does not stop writes. Losing two stops graph writes; reads continue.
9. The monitor marks nodes `suspect`, `down`, and `up`. Health is green, yellow, or red per set.
10. Graph sets archive their log before trimming it and have a nightly export. A graph restores to any `lsn` in the archive. Losing every graph copy restores the graph from backup.
11. A split takes the fixture wiki from 2 shards to 4 while writes and reads continue. A shrink is a rebuild under a new `epoch`.
12. No LLM, no tree-sitter, no glossary scan, no graph UI. M1–M4 tests stay green.




## Non-goals


| Later                                               | Why not here                                                                  |
| --------------------------------------------------- | ----------------------------------------------------------------------------- |
| `replica_count = 2`                                 | Not the HA default. Placement tests still cover three copies.                 |
| Headings from sidecar, glossary, tree-sitter, regex | [Extraction](../extraction/plan.md). Fixtures only on this board.             |
| Re-extract from git in the graph `rebuild` hook     | Needs the next board’s extractor. Here the hook restores from backup, then re-runs fixture jobs. |
| Gist re-projection after the semantic pass          | Needs the semantic model.                                                     |
| Semantic model, NER, HNSW                           | NER is extraction, off by default. HNSW is [vectors](../vectors/plan.md). The model is [semantic](../semantic/plan.md). |
| Surrealastic as its own proxy binary                | [scale.md](../scale.md#why-a-layer-not-a-surrealdb-patch) allows it without a design change. A library is enough now. |
| SurrealDB Enterprise, Raft between SurrealDB nodes  | Postgres chooses the writer. The layer replicates the log.                    |
| Tantivy instead of SurrealDB `SEARCH`               | [Tantivy](../tantivy/plan.md), after this board’s read path. This board keeps SurrealDB `SEARCH`. |




## Constraints

1. Search nodes run `surrealdb/surrealdb:v2.7.0`. Schema is namespace `search` only.
2. Order: one `layout.write`. The commit set acks per its policy. The layout then applies items to every `in_sync` shard copy in parallel (acked on one). A failed copy is `lagging` and the cluster is yellow. The caller does not see the shard.
3. One fenced writer per wiki, through `crates/surrealastic`. Copies of one set are in distinct zones. No leader among copies. Copies never connect to each other.
4. Catch up a graph copy from another copy of **that** set’s `_repl_log`, or by snapshot, when that copy is the reference tip and the fill happens before any new lsn. An entry the majority can ack without stays divergent. Refill a search shard from `_layout_item` and `_layout_commit` on the commit set when it has no copy, its cursor is behind the retained commit log, or it is ahead of the sibling. Not from a sibling shard, not from markdown while a graph copy or backup exists.
5. Postgres is not on the per-write or per-read path. It is written when a lease is claimed or renewed, or a node or copy changes state.
6. SurrealDB WebSocket RPC for writes, catch-up, and reads. HTTP `/export`, `/import`, `/health`. No gRPC, no compression, no second port on a SurrealDB node.
7. Every default in [scale.md — defaults](../scale.md#defaults) is an environment variable of that name. Tests set the timers low; they do not change the rules.
8. Block cache is set from each container’s cap (1 GB cap, 64 MB cache).
9. Hub and browser do not open these ports.
10. `graph_jobs` is not flush `jobs`.



## Design coverage

Every section of [scale.md](../scale.md) and the step that builds it.

| scale.md                                        | Step |
| ----------------------------------------------- | ---- |
| The replication layer, replica sets and policy  | 4 (generic), 6 (Venus sets), 12 (graph policy) |
| The layout, commit set, key, queued apply, refill | 5  |
| Item table `_layout_item`, shard cursor, `read` with `min_commit`, `lag` | 5; Venus items in 6; fresh reads in 8; refill after trim and queue rebuild in 9 |
| Schema: `schema(set)`, `ensure`, `define` entries | 5 (generic), 6 (Venus statements) |
| The log, guarded write, body rule               | 4    |
| Write path: one write, graph body, items        | 6    |
| Write path: fan-out, one in flight, ack, `REPL_LAG_MAX` | 4 |
| Write path: occupied lsn, empty hole fill, takeover fence | 9; graph short-of-ack in 12 |
| Protocol, TLS                                   | 4 (WebSocket, TLS), 9 (export and import), 10 (probe) |
| Catch-up, snapshot, writer takeover, divergence | 9; graph quorum in 12 |
| Log retention                                   | 9; archive before trim in 13 |
| Backups, point-in-time restore                  | 13   |
| Read path                                       | 8    |
| Placement (rendezvous, zones, weight)           | 4    |
| Replacement and join, draining                  | 11   |
| Shards: `hkey` as the item key, new wiki gets 1 | 6    |
| Shards: split, epoch rebuild, trigger, cap      | 14   |
| Failure detection, health                       | 10; health levels from 4 |
| Node ids from `repl_node_seq`, state `removed`  | 11   |
| Restore, case by case                           | 9, 10, 11, 12, 13 |
| Postgres tables, lease statement                | 4 (`repl_*`), 5 (`layout_db`) |
| Compared with Elasticsearch; Tantivy upgrade    | [Tantivy](../tantivy/plan.md), optional board 4. This board keeps SurrealDB `SEARCH`. |



## Steps summary

What each step **adds** to the product (not how to test it — that is under each step). Mark **status** on the step when the [board](./SS.state.yaml) moves.


| #                         | id                                          | Proves                                                                                       |
| ------------------------- | ------------------------------------------- | -------------------------------------------------------------------------------------------- |
| [1](#1-step-recon)        | [`step-recon`](#1-step-recon)               | ✅ **done.** Full binary, search schema only. P=2, R=1. HA target is 3 nodes, R=1.           |
| [2](#2-step-compose)      | [`step-compose`](#2-step-compose)           | ✅ **done.** Graph + two search nodes, three volumes.                                         |
| [3](#3-step-schema)       | [`step-schema`](#3-step-schema)             | ✅ **done.** Search indexes on :28731 and :28732. Allocation rows.                              |
| [4](#4-step-repl-core)    | [`step-repl-core`](#4-step-repl-core)       | ✅ **done.** `surrealastic` replication: map, leases, guarded log, fan-out with ack, placement, TLS. |
| [5](#5-step-layout)       | [`step-layout`](#5-step-layout)             | ✅ **done.** `surrealastic` layout: commit set, `_layout_item`, `key % shard_count`, shard cursor, queued apply, refill, `schema` hook. |
| [6](#6-step-project)      | [`step-project`](#6-step-project)           | ✅ **done.** Venus bodies on the layout: `hkey` as `key`, `doc_id` as item, `search_cluster` migrated into `layout_db`, one write per job. |
| [7](#7-step-enqueue)      | [`step-enqueue`](#7-step-enqueue)           | ✅ **done.** `graph_jobs` after flush. A down cluster does not fail a flush. One writer claims the row. |
| [8](#8-step-query)        | [`step-query`](#8-step-query)               | **pending.** Router: map cache, two choices, hedge, failover, `min_lsn`, merge, hydrate.      |
| [9](#9-step-recover)      | [`step-recover`](#9-step-recover)           | **pending.** Catch-up, snapshot, takeover with the fence stored first, divergence, occupied lsn, hole fill, retention. Shard refill stays in the layout. |
| [10](#10-step-monitor)      | [`step-monitor`](#10-step-monitor)           | **pending.** Monitor lease, probes, node states, position sweep, writer reactions.           |
| [11](#11-step-third-node) | [`step-third-node`](#11-step-third-node)    | **pending.** Third node joins by rendezvous, replacement after the delay, draining, `removed` state, ids never reused. R stays 1. |
| [12](#12-step-graph-copies) | [`step-graph-copies`](#12-step-graph-copies) | **pending.** Three graph copies, ack 2 of 3, quorum takeover, graph `lost`.               |
| [13](#13-step-backup)     | [`step-backup`](#13-step-backup)            | **pending.** Log archive, nightly export, point-in-time restore, graph `rebuild`.            |
| [14](#14-step-split)      | [`step-split`](#14-step-split)              | **pending.** Split by doubling, epoch rebuild, trigger, cap.                                 |




## Steps

[Steps summary](#steps-summary). Do them in order. A step is not started until its `dependsOn` steps are done. The [M4](../../M4/README.md) person pass does not block step 7. Test scenarios under each step are the accept rules. Encode them as tests where the How line names a command; do not invent extra scenarios.

Fault injection in live tests: `docker stop`, `docker start`, `docker pause` (a copy that does not answer), volume removal, and the layer’s `fault` cargo feature (drop one copy’s connection, stop the writer after the k-th copy commits, commit on one copy only). The `fault` feature is off in every non-test build.

<a id="1-step-recon"></a>

### 1. step-recon

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                                  |
| ------------- | ---------------------------------------------------------------- |
| **n**         | 1                                                                |
| **id**        | `step-recon`                                                     |
| **title**     | Pin the image, search schema only, HA default                    |
| **dependsOn** | (none)                                                           |
| **kind**      | implement                                                        |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`)          |


#### Work

- Pin image `surrealdb/surrealdb:v2.7.0` (channel `v2`). Rust client `surrealdb` `2.7.0` in `crates/venus-graph`, `default-features = false`, features `protocol-ws` and `rustls` only. Do not enable `kv-rocksdb`.
- Record that a search node is that full server and only the [search schema](./README.md#schema). Search nodes do not create namespace `graph`. No graph namespace on its volume.
- Record dev shape P=2, R=1, two nodes, and the HA default: three nodes, `replica_count` stays 1 ([README](./README.md#high-availability)).
- Record flush `jobs` (`workspace_id` primary key, `reason` `idle` | `flush` | `lease`). `graph_jobs` is step 7.



#### Do not

- Add a full-text index to the graph schema.
- Set `replica_count = 2` as the default.
- Change Flush.



#### Test scenarios


| Name       | Pass                                                                                          |
| ---------- | --------------------------------------------------------------------------------------------- |
| Binary     | This plan says search nodes use `surrealdb/surrealdb:v2` and do not create namespace `graph`. |
| HA default | This plan’s exit 7 is three nodes and `replica_count = 1`, not `replica_count = 2`.           |
| Queue split | `jobs.workspace_id` is the primary key. `reason` is `idle`, `flush`, or `lease`. No `graph_jobs` table. |
| Copies     | [scale.md](../scale.md), the README cluster, and steps 4–14 describe equal copies in replica sets: rendezvous placement, `fence`, `lsn`, `applied_lsn`, `_repl_log` catch-up from another copy of the same set. Search acks on one copy, the graph on two. None of them names a leader copy or a promotion. |
| Coverage   | Every name in [scale.md — defaults](../scale.md#defaults) appears in steps 4–14. No non-goal defers hedging, `min_lsn`, splits, or backups. |


- **How:** `cargo test -p venus-graph --test recon`. Fail if the client enables `kv-rocksdb`, if exit 7 sets `replica_count = 2`, if flush `jobs` is not keyed by `workspace_id`, if the design or steps 4–14 bring back a leader copy, or if a designed default has no step.




<a id="2-step-compose"></a>

### 2. step-compose

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · **2** · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 2                                                       |
| **id**        | `step-compose`                                          |
| **title**     | Compose graph and two search nodes                      |
| **dependsOn** | `step-recon`                                            |
| **kind**      | implement                                               |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Profile `graph`.
- `surreal-graph`: `127.0.0.1:28730`, volume `surreal-graph-data`, `rocksdb:/data/graph.db`, `mem_limit` 1g, `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` 64 MB.
- `surreal-search-0`: `127.0.0.1:28731`, volume `surreal-search-0-data`, `rocksdb:/data/search.db`, same cap and cache.
- `surreal-search-1`: `127.0.0.1:28732`, volume `surreal-search-1-data`, same command and cap.
- User and pass from Compose env. Healthcheck per port. `pids_limit` and dropped capabilities, same posture as `hub`.



#### Do not

- Start these on the default Compose path.
- Publish `0.0.0.0`.
- Share one volume between search nodes.
- Size the cache from the host’s RAM.



#### Test scenarios


| Name             | Pass                                                                    |
| ---------------- | ----------------------------------------------------------------------- |
| Profile off      | `docker compose up` without `--profile graph` starts none of the three. |
| Three processes  | With the profile, :28730, :28731, and :28732 are healthy, 1g each.         |
| Independent disk | A row on :28731 survives a recreate of :28732.                            |


- **How:** `cargo test -p venus-graph --test compose`. Fail if default `docker compose config --services` lists a surreal service, if a host port is `0.0.0.0`, or if :28731 and :28732 share a volume.




<a id="3-step-schema"></a>

### 3. step-schema

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · **3** · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 3                                                       |
| **id**        | `step-schema`                                           |
| **title**     | Graph schema, search schema, allocation rows            |
| **dependsOn** | `step-compose`                                          |
| **kind**      | implement                                               |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Crate `crates/venus-graph`. Apply [store.md](../store.md#schema) to :28730 only.
- Apply the [search schema](./README.md#schema) to :28731 and :28732. Idempotent.
- Seed `search_cluster` (`shard_count = 2`, `replica_count = 1`) and four `search_allocation` rows: shard 0 primary on node 0, replica on node 1; shard 1 primary on node 1, replica on node 0.
- `page.search_sha` and `page.search_error` live on the graph `page` table.
- The graph migrator refuses a `SEARCH` index. Search migrator does not create `links_to`.



#### Do not

- Create namespace `graph` on a search node.
- Open these sockets from `venus-hub` or `apps/web`.



#### Test scenarios


| Name                | Pass                                                                      |
| ------------------- | ------------------------------------------------------------------------- |
| Graph has no index  | :28730 has `links_to` and no full-text index.                              |
| Search has no graph | :28731 and :28732 have `heading_text` and `mention_text` and no `links_to`. |
| Allocation          | The four rows above. A second migrate does not add a second primary.      |


- **How:** `cargo test -p venus-graph --test schema`. Fail if :28730 has a `SEARCH` index, if a search node has `links_to` or namespace `graph`, or if a second migrate adds a primary.




<a id="4-step-repl-core"></a>

### 4. step-repl-core

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · **4** · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 4                                                       |
| **id**        | `step-repl-core`                                        |
| **title**     | Replication layer core                                  |
| **dependsOn** | `step-schema`                                           |
| **kind**      | implement                                               |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- New crate `crates/surrealastic` ([scale.md — the replication layer](../scale.md#the-replication-layer)). No Venus types: no page, doc, or shard rule. Dependencies: `surrealdb` 2.7.0 (`protocol-ws`, `rustls`), `sqlx` (Postgres), `tokio`, `xxhash-rust` (xxh3). API: `claim(set) → Writer`, `writer.write(set, body, tag) → lsn`, `read(set, query, min_lsn)`, hooks `rebuild(set)` and `lost(set, tags)` as a trait the owner implements. Layout is step 5, in this same crate.
- Postgres migration owned by the layer, called from `venus-graph` migrate ([scale.md — Postgres tables](../scale.md#postgres-tables)): `repl_lease`, `repl_node`, `repl_set`, `repl_copy`. Idempotent. Every change to `repl_node`, `repl_set`, or `repl_copy` sends `NOTIFY repl_map`.
- Lease: the one-statement claim or renew from scale.md, 10 s, renewed every 3 s. No row returned means another holder. A writer that fails to renew stops sending before `lease_until`.
- `ensure(set)` on each copy: creates the database and `_repl` / `_repl_log` ([scale.md — the log](../scale.md#the-log)). `_repl_log` ids are integers.
- Guarded write ([scale.md](../scale.md#guarded-write)): one SurrealQL transaction per entry with the five checks, the body, the `_repl_log` `UPSERT`, and the `_repl:state` update. Replies `ok`, `already`, `fenced`, `gap at N`, `divergent`, each with the copy’s `applied_lsn` and `applied_fence`.
- Body rule: bodies are built from `upsert(id, content)`, `delete(id)`, `delete_range(table, from, to)`, and `insert_relation(id, in, out, content)`, with parameters bound. Raw SurrealQL is refused if it contains `rand::`, `time::now`, `CREATE` without an id, `+=`, or `-=`. `at` and any time value are parameters filled once by the writer.
- Writer: one queue per set. On claim, the head is the highest `applied_lsn` among reachable copies (full takeover in step 9, including the fence write before any new data entry). Entry `n` is `head + 1`, `prev = n − 1`, `prev_fence` = fence of the head entry.
- Fan-out ([scale.md — write path](../scale.md#write-path)): every `in_sync` copy of the set in parallel. One entry in flight per copy, in `lsn` order. The call returns when `ack` copies answered `ok` or `already`. The other copies keep their entry in flight. A copy that replies `gap`, times out, or falls `REPL_LAG_MAX` behind becomes `lagging` and leaves the live stream. `fenced` stops this writer. `repl_copy` is written only when a copy changes state. An lsn that committed on fewer copies than `ack`, and an empty row for an lsn nobody committed, are step 9.
- Connections: one WebSocket per node per process, signed in once, requests multiplexed. `wss://` URLs use `rustls` with the configured CA. No compression.
- Placement ([scale.md — placement](../scale.md#placement)): a pure function `homes(set_id, copies, members)` with xxh3 rendezvous, `weight`, the zone rule, `node_id` tie-break, `down` nodes still members, `draining` nodes not. Fewer zones than `copies` returns fewer homes.
- Health per set from `repl_copy` and `repl_node`: green, yellow, red ([scale.md — failure detection](../scale.md#failure-detection)).
- Every default from [scale.md — defaults](../scale.md#defaults) is read from the environment with that name.
- Live tests use scratch sets in namespace `search`, database `repl_t_{uuid}`, on :28731 and :28732, and drop them at the end.



#### Do not

- Import a type from `venus-graph`.
- Send a write without `lsn`, `prev`, and `fence`.
- Send more than one entry in flight to a copy, or a live entry to a `lagging` copy.
- Use a string id or a field scan for `_repl_log`.
- Read or write Postgres per entry.
- Use `hash % N` for copies.



#### Test scenarios


| Name          | Pass                                                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Placement     | For N = 1…10 and copies = 1…3: homes are distinct zones, stable across runs, and the same for any order of `members`. Adding a node moves only the sets whose homes now include it, about `copies / N` of them. Removing one moves only its own copies. |
| Weight        | Over 10 000 sets, a node with weight 2 holds 1.8–2.2× the copies of a weight-1 node.                                                  |
| Zones         | Two nodes in one zone and copies = 2: one home is in that zone, never two. One zone in the pool: one home, set yellow.                |
| Draining      | A `draining` node is never a home. A `down` node still is.                                                                            |
| Body rule     | `rand::uuid()`, `time::now()`, `CREATE t`, `x += 1`, and `x -= 1` are refused before anything is sent. Builder bodies pass.           |
| Lease         | Two claimants: one gets a row, the other gets none. After `lease_until`, the second claims with `fence + 1`. Renewing keeps the fence.|
| Renew lost    | The writer’s Postgres connection is cut. No entry is sent after its `lease_until`.                                                    |
| Fenced        | A second claimant takes the lease. An entry with the old fence is refused `fenced` by every copy, and the old writer stops.          |
| Gap           | An entry whose `prev` is past a copy’s `applied_lsn` is refused with `gap at N`, and nothing is written.                             |
| Already       | Sending the last entry again returns `already` and leaves one log row.                                                                |
| Divergent     | A copy whose `applied_lsn > prev`, or whose `applied_fence ≠ prev_fence`, refuses with `divergent`. Nothing is written.              |
| Atomic        | A body that fails mid-way leaves no data, no log row, and `_repl:state` unchanged.                                                    |
| Range read    | `_repl_log:98..=101` returns 98, 99, 100, 101 in that order. `EXPLAIN` shows a range read, not a table scan.                          |
| Ack policy    | Two copies, `ack` 1: with :28732 paused, a write returns after :28731 commits. `ack` 2: the same write is not acked until :28732 resumes.|
| One copy down | Stop :28732, then write with `ack` 1. :28731 has the entry. The copy on :28732 is `lagging`. Yellow.                                     |
| Late timeout  | Pause :28732, then write with `ack` 1. The write returns with that copy `in_sync`. When its send times out, it is `lagging`. Yellow. |
| One in flight | 200 writes in a burst: never two entries in flight to one copy. Every copy applies them in `lsn` order.                              |
| Lag max       | Pause :28732 while writing 100 entries. It becomes `lagging` at `REPL_LAG_MAX`. Writes to :28731 do not wait for it.                    |
| Quiet map     | 100 writes with every copy in sync change no `repl_copy` row.                                                                         |
| TLS           | A SurrealDB container started with `--web-crt` / `--web-key` from a test CA: `wss://` with that CA connects; without it, it is refused. |


- **How:** `cargo test -p surrealastic` (pure placement, body rule) and `cargo test -p surrealastic --test live --features fault` with profile `graph` up. Fail if a scenario writes to a copy outside a guarded transaction.




<a id="5-step-layout"></a>

### 5. step-layout

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · **5** · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 5                                                       |
| **id**        | `step-layout`                                           |
| **title**     | Layout: commit set, shard key, apply, refill           |
| **dependsOn** | `step-repl-core`                                        |
| **kind**      | implement                                               |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Layout in `crates/surrealastic` ([scale.md — the layout](../scale.md#the-layout)). No Venus types: no page, doc, or field hash. Same crate as replication.
- A database is one commit set plus its shard sets. Postgres table `layout_db`: `db_id`, `commit_set`, `epoch`, `shard_count`, `shard_count_next`, `replica_count`, `shard_ack`. Shard set id `{db_id}:{epoch}:{shard}`. Idempotent. A change sends `NOTIFY repl_map`.
- API ([scale.md — the layout](../scale.md#the-layout)): `write(db, commit_body, items) → commit`, `read(db, query, min_commit?) → rows, partial`, `lag(db)`. An item is `key` (i64), `item` (the owner’s stable id), `tag`, `rids` (record ids it writes on a shard), `body`, and `delete`. `commit` is the commit set’s `lsn`. `tag` and `item` are stored and not interpreted.
- Item table ([scale.md — item table](../scale.md#item-table)): the commit entry carries the `_layout_item` upsert (or delete) of each item, id = `key`, in the same transaction as `commit_body`. A row whose `item` differs throws `key collision` and the whole entry rolls back.
- Shard cursor ([scale.md — shard cursor](../scale.md#shard-cursor)): every shard entry sets `_layout:cursor.commit` in its own transaction. `lag(db)` is commit head minus cursor per shard. `read` with `min_commit` skips a copy whose cursor is lower and lists the shard in `partial` with reason `behind` when no copy qualifies.
- Schema ([scale.md — schema](../scale.md#schema)): the hook `schema(set) → statements`. `ensure` applies it to every new copy before the first entry or import. A schema change is one guarded entry of `define` ops; the `define` body op accepts only `DEFINE` and `REMOVE` statements. This extends step 4’s `ensure` and body ops; step 4’s tests keep passing unchanged.
- `write` returns when the commit set has acked. It then applies each item to shard `key % shard_count`. While `shard_count_next` is set, it also applies the item to `key % shard_count_next`. Every shard gets an entry for that commit, including one with no items, and that entry sets `_layout:cursor`. Packs stay within `REPL_ENTRY_DOCS` and `REPL_ENTRY_BYTES`. A delete item removes its `rids` on the shard and its `_layout_item` row. The commit entry also writes `_layout_commit:{lsn}` so the delete remains after the item row is gone.
- A shard copy that gaps, times out, or falls behind is `lagging` and is caught up by surrealastic replication. The write result does not name a shard.
- A shard with no acking copy keeps that commit queued, behind earlier queued commits, in commit order. When a copy is up, the layout applies them. The commit set is not written again. On start, the layout rebuilds the queue from `_layout_commit` after each shard’s cursor. A copy is `in_sync` only once replay has reached the head. Refill refuses a database whose `applied_lsn` is already past 0.
- A shard with no copy left is refilled ([scale.md — refill](../scale.md#refill)): note commit head `L0`, create empty copies with the schema, range-read `_layout_item` in `REPL_CATCHUP_BATCH` pages, write the items that land on the shard within the entry limits (last entry sets the cursor to `L0`), then apply commits after `L0`. A sibling shard is not a source. The owner hook is not called.
- The caller is told the write failed only when the commit set cannot ack.
- Live tests use a scratch commit set on :28730 (ack 1) and shard copies on :28731 and :28732 (ack 1), `shard_count = 2`, and drop the databases at the end.


#### Do not

- Import a type from `venus-graph`.
- Hash a record field. The key arrives on the item.
- Return a per-shard error to the caller.
- Refill a shard from a sibling shard, by replaying the commit log from 1, or into a database that still has rows.
- Call `rebuild` or `lost` for a shard set.
- Write DDL outside `ensure` or a guarded `define` entry.
- Keep the shard cursor in memory or Postgres only.


#### Test scenarios


| Name           | Pass                                                                                                                                  |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Modulo         | Keys 0..9 with `shard_count` 2 land on `key % 2`. Item order does not change the shard.                                              |
| Commit returns | Pause both copies of shard 1. A write that touches shard 0 and shard 1 returns after the commit set acks. Shard 0 has its items. Shard 1 is queued. |
| Queued apply   | Start shard 1’s copies. The queued commit appears there without a second write. The commit set has one entry.                        |
| Queue order    | Pause shard 1, make three writes that touch it, start it. The three commits apply in commit order; the cursor ends at the third.   |
| One copy down  | Pause :28732, then write. The copy on :28731 has the item. The copy on :28732 is `lagging`. The result is one commit.                   |
| Item table     | After a write, `_layout_item:{key}` on the commit set holds `item`, `tag`, `rids`, `body`, and `commit`. Same transaction: a failed commit entry leaves no item row. |
| Key collision  | A second item with the same `key` and a different `item` fails the write with `key collision`. Nothing is applied; the commit set has no new entry. |
| Delete item    | A delete item removes its `rids` on the shard and its `_layout_item` row.                                                           |
| Cursor         | Each shard’s `_layout:cursor.commit` equals the last commit applied there, written in the same transaction as the entry. `lag(db)` is 0 per shard after apply, above 0 while queued. |
| Fresh read     | With shard 1 paused, `read(min_commit = last)` lists shard 1 in `partial` with reason `behind`; after start, the same read is complete. |
| Schema         | A new copy has the hook’s schema before its first entry. A schema change is one `define` entry and reaches every copy, also one that was down. A `define` with a non-DDL statement is refused. |
| Empty shard    | Wipe both copies of shard 0. The layout refills it from `_layout_item` (keys with `key % 2 = 0`), then applies commits after `L0`. Shard 1 is not read. The owner hook is not called. |
| Dual           | With `shard_count_next = 4`, an item is applied to `key % 2` and to `key % 4`.                                                       |
| Entry limit    | 300 items in one shard become two entries of at most 256, each under 4 MiB.                                                          |
| No field hash  | The crate source has no `doc_id` and no `sha256`.                                                                                    |


- **How:** `cargo test -p surrealastic --test layout` and `cargo test -p surrealastic --test layout_live` with profile `graph` up. Fail if a write result names a shard, if a shard refill reads a sibling shard, or if a refill reads `_repl_log` below `L0`.



<a id="6-step-project"></a>

### 6. step-project

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · **6** · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 6                                                       |
| **id**        | `step-project`                                          |
| **title**     | Venus bodies on the layout; one write per job           |
| **dependsOn** | `step-layout`                                           |
| **kind**      | implement                                               |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- `venus-graph` migrate: run the layout migration; migrate step 3’s `search_cluster` rows into `layout_db` (`shard_count`, `replica_count` carried over), then drop `search_node`, `search_allocation`, and `search_cluster`. A second migrate is a no-op. A new wiki gets `shard_count = 1`. Seed `layout_db` for the fixture wiki: `shard_count = 2`, `replica_count = 1`; commit set `graph:{ws}` with copies 1, `ack` 1; shard copies 2, `shard_ack` 1; nodes 0 (graph), 1 and 2 (search); homes from placement. Step 3’s allocation test moves to this shape.
- Graph schema ([store.md](../store.md#schema)) gains `page.hkey`. Still no `SEARCH` index. The commit set gets `_repl`, `_layout_item` tables through `ensure`.
- Search schema ([README](./README.md#schema)) moves to one database per shard copy, `⟨{workspace_id}_{epoch}_{shard}⟩`. The layout creates each database on its homes. Venus returns the graph and search statements from the `schema(set)` hook; `ensure` applies them. Venus does not run DDL on a copy itself.
- `hkey(doc_id)` exactly as [scale.md — shards](../scale.md#shards). The item `key` is that `hkey`. Venus does not compute the shard.
- One job calls `layout.write` once ([scale.md — write path](../scale.md#write-path)):
  1. `commit_body`: that job’s pages (with `hkey`), headings, mentions, and outbound edges, replacing that doc’s rows.
  2. One item per doc: `key = hkey`, `item = doc_id`, `tag` the job id, `rids` the doc’s search record ids, `body` that doc’s search rows (delete on its id range, then upsert), each row carrying the page’s `indexed_sha`. A removed doc is a `delete` item.
  3. The call returns the graph commit. Not acked: the job stays pending, and nothing is applied to a shard. Venus does not set `search_error` and does not stamp `search_sha` from a shard ack.
- Hooks only on the commit set: `rebuild` and `lost`. Search shards are the layout’s. Step 9 and step 12 call the graph hooks.
- Two fixture docs, chosen so `hkey % 2` differs. Each: one heading with `body`, one `mention` kind `uuid`. No file read. A bulk fixture of 300 small docs for the entry limit.
- Binary `venus-graph serve`: runs migrate, then holds the writer lease and the layout for each workspace in `VENUS_GRAPH_WORKSPACES`. Steps 7, 8, and 10 add the claim loop, the router, and the monitor to it.



#### Do not

- Parse `wiki/`.
- Write a replicated database outside surrealastic.
- Group items by shard, or retry a shard from the owner.
- Roll back graph rows because a search copy failed.
- Write the graph commit again to retry a search shard.
- Delete a document’s rows with a field scan.



#### Test scenarios


| Name             | Pass                                                                                                                         |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Hkey             | Fixed `doc_id`s give fixed `hkey`s (vectors in the test), all non-negative, and `hkey % 2` puts the two fixtures on different shards. |
| New wiki         | A second workspace gets `shard_count = 1` and one search set.                                                                |
| Migrate          | Step 3’s `search_cluster` (`shard_count 2`, `replica_count 1`) becomes the fixture wiki’s `layout_db` row with the same values. The step 3 tables are gone. A second migrate changes nothing. |
| Schema hook      | A new shard copy has the search schema and the commit set has the graph schema before their first entry, with no DDL from Venus outside the hook. |
| Item             | `_layout_item:{hkey}` holds `item = doc_id` and the doc’s search `rids`. Removing a doc deletes its search rows and its item row. |
| Indexed sha      | Every search row’s `indexed_sha` equals its graph page’s `indexed_sha`, on every copy, after a second projection of doc 0 with a new sha. |
| Search lag       | With both search nodes stopped, `lag(db)` is above 0 for both shards; after start and apply it is 0. |
| Both copies      | Doc 0 and doc 1 are on both search nodes, each in its shard database. Copies of a set have the same `(applied_lsn, applied_fence)`. The shard cursor has reached the commit. Green. |
| Search node down | Stop :28732, then project. The write returns one commit. :28731 has both docs. The copies on :28732 are `lagging`. Yellow. `search_error` is unset. |
| All copies down  | Stop both search nodes. The write returns after the graph acks. Graph heading remains. The shard cursor is behind the commit. Red for search. `search_error` is unset. |
| Retry            | Start the search nodes again. The layout applies the queued commit. The graph log has one job entry, not two.                 |
| Entry limit      | 300 bulk docs in one shard become two entries of at most 256 docs, each under 4 MiB.                                         |
| Graph not acked  | Stop :28730, then project. Nothing is sent to search. The job stays pending.                                                  |
| Serve            | `venus-graph serve` holds `writer:{ws}`; a second `serve` for the same workspace holds nothing and writes nothing.          |


- **How:** `cargo test -p venus-graph --test project` with profile `graph` up.




<a id="7-step-enqueue"></a>

### 7. step-enqueue

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · **7** · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                              |
| ------------- | ------------------------------------------------------------ |
| **n**         | 7                                                            |
| **id**        | `step-enqueue`                                               |
| **title**     | Enqueue `graph_jobs` after flush                             |
| **dependsOn** | `step-project`                                           |
| **kind**      | implement                                                    |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`)   |

#### Work

- Table `graph_jobs`: `workspace_id` primary key, `wiki_sha`, dirty doc ids, claim fields, `state` `pending` | `failed`. Hub does not insert or claim it.
- Sidecar, after git and `last_flushed` have committed: upsert `graph_jobs`. Coalesce a pending row. A failed upsert does not fail the flush.
- `venus-graph serve` claims with `SKIP LOCKED` while it holds lease `writer:{workspace_id}`, and runs the write path with that `fence`. The job id goes in the entry `tag`. No markdown parse yet: a claim with no fixture records the SHA and exits.



#### Do not

- Insert into flush `jobs`.
- Hold the pin cut.
- Start tree-sitter or the glossary scan.



#### Test scenarios


| Name            | Pass                                                                               |
| --------------- | ---------------------------------------------------------------------------------- |
| Row after flush | One `graph_jobs` row carries that `wiki_sha`. `last_flushed` matches.              |
| Cluster down    | Stop graph and both search nodes. Flush still commits. `graph_jobs` still upserts. |
| One claim       | Two workers: one holds the writer lease for that `workspace_id`. The other’s writes are fenced. |


- **How:** `cargo test -p venus-graph --test enqueue` with the sidecar and profile `graph` up.




<a id="8-step-query"></a>

### 8. step-query

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · **8** · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 8                                                       |
| **id**        | `step-query`                                            |
| **title**     | Router: merged search, hedging, failover, fresh reads   |
| **dependsOn** | `step-project`                                          |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

Layer `read` ([scale.md — read path](../scale.md#read-path)):

- Map: `repl_set`, `repl_node`, `repl_copy`, and `layout_db` in memory. Reload on `NOTIFY repl_map` and every 30 s. A failed reload keeps the old map.
- Pick: candidates are `in_sync` copies on `up` nodes, else `suspect`. Two at random; the one with fewer requests in flight; a tie goes to the lower recent latency.
- Hedge: per-copy p95 latency, floor `REPL_HEDGE_FLOOR_MS`. After it, send the same read to another candidate. First answer wins. One hedge per set per request.
- Fail over: a connection error or timeout tries the next candidate in the same request and ejects that node locally for 5 s. No Postgres write.
- Fresh read: with `min_lsn`, the read starts with the `_repl:state` check and throws `behind`; a copy that is behind fails over. On a shard set, layout `read` with `min_commit` checks `_layout:cursor.commit` the same way; no copy at `min_commit` lists the shard in `partial` with reason `behind`.

Venus search on it:

- `@@` to every search set of the wiki’s current layout: top `k + offset` by `search::score` per set; merge, remove duplicate record ids, cut. A set with no answer within `SEARCH_SHARD_TIMEOUT_MS` is listed in `partial` with reason `timeout`.
- A search after a job passes that job’s commit as `min_commit`. Packs report word search as lagging when `lag(db)` is above 0.
- Hydrate: one batched graph read within `SEARCH_HYDRATE_MS`. A hit whose graph row is gone is dropped. No graph answer: hits from search rows (title, `git_path`, heading text, `block_id`), marked `unverified`.
- `mention.norm` exact lookup reads the graph set only. Packs read any in-sync graph copy, with `min_lsn` of the job that produced them.
- Library API only (`venus_graph::search`, `venus_graph::exact`), started inside `venus-graph serve`.



#### Do not

- Read from a `lagging` or `joining` copy.
- Run `@@` on a graph node.
- Read Postgres per query.
- Serve the browser.



#### Test scenarios


| Name          | Pass                                                                                                                            |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Merged words  | The query returns both fixture headings, one from each shard, no duplicates. :28730 has no `SEARCH` index.                      |
| Exact         | `norm` on the graph finds the UUID while both search nodes are stopped.                                                        |
| Spread        | 1 000 reads of one set split between its two copies within 40–60%.                                                             |
| Failover      | Stop :28731. `@@` returns both docs from :28732 in the same request. No `repl_copy` or `repl_node` row changes.                  |
| Hedge         | Pause the copy the router picked. The answer comes from the other copy after about p95, not after the timeout. One hedge sent. |
| Fresh read    | A read with `min_lsn` of the last write never answers from a copy behind it; it fails over.                                    |
| Fresh words   | Pause :28732, project a new heading for doc 0, resume :28732 without catch-up done. `@@` with `min_commit` of that job never returns the old heading; with no copy at it, the shard is in `partial` with reason `behind`. |
| Partial       | Pause both copies of shard 1. Within 2 s, hits from shard 0 return and shard 1 is in `partial` with reason `timeout`.          |
| Pack lag      | With shard 1 queued, a pack says word search lags; after apply it does not.                                                     |
| Graph down    | Stop :28730. `@@` returns both headings, marked `unverified`.                                                                   |
| Dropped hit   | Delete doc 0 on the graph only. Its search hit is not returned.                                                                |
| Postgres down | Close the router’s Postgres connection after the map loads. Reads keep answering from the cached map.                          |
| Map reload    | Change a `repl_copy` row. The router sees it within 1 s by `NOTIFY`, and within 30 s with `NOTIFY` blocked.                     |


- **How:** `cargo test -p venus-graph --test query` with profile `graph` up.




<a id="9-step-recover"></a>

### 9. step-recover

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · **9** · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 9                                                       |
| **id**        | `step-recover`                                          |
| **title**     | Catch-up, snapshot, takeover, divergence, retention     |
| **dependsOn** | `step-project`                                          |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Catch-up ([scale.md](../scale.md#catch-up)): on `gap at N` or a `lagging` copy, read `_repl_log:N+1..` from an in-sync copy of that set, `REPL_CATCHUP_BATCH` entries per range read, and replay each as a guarded write in order on that set’s queue. At the head, the copy rejoins the live stream as `in_sync`.
- Snapshot ([scale.md](../scale.md#snapshot)) when a copy is new, empty, behind the oldest retained entry, or divergent: read `L0` and its fence from the source, `GET /export`, recreate the target database, `POST /import`, set `_repl:state`, catch up from `L0`. At most `REPL_MAX_BUILDS_PER_NODE` snapshots into one node at once; the rest queue.
- Takeover ([scale.md](../scale.md#writer-takeover)): on claim, read `_repl:state` from `copies − ack + 1` copies when `ack > 1`, else from every reachable copy. Fewer answer: that set’s writes wait. Store the new `fence` on each copy in that read with `UPDATE _repl:state SET fence = $fence WHERE applied_lsn = $seen AND fence < $fence` before any data entry and before the reference is chosen. Zero rows: read again. Continue only when each of those copies has the new fence and `applied_lsn` is unchanged since the fence write. On a graph set the highest `(applied_lsn, applied_fence)` in that stable majority is the reference. Propagate its last entry to the other copies before any new lsn. An entry that majority does not contain is left on its copy as divergent. A `lagging` copy is not picked while enough others answer.
- A search shard does not take its ahead copy as the source for the sibling. Empty that copy and refill it from `_layout_item` and `_layout_commit`. Continue from a copy that can still ack.
- Occupied lsn ([scale.md — when a write does not reach ack](../scale.md#when-a-write-does-not-reach-ack)): `Ok(lsn)` only at `ack`. While a body has committed on fewer copies than `ack` and a resend can still succeed, that lsn stays occupied and the same entry is resent (`already` on a copy that has it). The caller does not submit a second body. When the other copies rejected the body itself, restore the copies that committed back to `prev`, roll the tip back, and return the error. `lost` is not called.
- Hole fill: when every copy rejected an entry and a newer lsn is already issued, commit an empty `_repl_log` row at the failed lsn with tag `hole`, then resend the successors. `rollback_head` runs only when that lsn is still the tip and nobody committed it. `lost` ignores `hole`.
- Divergence ([scale.md](../scale.md#divergence)): on `divergent`, find the common point (highest `lsn` where the copy’s and the reference’s `(lsn, fence)` agree). A graph copy is snapshotted from the reference. On the commit set, pass the tags past that point to `lost(set, tags)`, skipping `hole`. The owner’s re-run is a new lsn after that tail is outside the reference. A shard copy is emptied and refilled from the commit set. Do not call the owner. Do not read the sibling shard’s `_repl_log`.
- No copy left on a shard, or its cursor below the oldest retained commit entry: the layout refills it from `_layout_item` ([scale.md — refill](../scale.md#refill)). No copy left on the commit set: call `rebuild(set)`.
- Takeover on the layout: the new writer reads each shard’s `_layout:cursor` and applies `_layout_commit` after it in order. A delete is in that list.
- Retention ([scale.md](../scale.md#log-retention)): delete `_repl_log:..=X` where every copy of the set has applied `X`, and anything past `REPL_LOG_RETAIN` (24 h, or beyond the newest 1M entries). A copy behind the oldest entry returns through a snapshot. Commit-set retention does not wait for shard cursors.



#### Do not

- Let one copy connect to another.
- Refill a search shard from a sibling shard, or catch a divergent shard copy up from that sibling’s `_repl_log`.
- Pick a lagging copy as the takeover reference while enough others answer.
- Append a data entry on takeover before the new fence is stored on the read set.
- Allocate a new lsn while a body that committed on fewer copies than `ack` is still on a copy.
- Leave an lsn with no `_repl_log` row under an issued successor.
- Run catch-up and live writes for one set on different queues.
- Trim entries a reachable copy has not applied, unless they are past `REPL_LOG_RETAIN`.
- Move a copy to another node (step 11).



#### Test scenarios


| Name           | Pass                                                                                                                            |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Catch-up       | Stop :28732, project a new body for doc 0, start :28732. Its next entry gets `gap at N`. Only `_repl_log` entries after N are replayed from :28731. Copy `in_sync`. Green. |
| Batch          | :28732 misses 1 200 entries. Catch-up uses three range reads of at most 500.                                                     |
| Wiped, log kept | Remove :28732’s volume while the log still starts at 1. The copies return by replay from 1, without a snapshot.                |
| Wiped, trimmed | Remove :28732’s volume after the log is trimmed. The copies return by snapshot from :28731 plus catch-up, with the same `(applied_lsn, applied_fence)` and rows. |
| Fuzzy export   | Writes continue during the export. After catch-up from `L0`, both copies have the same rows.                                    |
| Build cap      | Three snapshots into one node: never more than two at once.                                                                      |
| Takeover       | Stop the writer after :28731 committed an entry and before :28732 did. A second process claims the lease, stores the new fence on the reachable copies with `applied_lsn` unchanged, picks :28731 as reference, and fills :28732. No acked entry is missing. |
| Fence first    | After claim and before the first data entry, every copy in the read set has `_repl:state.fence` equal to the new fence. An entry that still carries the old fence is refused `fenced` and does not advance `applied_lsn`. |
| Occupied lsn   | `ack` 2. One copy commits, the other rejects the body. The committed copy is restored to `prev`, the caller receives the error, and a retry is a single new log row. The rejected body is absent. A resendable miss (transport) keeps the same lsn until the second copy answers `already` or `ok`. |
| Hole           | Two overlapping writes. The older body fails on every copy after the newer lsn is issued. An empty `_repl_log` row with tag `hole` occupies the older lsn, the newer body commits after it, and both copies have a contiguous log. |
| Divergent      | Commit an entry on :28732 only under the old fence. Pause :28732, take over (reference :28731), write one entry, resume :28732. Its next write replies `divergent`. The owner hook is not called. :28732 is rebuilt by snapshot from :28731. This set is its own source of truth, not a layout shard. |
| Shard divergent | One copy of shard 1 has an entry the other does not. Takeover continues from the copy that can still ack. The ahead copy is emptied and refilled from `_layout_item` and `_layout_commit`. Shard 0 is not read. The owner hook is not called. |
| Rebuild        | Wipe both copies of shard 0. The layout refills it from `_layout_item` (`key % 2 = 0`). Shard 1 is not read. The owner hook is not called. |
| Refill after trim | Stop both copies of shard 1, write and trim the commit set past shard 1’s cursor, start them. Shard 1 is refilled from `_layout_item` into empty databases, then catches up; a doc deleted during the outage is gone, and no `_repl_log` read goes below `L0`. |
| Queued takeover | Pause shard 1, write two commits, stop the writer. A second process claims the lease and applies both commits to shard 1 in order from its cursor. |
| Retention      | After every copy applies 100 entries, they are deleted with one range delete. With :28732 down, entries it lacks stay until `REPL_LOG_RETAIN`. |
| Retention cap  | With `REPL_LOG_RETAIN` set to 50 entries and :28732 down, the log keeps the newest 50; :28732 returns by snapshot.                  |


- **How:** `cargo test -p surrealastic --test recover --features fault` and `cargo test -p venus-graph --test hooks`, with profile `graph` up.




<a id="10-step-monitor"></a>

### 10. step-monitor

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · **10** · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 10                                                       |
| **id**        | `step-monitor`                                          |
| **title**     | Monitor, node states, writer reactions                  |
| **dependsOn** | `step-recover`                                          |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Monitor ([scale.md — failure detection](../scale.md#failure-detection)) in `venus-graph serve`, under lease `monitor` (10 s, renewed every 3 s). Any `serve` process can take it over.
- Probe every `REPL_PROBE_MS`: `GET /health` plus a one-row read of `_repl:state` in one database on that node.
- States: `up` → `suspect` after 3 missed probes; `suspect` → `down` after `REPL_DOWN_MS`; any → `up` on the first good probe. `draining` is set by an operator and left alone. Each change writes `repl_node.state` and `state_at` and sends `NOTIFY repl_map`.
- Writers react: `down` → stop sending to that node’s copies, mark them `lagging`, start the replacement delay (used in step 11); `up` → catch up its lagging copies.
- Routers prefer copies not on `suspect` nodes.
- Position sweep: every 30 s the writer reads `_repl:state` of copies it has not written recently and updates `repl_copy.applied_lsn` when it changed.



#### Do not

- Let the monitor write data or copy state.
- Move a copy because a node is `suspect`.
- Run two monitors at once.



#### Test scenarios


| Name             | Pass                                                                                                         |
| ---------------- | ------------------------------------------------------------------------------------------------------------ |
| Monitor          | Stop :28732. It is `suspect` after about 3 s and `down` after 10 s, and its copies are `lagging`. Start it: `up`, then its copies catch up and are `in_sync`. |
| Suspect          | Pause :28732 for 5 s. It goes `suspect`, not `down`. Reads go to :28731. Nothing moves.                         |
| One monitor      | Two `serve` processes: one holds `monitor`. Stop it. The other takes the lease within 10 s and keeps probing.|
| Down skips       | While :28732 is `down`, writes ack on :28731 and send nothing to :28732.                                        |
| Sweep            | A copy not written for 30 s has its `repl_copy.applied_lsn` refreshed from its own `_repl:state`.           |
| Health           | Green with all up; yellow with :28732 down; red for search with both search nodes down; green again after restart. |


- **How:** `cargo test -p venus-graph --test monitor` with profile `graph` up.




<a id="11-step-third-node"></a>

### 11. step-third-node

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · **11** · [12](#12-step-graph-copies) · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 11                                                      |
| **id**        | `step-third-node`                                       |
| **title**     | Third search node joins; replacement; draining          |
| **dependsOn** | `step-monitor`                                          |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Service `surreal-search-2`: `127.0.0.1:28733`, its own volume, same image, cap, and search schema. Node 3, zone 3. `shard_count` and `replica_count` stay 2 and 1.
- Migration in `crates/surrealastic` (additive to step 4’s tables): sequence `repl_node_seq`, and `removed` added to the `repl_node.state` check. Placement members exclude `draining` and `removed`. Step 4’s placement tests keep passing unchanged.
- Join ([scale.md — replacement and join](../scale.md#replacement-and-join)): insert `repl_node` 3 with its id from `repl_node_seq` (an id that has a row is refused), pool `search`, `NOTIFY repl_map`. Each writer recomputes rendezvous homes. Where node 3 is now a home, a `joining` copy is filled by snapshot from an in-sync copy plus catch-up and becomes `in_sync`. Only then is the copy that dropped out removed and its database dropped.
- Replacement: a node `down` past `REPL_REALLOCATE_DELAY_MS` gets, for each of its sets, a `joining` copy on the next ranked node that is not `down`, not draining, and in an unused zone, filled by snapshot and catch-up. When the original home is back and `in_sync`, the replacement is removed. When an operator sets the dead node `removed`, the replacement is already the next home and stays.
- Draining: an operator sets `draining`. Each set with a copy there gets a copy on its next home, then the draining copy is removed. When the node holds nothing, its state becomes `removed`; the row stays.



#### Do not

- Change `shard_count` or rehash `doc_id`s.
- Set `replica_count` to 2.
- Put two copies of one shard on one node.
- Let search nodes connect to each other.
- Remove a copy before its successor is `in_sync`.
- Move copies when a node is `down` for less than the delay.
- Delete a `repl_node` row or reuse a `node_id`.



#### Test scenarios


| Name             | Pass                                                                                                                       |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Third node       | :28733 is healthy and has the search schema. `layout_db.replica_count` is still 1.                                          |
| Join moves few   | The copies that moved are exactly the rendezvous difference between two and three nodes. No other copy is rewritten.      |
| Restart in delay | Stop and start a search node inside the delay. No new copy is placed. Its copies catch up.                                 |
| Re-replicate     | With a search node stopped past the delay, each shard has two `in_sync` copies on the other two nodes. Green.              |
| Back home        | Start that node again. Its copies catch up, then the replacements are removed. Homes match rendezvous again.               |
| Remove dead node | With a node stopped past the delay and its replacements `in_sync`, set it `removed`. No copy moves; the replacements stay as homes. |
| Draining         | Drain a search node. Every set keeps two `in_sync` copies throughout. The node ends with no copies and state `removed`.     |
| No id reuse      | After a node is `removed`, the next join gets a new `node_id` from `repl_node_seq`; inserting the removed id is refused.     |
| No rehash        | The same fixture `doc_id` still maps to the same shard number.                                                             |


- **How:** `cargo test -p venus-graph --test third_node` with profile `graph` up and `REPL_REALLOCATE_DELAY_MS` set low.




<a id="12-step-graph-copies"></a>

### 12. step-graph-copies

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · **12** · [13](#13-step-backup) · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 12                                                      |
| **id**        | `step-graph-copies`                                     |
| **title**     | Three graph copies, ack 2 of 3                          |
| **dependsOn** | `step-monitor`                                          |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Services `surreal-graph-1` (`127.0.0.1:28734`) and `surreal-graph-2` (`:28735`): own volumes, same image and cap, graph schema only. `repl_node` 4 and 5 in pool `graph`, zones 4 and 5.
- Graph set policy ([scale.md — replica sets and policy](../scale.md#replica-sets-and-policy)): `GRAPH_COPIES` (1 by default, 3 on this profile), `ack` a majority. The new copies join by snapshot from :28730 plus catch-up.
- Takeover reads `_repl:state` from 2 of 3, stores the new fence on those copies, and re-reads any copy whose `applied_lsn` moved, before it appends. Fewer reachable: graph writes wait; reads continue from any in-sync copy.
- A graph body that committed on one copy and was rejected by another stays on that lsn when a resend can succeed, and is restored away when it cannot. The job is re-run from `lost` only after a divergent tail is outside the reference. Propagate an unacked graph entry only when it is the tip of the majority reference, and do it before any new lsn.
- Graph `lost(set, tags)` hook: re-run the jobs named in the tags (fixture jobs here; `graph_jobs` rows once step 7 is done). Skip the tag `hole`.
- Packs and hydrate pick any in-sync graph copy through the router, with `min_lsn` when they must see a job.



#### Do not

- Ack a graph entry on one copy when `copies` is 3.
- Pick a graph takeover reference without a majority.
- Append a graph entry before the new fence is stored on the two copies the reference was chosen from.
- Submit a second graph body while the first has committed on fewer than two copies and is still on one of them.
- Put two graph copies in one zone.
- Rebuild the graph from markdown while a graph copy exists.



#### Test scenarios


| Name           | Pass                                                                                                                          |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Majority ack   | Stop :28735. Graph entries ack on :28730 and :28734. Stop :28734 too: graph writes wait, reads still answer from :28730. Yellow, then red. |
| No lost ack    | Ack an entry on two copies, then wipe one of them. The entry is on the reference after takeover and on the rebuilt copy.     |
| Divergent      | Commit an entry on :28735 only under the old fence (not acked). Pause :28735, take over on :28730 and :28734, write one entry, resume :28735. It replies `divergent`, its job reaches `lost` and is re-run, and :28735 is rebuilt by snapshot to the reference’s `(applied_lsn, applied_fence)`. The reference log does not also contain that old body. |
| Short of ack   | `ack` 2. One graph copy commits, another rejects the body, the third is down. The committed copy is restored to `prev`. `lost` is not called. The retry acks on two copies as one new lsn, and the rejected body is absent. |
| Quorum wait    | With only :28730 reachable, a new writer’s takeover waits. When :28734 returns, it picks the higher of the two.                |
| Fresh read     | A read with `min_lsn` of the last acked entry never returns from a graph copy behind it.                                     |


- **How:** `cargo test -p venus-graph --test graph_copies --features fault` with profile `graph` up.




<a id="13-step-backup"></a>

### 13. step-backup

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · **13** · [14](#14-step-split)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 13                                                      |
| **id**        | `step-backup`                                           |
| **title**     | Log archive, nightly export, point-in-time restore      |
| **dependsOn** | `step-graph-copies`                                     |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Object store through the `object_store` crate at `REPL_ARCHIVE_URL`: `file://` in dev and tests, `s3://` in production. Per-set policy `archive`: on for graph sets, off for search sets.
- Archive ([scale.md — log retention](../scale.md#log-retention)): before trimming `_repl_log:A..=B` on a graph set, write that range to `{set_id}/log/{A}-{B}`. Trim only after the write succeeded. Archive unreachable: no trim, the log grows, health yellow.
- Nightly export ([scale.md — backups](../scale.md#backups)) at `REPL_BACKUP_AT` (03:00 UTC): from one in-sync graph copy, read `L0` and its fence, `GET /export`, write `{set_id}/base/{L0}`.
- Restore `restore(set, upto_lsn)`: import the newest base at or below `upto_lsn` into an empty copy, replay archived and live entries up to `upto_lsn`, set `_repl:state`.
- Graph `rebuild(set)` hook: restore to the newest `lsn` in the archive, then catch up from any live copy; with no backup, re-run fixture jobs (the extractor comes next board).



#### Do not

- Trim a graph entry that is not archived.
- Archive search sets.
- Rebuild a graph from markdown while a backup exists.



#### Test scenarios


| Name            | Pass                                                                                                                  |
| --------------- | --------------------------------------------------------------------------------------------------------------------- |
| Archive         | After retention runs, every trimmed graph range is in the archive, contiguous from 1, and no search range is.         |
| Archive down    | With the archive path unwritable, nothing is trimmed and health is yellow. When it returns, the backlog is archived and trimmed. |
| Nightly         | A forced run writes `{set_id}/base/{L0}` from an in-sync copy while writes continue.                                 |
| Point in time   | Restore to `lsn` N into a scratch database: rows equal a copy taken at N; entry N + 1’s change is absent.            |
| All graph lost  | Wipe all three graph copies. The `rebuild` hook restores from base and archive. Every acked entry and every edge is back. |


- **How:** `cargo test -p venus-graph --test backup` with profile `graph` up and `REPL_ARCHIVE_URL` pointing at a temp dir.




<a id="14-step-split"></a>

### 14. step-split

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-repl-core) · [5](#5-step-layout) · [6](#6-step-project) · [7](#7-step-enqueue) · [8](#8-step-query) · [9](#9-step-recover) · [10](#10-step-monitor) · [11](#11-step-third-node) · [12](#12-step-graph-copies) · [13](#13-step-backup) · **14**


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 14                                                      |
| **id**        | `step-split`                                            |
| **title**     | Split by doubling, epoch rebuild                        |
| **dependsOn** | `step-third-node`                                       |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Split `c → 2c` ([scale.md — shards](../scale.md#shards)):
  1. Set `shard_count_next = 2c`. Create sets `search:{ws}:{epoch}:{s + c}` on their rendezvous homes. Fill them in one refill pass over `_layout_item`: set `s + c` takes the items whose `key % 2c = s + c`.
  2. Live writes apply to both shard counts. Reads stay on `c`.
  3. When every new set has an in-sync copy whose cursor is at the commit head, flip in one Postgres transaction: `shard_count = 2c`, `shard_count_next = null`.
  4. One entry per old set deletes the items where `key % 2c ≠ s`, by the `rids` stored in `_layout_item`. The router removes duplicate ids until then.
- Rebuild (any other change, including a shrink): `epoch + 1`, new sets filled by a refill pass over `_layout_item`, dual writes, flip, then drop the old epoch’s sets and databases. The owner is not called.
- Trigger: the writer’s 30 s sweep starts a split when a shard copy is over 2 GB, over 500k headings, or its `@@` p95 is over `SEARCH_P95_TARGET_MS`. Thresholds are environment variables. Never past 64 shards.



#### Do not

- Flip `shard_count` before every new set has an in-sync copy at its head.
- Fill a new shard from a sibling shard.
- Ask the owner to refill a shard.
- Rehash `doc_id` or key shards by node count.
- Stop writes or reads for a split.



#### Test scenarios


| Name          | Pass                                                                                                                 |
| ------------- | -------------------------------------------------------------------------------------------------------------------- |
| Split         | Fixture wiki 2 → 4 under continuous writes, with the commit log trimmed before the split. Old shards 0 and 1 keep their sets; only sets 2 and 3 are built, from `_layout_item`. Afterwards each doc is on exactly `hkey % 4`, and old shards hold none of the moved `rids`. |
| No early flip | With one new set’s copy held behind its head, the flip does not happen and reads stay on 2 shards.                   |
| Reads during  | `@@` during the split returns each fixture once.                                                                     |
| Shrink        | 4 → 2 builds epoch 1, flips, and drops epoch 0’s databases. Reads never miss a doc.                                  |
| Trigger       | With the heading threshold at 100, 150 bulk headings in one shard start a split.                                     |
| Cap           | A split past 64 shards is refused.                                                                                   |


- **How:** `cargo test -p venus-graph --test split` with profile `graph` up.




## After this board

Next board: [extraction](../extraction/plan.md) reads committed markdown and the sidecar into the graph, then projects each doc with this writer. The graph `rebuild` hook gains re-extraction from git. Heading vectors are [board 5](../vectors/plan.md). The semantic model is [board 6](../semantic/plan.md). NER stays off on the extraction board.
