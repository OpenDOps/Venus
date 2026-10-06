# Semantic graph — index split

| | |
|---|---|
| **planId** | `sg-store` |
| **Milestone** | First board of [SemanticGraph](./README.md). The SurrealDB **split**, nothing else. Parallel to M5. |
| **Duration** | About 1 week |
| **Board** | [SG.state.yaml](./SG.state.yaml) |

Design: [scale.md](./scale.md), [store.md](./store.md). Flush queue this must not join: [LiveSnapshot HA](../LiveSnapshot/high-availability.md).

This board starts with the split **and** the search cluster. Edges live on one SurrealDB process. The full-text index lives on a two-node cluster: two primary shards, one replica each, placed by an allocation table ([scale — search cluster](./scale.md#search-cluster)). A hand-built page is enough to prove a write to the right primary, a copy to its replica, and a query that merges both shards. Glossary, tree-sitter, regex, and the semantic model are the **next** board.

## The split

```text
venus-graph
    │  1. commit edges on surreal-graph :8000
    │  2. project the doc onto its primary shard
    │  3. apply that write to the replica
    │  4. stamp page.search_sha
    ▼
surreal-search-0 :8001          surreal-search-1 :8002
  primary shard 0                 primary shard 1
  replica shard 1                 replica shard 0
```

`shard = sha256(doc_id) % 2`. The modulus is `shard_count`, not “how many containers are up”.

| | `surreal-graph` | each search node |
|---|---|---|
| Holds | `RELATE`, heading `body`, `mention.norm` | `SEARCH` indexes for the shards allocated to it |
| Does not hold | A full-text index | Edges |
| Cap | 1 GB, block cache 64 MB | 1 GB, block cache 64 MB, per node |

## Gate

| Steps | When |
|---|---|
| **1–5** | Now. They do not change Flush. |
| **6** | [M4](../M4/README.md) **closed**. This is the hook after `last_flushed`. |

## Story

As an implementer I need the search index on a cluster before any wiki parser exists. Two fixture docs land on different shards. Each is written to that shard’s primary and copied to its replica. A word query merges one copy of each shard. Stopping a replica leaves the cluster yellow and the edges intact. Stopping a primary promotes the in-sync replica.

## Exit

1. Profile `graph` starts `surreal-graph` (`127.0.0.1:8000`), `surreal-search-0` (`:8001`), and `surreal-search-1` (`:8002`). Three volumes. Three memory caps. Default `postgres` + `hub` + `web` starts none of them.
2. Allocation for the dev workspace: `shard_count = 2`, `replica_count = 1`. Shard 0 primary on node 0, replica on node 1. Shard 1 primary on node 1, replica on node 0. A node does not hold both roles of one shard.
3. Graph schema on :8000 has no `SEARCH` index. Both search nodes have `heading_text` and `mention_text`.
4. Two fixture docs, one per shard, write the graph, then the primary, then the replica, then `page.search_sha`. Primary down for that shard: graph rows remain, `search_sha` unset. Replica down: primary stays, stamp is allowed, that replica is `stale`, health is yellow.
5. `@@` merges both shards and does not run on :8000. `mention.norm` runs on :8000 while a search node is stopped. Promoting an in-sync replica makes `@@` hit that doc again.
6. After M4 is closed, a successful flush upserts `graph_jobs`. The flush `jobs` table is unchanged. All three SurrealDB processes down does not fail the flush.
7. Existing M1–M4 tests stay green. No LLM, no tree-sitter, no glossary automaton, no graph UI.

## Non-goals (do not start)

| Later board | Why not this one |
|---|---|
| Headings from sidecar, `links_to`, glossary, tree-sitter, regex | Next board. This board’s rows are a fixture. |
| Semantic model and gist projection | After extract. [connect](./connect.md#semantic--background-model). |
| ONNX NER, HNSW, LanceDB | Not the split. |
| Raising `shard_count` past 2, or adding a third search node | The allocation table and the rebuild rule are in [scale](./scale.md#search-cluster). This board runs P=2, R=1, two nodes. |
| Two primaries for one shard, or a primary and its replica on one node | [scale — do not](./scale.md#do-not). |
| SurrealDB Enterprise / a built-in RocksDB replica | The cluster is the allocation table plus ordinary SurrealDB processes. |
| AB2 / AB4 / AB5 | Other tracks. |

## Constraints

1. **Search index only on search nodes.** No `DEFINE INDEX … SEARCH` against the graph process.
2. **Order:** graph commit, primary commit, replica apply, stamp `search_sha`. Stamp after the primary commits. A failed replica does not unstamp and does not roll back the primary.
3. **One writer** for this wiki. Replicas are copies of that write, not a second projector. One primary per shard.
4. **Rebuild search from the graph**, not from markdown.
5. **Block cache** is set from each container’s cap, not from the host’s RAM.
6. **Hub and browser** do not open either port.
7. **`graph_jobs` is not `jobs`.** Flush `jobs` stays `workspace_id` plus reason `idle` | `flush` | `lease`. Step 6 inserts `graph_jobs` only after `last_flushed` commits, and a failed insert does not fail the flush.
8. If this plan disagrees with [scale.md](./scale.md) on which process holds the index, **scale wins**. If it disagrees with `crates/venus-hub/src/schema.sql` on the flush `jobs` shape, **the schema wins**.

## Steps summary

| # | Step | Proves |
|---|---|---|
| 1 | [step-recon-split](#1-step-recon-split) | Cluster shape: P=2, R=1, two search nodes. Image pin. |
| 2 | [step-compose-split](#2-step-compose-split) | Graph plus two search nodes, three volumes. |
| 3 | [step-schema-split](#3-step-schema-split) | Graph schema on :8000. Search schema on :8001 and :8002. Allocation rows. |
| 4 | [step-project](#4-step-project) | Each fixture doc hits its primary, then its replica. |
| 5 | [step-query](#5-step-query) | Merged `@@`, yellow on replica loss, promote an in-sync replica. |
| 6 | [step-enqueue](#6-step-enqueue) | `graph_jobs` after flush. M4 must be closed. |

## Steps

### 1. step-recon-split

#### Work

- Pin `surrealdb/surrealdb:v2` to a patch once step 2 pulls it. Pin the Rust `surrealdb` client to that 2.x line. Client only: do not embed RocksDB in `venus-graph`.
- Record the three services, the shard map (`shard_count = 2`, `replica_count = 1`), and a 1 GB cap with a 64 MB block cache on each process.
- Record that flush `jobs` cannot carry this work (`workspace_id` primary key, `reason` check). `graph_jobs` is step 6. `search_allocation` is the cluster state.

#### Do not

- Add a full-text index to the graph schema.
- Start a markdown parser, tree-sitter, or a model.
- Change Flush.

#### Test scenarios

| Name | Pass |
|---|---|
| Split written | This plan’s opening diagram shows two search nodes, crossed primaries and replicas, and no `SEARCH` index on the graph. |
| Queue split | Recon quotes the `jobs` primary key and `reason` check from `schema.sql`. |

### 2. step-compose-split

#### Work

- Profile `graph` in `docker-compose.yml`.
- Service `surreal-graph`: `127.0.0.1:8000`, volume `surreal-graph-data`, `rocksdb:/data/graph.db`, `mem_limit` 1g, `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` 64 MB.
- Service `surreal-search-0`: `127.0.0.1:8001`, volume `surreal-search-0-data`, `rocksdb:/data/search.db`, same cap and cache.
- Service `surreal-search-1`: `127.0.0.1:8002`, volume `surreal-search-1-data`, same command and cap.
- User and pass from Compose env. Healthcheck on each port. `pids_limit` and dropped capabilities, same posture as `hub`.

#### Do not

- Start these services on the default Compose path.
- Publish `0.0.0.0`.
- Put both search nodes on one volume.
- Size the cache from the host’s total RAM.

#### Test scenarios

| Name | Pass |
|---|---|
| Profile off | `docker compose up` without `--profile graph` starts neither SurrealDB service. |
| Three processes | With the profile, all three containers are healthy, on :8000, :8001, and :8002, with a 1g limit each. |
| Independent disk | A row on :8000 survives a recreate of either search node. A row on :8001 survives a recreate of :8002. |

### 3. step-schema-split

#### Work

- Crate `crates/venus-graph`. On startup, apply [store.md](./store.md#schema) to the graph process (namespace `graph`, database = workspace id).
- Apply [scale.md — search schema](./scale.md#search-schema) to **both** search nodes (namespace `search`, same database id).
- Seed `search_cluster` (`shard_count = 2`, `replica_count = 1`) and the four `search_allocation` rows from the diagram. Idempotent.
- `page.search_sha` and `page.search_error` live on the graph `page` table.
- The graph migrator refuses to run a `SEARCH` index statement. Only the search migrator defines `heading_text` and `mention_text`.

#### Do not

- Point both migrators at one URL in this step.
- Open either socket from `venus-hub` or `apps/web`.
- Embed the engine in the hub binary.

#### Test scenarios

| Name | Pass |
|---|---|
| Graph has no index | On :8000, `page`, `heading`, `mention`, `links_to` exist. No full-text index. |
| Both nodes indexed | On :8001 and :8002, `heading_text` and `mention_text` exist. No `links_to`. |
| Allocation | Four rows: shard 0 primary on node 0, replica on node 1; shard 1 primary on node 1, replica on node 0. |
| Twice | Each migrator runs twice without dropping rows or duplicating a primary. |

### 4. step-project

#### Work

- Insert two fixture docs into `surreal-graph`, chosen so `sha256(doc_id) % 2` differs. Each doc: one heading with `body`, one `mention` (`kind = uuid`). No file read.
- For each doc, project onto the **primary** of its shard, then apply that transaction to the **replica**.
- Set `page.search_sha` after the primary commits. Mark the replica `in_sync` at that sha, or `stale` if the replica apply failed.
- If the primary refuses the write: graph rows remain, `search_sha` unset, `search_error` set. Retry does not delete the graph heading.

#### Do not

- Parse `wiki/`.
- Stamp `search_sha` before the search commit.
- Roll back edges because search failed.

#### Test scenarios

| Name | Pass |
|---|---|
| Both shards | Doc 0’s body is on :8001 (primary) and :8002 (replica). Doc 1’s body is on :8002 (primary) and :8001 (replica). Both `search_sha` values match. Health is green. |
| Primary down | Stop the primary of shard 0 before its project. The graph heading remains. `search_sha` is empty. |
| Replica down | Stop the replica of shard 0. The primary still has the body. `search_sha` is set. That replica row is `stale`. Health is yellow. |
| Retry | Start the stopped node. The retry copies the shard. The graph heading is unchanged. |

### 5. step-query

#### Work

- Word query: `@@` against one in-sync copy of shard 0 and one of shard 1. Merge by score. Hydrate ids from the graph. Do not query :8000 for words.
- Exact lookup: `mention.norm` on the graph only.
- Lag: commit the graph fixture, pause before the primary commit, run both lookups.
- Yellow: with a replica `stale`, `@@` uses the primary and still returns the doc.
- Promotion: stop `surreal-search-0`. Flip shard 0’s in-sync replica (on node 1) to primary. `@@` for doc 0 still hits. The graph process was not restarted.
- Rebuild: delete one search volume, project that node’s shards from the graph (or from the surviving copy), `@@` hits again.

#### Do not

- Hash the shard with the live node count.
- Leave two primaries for one shard after promotion.
- Serve the browser.

#### Test scenarios

| Name | Pass |
|---|---|
| Merged words | A query returns both fixture headings. :8000 has no `SEARCH` index. |
| Exact | `norm` on :8000 finds the UUID while both search nodes are stopped. |
| Lag | Between the graph commit and the primary commit, `norm` hits and `@@` does not see the new body. |
| Promote | After `surreal-search-0` (:8001) is stopped, shard 0’s replica on :8002 becomes primary and `@@` returns doc 0. |

### 6. step-enqueue

**Gate:** M4 closed.

#### Work

- Table `graph_jobs`: `workspace_id` primary key, `wiki_sha`, dirty doc ids, `owner`, `lease_until`, `attempts`, `state` (`pending` | `failed`), `last_error`. Same migration path as `jobs`. The hub does not insert or claim it.
- Sidecar, after git and `last_flushed` have committed: upsert `graph_jobs`. Coalesce a pending row (newer SHA, union of doc ids). A failed upsert is logged and does not fail the flush.
- `venus-graph` claims with `SKIP LOCKED`. It does not read flush `jobs`. Claiming a row for the fixture workspace runs the step 4 projector. Parsing markdown is still out of scope: a claim with no fixture is a successful no-op that records the SHA, until the next board fills headings from the sidecar.

#### Do not

- Insert a graph reason into `jobs`.
- Have the flush worker claim `graph_jobs`.
- Hold the pin cut for the insert.
- Start tree-sitter or the glossary scan in this step.

#### Test scenarios

| Name | Pass |
|---|---|
| Row after flush | Flush commits. One `graph_jobs` row carries that `wiki_sha`. `last_flushed` matches. |
| Cluster down | Stop `surreal-graph`, `surreal-search-0`, and `surreal-search-1`. Flush still commits. `graph_jobs` still upserts. |
| Flush queue intact | No new `jobs.reason`. The next flush still claims `jobs`. |
| One claim | Two graph workers: one owner for that `workspace_id`. |

## After this board

Next board: read committed markdown + sidecar into the graph process (headings, `links_to`, glossary, tree-sitter, regex), then project each doc onto its shard with the writer this board already has. NER and the semantic model stay behind that. More search nodes, or a higher `shard_count`, stay a rebuild-and-allocate move ([scale](./scale.md#search-cluster)).
