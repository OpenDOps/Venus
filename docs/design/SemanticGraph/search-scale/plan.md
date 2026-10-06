# Search cluster

| | |
|---|---|
| **planId** | `ss-cluster` |
| **Milestone** | First board of [SemanticGraph](../README.md). Search design: [README](./README.md). |
| **Duration** | About 1 week, plus the third node after the two-node cluster is green |
| **Board** | [SS.state.yaml](./SS.state.yaml) |

Graph role and write order: [scale.md](../scale.md). Graph schema: [store.md](../store.md). Flush queue this must not join: [LiveSnapshot HA](../../LiveSnapshot/high-availability.md).

This board starts the search cluster. Each search node is a full SurrealDB server with the **search schema only**. The graph is a separate process. HA default is three search nodes and `replica_count = 1` ([README — high availability](./README.md#high-availability)). Steps 1–5 ship two nodes so a copy can be lost and promoted. Step 7 adds the third node so a replacement replica can land while the dead node is still gone.

## Shape

```text
surreal-graph :8000             no SEARCH index
surreal-search-0 :8001          primary shard 0, replica shard 1
surreal-search-1 :8002          primary shard 1, replica shard 0
```

`shard = sha256(doc_id) % shard_count`. Step 7 adds `surreal-search-2` (`:8003`) without changing `shard_count` or `replica_count`.

## Gate

| Steps | When |
|---|---|
| **1–5, 7** | Now. They do not change Flush. |
| **6** | [M4](../../M4/README.md) **closed**. Hook after `last_flushed`. |

## Exit

1. Profile `graph` starts `surreal-graph` (`127.0.0.1:8000`), `surreal-search-0` (`:8001`), and `surreal-search-1` (`:8002`). Three volumes. Search nodes have no graph namespace.
2. Allocation: `shard_count = 2`, `replica_count = 1`, crossed as in the diagram. One primary per shard.
3. Two fixture docs, one per shard, write the graph, then the primary, then the replica, then `page.search_sha`.
4. `@@` merges both shards and does not run on :8000. Stopping a replica is yellow. Stopping a primary promotes the in-sync replica.
5. A wiped search volume is rebuilt from the other copy of **that** shard, or from the graph if no copy remains. The other shard is not a source.
6. After M4 is closed, flush upserts `graph_jobs` and still commits if every SurrealDB process is down.
7. A third search node accepts a new replica after one node is stopped, `replica_count` still 1, `shard_count` still 2.
8. No LLM, no tree-sitter, no glossary scan, no graph UI. M1–M4 tests stay green.

## Non-goals

| Later | Why not here |
|---|---|
| `replica_count = 2` | Not the HA default. |
| Raising `shard_count` | Rebuild from the graph under a new modulus. [README](./README.md#cluster). |
| Headings from sidecar, glossary, tree-sitter, regex | Next board. Fixtures only. |
| Semantic model, NER, HNSW | Later. |
| SurrealDB Enterprise, Raft between search nodes | Allocation table is the cluster state. |

## Constraints

1. Search nodes run `surrealdb/surrealdb:v2`. Schema is namespace `search` only.
2. Order: graph commit, primary commit, replica apply, stamp `search_sha`. Stamp after the primary. A failed replica stays yellow.
3. One writer per wiki. One primary per shard. Primary and its replica are on different nodes.
4. Rebuild a shard from its other copy, or from the graph. Not from a sibling shard. Not from markdown while the graph exists.
5. Block cache is set from each container’s cap (1 GB cap, 64 MB cache).
6. Hub and browser do not open these ports.
7. `graph_jobs` is not flush `jobs`.

## Steps summary

| # | Step | Proves |
|---|---|---|
| 1 | [step-recon](#1-step-recon) | Full binary, search schema only. P=2, R=1. HA target is 3 nodes, R=1. |
| 2 | [step-compose](#2-step-compose) | Graph + two search nodes, three volumes. |
| 3 | [step-schema](#3-step-schema) | Search indexes on :8001 and :8002. Allocation rows. |
| 4 | [step-project](#4-step-project) | Fixture doc to its primary, then its replica. |
| 5 | [step-query](#5-step-query) | Merged `@@`, yellow, promote, rebuild from the other copy. |
| 6 | [step-enqueue](#6-step-enqueue) | `graph_jobs` after flush. M4 closed. |
| 7 | [step-third-node](#7-step-third-node) | Third node takes a replica after one node dies. R stays 1. |

## Steps

### 1. step-recon

#### Work

- Pin `surrealdb/surrealdb:v2` and the Rust `surrealdb` client to that 2.x line. Client only. Do not embed RocksDB.
- Record that a search node is that full server and only the [search schema](./README.md#schema). No graph namespace on its volume.
- Record dev shape P=2, R=1, two nodes, and the HA default: three nodes, `replica_count` stays 1 ([README](./README.md#high-availability)).
- Record flush `jobs` (`workspace_id` primary key, `reason` `idle` | `flush` | `lease`). `graph_jobs` is step 6.

#### Do not

- Add a full-text index to the graph schema.
- Set `replica_count = 2` as the default.
- Change Flush.

#### Test scenarios

| Name | Pass |
|---|---|
| Binary | This plan says search nodes use `surrealdb/surrealdb:v2` and do not create namespace `graph`. |
| HA default | This plan’s exit 7 is three nodes and `replica_count = 1`, not `replica_count = 2`. |

### 2. step-compose

#### Work

- Profile `graph`.
- `surreal-graph`: `127.0.0.1:8000`, volume `surreal-graph-data`, `rocksdb:/data/graph.db`, `mem_limit` 1g, `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` 64 MB.
- `surreal-search-0`: `127.0.0.1:8001`, volume `surreal-search-0-data`, `rocksdb:/data/search.db`, same cap and cache.
- `surreal-search-1`: `127.0.0.1:8002`, volume `surreal-search-1-data`, same command and cap.
- User and pass from Compose env. Healthcheck per port. `pids_limit` and dropped capabilities, same posture as `hub`.

#### Do not

- Start these on the default Compose path.
- Publish `0.0.0.0`.
- Share one volume between search nodes.
- Size the cache from the host’s RAM.

#### Test scenarios

| Name | Pass |
|---|---|
| Profile off | `docker compose up` without `--profile graph` starts none of the three. |
| Three processes | With the profile, :8000, :8001, and :8002 are healthy, 1g each. |
| Independent disk | A row on :8001 survives a recreate of :8002. |

### 3. step-schema

#### Work

- Crate `crates/venus-graph`. Apply [store.md](../store.md#schema) to :8000 only.
- Apply the [search schema](./README.md#schema) to :8001 and :8002. Idempotent.
- Seed `search_cluster` (`shard_count = 2`, `replica_count = 1`) and four `search_allocation` rows: shard 0 primary on node 0, replica on node 1; shard 1 primary on node 1, replica on node 0.
- `page.search_sha` and `page.search_error` live on the graph `page` table.
- The graph migrator refuses a `SEARCH` index. Search migrator does not create `links_to`.

#### Do not

- Create namespace `graph` on a search node.
- Open these sockets from `venus-hub` or `apps/web`.

#### Test scenarios

| Name | Pass |
|---|---|
| Graph has no index | :8000 has `links_to` and no full-text index. |
| Search has no graph | :8001 and :8002 have `heading_text` and `mention_text` and no `links_to`. |
| Allocation | The four rows above. A second migrate does not add a second primary. |

### 4. step-project

#### Work

- Two fixture docs on `surreal-graph`, chosen so `sha256(doc_id) % 2` differs. Each: one heading with `body`, one `mention` kind `uuid`. No file read.
- Project each doc onto its primary, then apply that transaction to its replica.
- Stamp `search_sha` after the primary commits. Replica `in_sync`, or `stale` if the apply failed.
- Primary down: graph rows remain, `search_sha` unset, `search_error` set.

#### Do not

- Parse `wiki/`.
- Stamp before the primary commits.
- Roll back graph rows because a replica failed.

#### Test scenarios

| Name | Pass |
|---|---|
| Both shards | Doc 0 is on :8001 as primary and :8002 as replica. Doc 1 is the swap. Health is green. |
| Primary down | Stop shard 0’s primary before the project. Graph heading remains. `search_sha` is empty. |
| Replica down | Stop shard 0’s replica. Primary has the body. `search_sha` is set. Replica is `stale`. Health is yellow. |

### 5. step-query

#### Work

- `@@` on one in-sync copy of each shard. Merge scores. Hydrate from the graph. Do not run `@@` on :8000.
- `mention.norm` on the graph only.
- Yellow path uses the primary when the replica is `stale`.
- Stop `surreal-search-0`. Promote shard 0’s replica on :8002. `@@` returns doc 0.
- Rebuild: delete one search volume. Restore that shard from its other copy. If no copy remains, restore from graph rows in that shard only.

#### Do not

- Rebuild shard 0 from shard 1’s rows.
- Leave two primaries after promotion.
- Serve the browser.

#### Test scenarios

| Name | Pass |
|---|---|
| Merged words | The query returns both fixture headings. :8000 has no `SEARCH` index. |
| Exact | `norm` on :8000 finds the UUID while both search nodes are stopped. |
| Promote | After :8001 is stopped, shard 0’s replica on :8002 is primary and `@@` returns doc 0. |
| Sibling | Wiping shard 0’s only remaining copy does not fill it from shard 1. A graph rebuild of shard 0 does. |

### 6. step-enqueue

**Gate:** M4 closed.

#### Work

- Table `graph_jobs`: `workspace_id` primary key, `wiki_sha`, dirty doc ids, claim fields, `state` `pending` | `failed`. Hub does not insert or claim it.
- Sidecar, after git and `last_flushed` have committed: upsert `graph_jobs`. Coalesce a pending row. A failed upsert does not fail the flush.
- `venus-graph` claims with `SKIP LOCKED` and runs the projector. No markdown parse yet: a claim with no fixture records the SHA and exits.

#### Do not

- Insert into flush `jobs`.
- Hold the pin cut.
- Start tree-sitter or the glossary scan.

#### Test scenarios

| Name | Pass |
|---|---|
| Row after flush | One `graph_jobs` row carries that `wiki_sha`. `last_flushed` matches. |
| Cluster down | Stop graph and both search nodes. Flush still commits. `graph_jobs` still upserts. |
| One claim | Two workers: one owner for that `workspace_id`. |

### 7. step-third-node

#### Work

- Service `surreal-search-2`: `127.0.0.1:8003`, its own volume, same image, cap, and search schema. `shard_count` and `replica_count` stay 2 and 1.
- Stop `surreal-search-0`. Promote where this board already does. Allocate the missing replica of each shard that has only one copy onto node 2. Copy from the surviving primary.
- Health returns to green without starting node 0 and without a second primary.

#### Do not

- Change `shard_count` or rehash `doc_id`s.
- Set `replica_count` to 2.
- Put the new replica on the node that holds that shard’s primary.

#### Test scenarios

| Name | Pass |
|---|---|
| Third node | :8003 is healthy and has the search schema. `search_cluster.replica_count` is still 1. |
| Re-replicate | With :8001 stopped, each shard has a primary and one `in_sync` replica, and the replica is not on the primary’s node. One of those replicas is on :8003. |
| No rehash | The same fixture `doc_id` still maps to the same shard number. |

## After this board

Next board: read committed markdown and the sidecar into the graph, then project each doc with this writer. NER and the semantic model stay behind that.
