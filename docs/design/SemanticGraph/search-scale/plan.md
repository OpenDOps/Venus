# Search cluster


|               |                                                                                     |
| ------------- | ----------------------------------------------------------------------------------- |
| **planId**    | `ss-cluster`                                                                        |
| **Milestone** | First board of [SemanticGraph](../README.md). Search design: [README](./README.md). |
| **Duration**  | About 1 week, plus the third node after the two-node cluster is green               |
| **Board**     | [SS.state.yaml](./SS.state.yaml)                                                    |


Graph role and write order: [scale.md](../scale.md). Graph schema: [store.md](../store.md). Flush queue this must not join: [LiveSnapshot HA](../../LiveSnapshot/high-availability.md).

This board starts the search cluster. Each search node is a full SurrealDB server with the **search schema only**. The graph is a separate process. Every copy of a shard is equal: writes are fenced and carry a `seq`, and placement is rendezvous hashing ([scale.md](../scale.md)). HA default is three search nodes and `replica_count = 1` ([README — high availability](./README.md#high-availability)). Steps 1–5 ship two nodes so a copy can be lost while writes and reads continue. Step 7 adds the third node so a replacement copy can land while the dead node is still gone.

## Shape

```text
surreal-graph :8000             no SEARCH index; search_seq, hkey, fence
surreal-search-0 :8001          copy of shard 0, copy of shard 1
surreal-search-1 :8002          copy of shard 0, copy of shard 1
```

`shard = hkey % shard_count`, `hkey` = first 8 bytes of `sha256(doc_id)`, shifted right by 1 ([scale.md](../scale.md#shards)). Two copies on two nodes puts both shards on both nodes. Step 7 adds `surreal-search-2` (`:8003`) without changing `shard_count` or `replica_count`.

## Gate


| Steps      | When                                                            |
| ---------- | --------------------------------------------------------------- |
| **1–5, 7** | Now. They do not change Flush.                                  |
| **6**      | [M4](../../M4/README.md) **closed**. Hook after `last_flushed`. |




## Exit

1. Profile `graph` starts `surreal-graph` (`127.0.0.1:8000`), `surreal-search-0` (`:8001`), and `surreal-search-1` (`:8002`). Three volumes. Search nodes have no graph namespace.
2. Allocation: `shard_count = 2`, `replica_count = 1`, two copies per shard, one row per copy, no role column. Leases carry a `fence`.
3. Two fixture docs, one per shard: graph transaction assigns `seq`, one batch reaches both copies in parallel, then one stamp of `page.search_sha`. A stale fence and an older `seq` are refused.
4. `@@` merges both shards and does not run on :8000. Stopping either node leaves writes and reads working with no failover step. The graph being down still returns hits, `unverified`.
5. A lagging or wiped copy catches up from the graph by `applied_seq`. The other shard and the other search node are not sources.
6. After M4 is closed, flush upserts `graph_jobs` and still commits if every SurrealDB process is down.
7. A third search node takes a new copy after one node is stopped past the delay, `replica_count` still 1, `shard_count` still 2.
8. No LLM, no tree-sitter, no glossary scan, no graph UI. M1–M4 tests stay green.



## Non-goals


| Later                                               | Why not here                                                               |
| --------------------------------------------------- | -------------------------------------------------------------------------- |
| `replica_count = 2`                                 | Not the HA default.                                                        |
| Raising `shard_count`                               | Split by doubling, later. [scale.md — shards](../scale.md#shards).         |
| Hedged reads, `min_seq`, weighted nodes             | Designed in [scale.md](../scale.md#read-path). Not needed for two fixtures. |
| Headings from sidecar, glossary, tree-sitter, regex | Next board. Fixtures only.                                                 |
| Semantic model, NER, HNSW                           | Later.                                                                     |
| SurrealDB Enterprise, Raft between search nodes     | Postgres is the cluster state. The graph is the durable copy.              |




## Constraints

1. Search nodes run `surrealdb/surrealdb:v2.7.0`. Schema is namespace `search` only.
2. Order: graph commit (assigns `seq`), one batch to every `in_sync` copy in parallel, stamp `search_sha` after one copy commits. A failed copy is `lagging` and the cluster is yellow.
3. One fenced writer per wiki. Copies of one shard are in distinct zones. No leader among copies.
4. Catch up or rebuild a copy from the graph by `applied_seq`. Not from a sibling shard, not from another search node, not from markdown while the graph exists.
5. Block cache is set from each container’s cap (1 GB cap, 64 MB cache).
6. Hub and browser do not open these ports.
7. `graph_jobs` is not flush `jobs`.



## Steps summary

What each step **adds** to the product (not how to test it — that is under each step). Mark **status** on the step when the [board](./SS.state.yaml) moves.


| #                      | id                                        | Proves                                                                                  |
| ---------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------- |
| [1](#1-step-recon)     | [`step-recon`](#1-step-recon)             | ✅ **done.** Full binary, search schema only. P=2, R=1. HA target is 3 nodes, R=1.      |
| [2](#2-step-compose)   | [`step-compose`](#2-step-compose)         | ✅ **done.** Graph + two search nodes, three volumes.                                    |
| [3](#3-step-schema)    | [`step-schema`](#3-step-schema)           | ✅ **done.** Search indexes on :8001 and :8002. Allocation rows.                         |
| [4](#4-step-project)   | [`step-project`](#4-step-project)         | **pending.** Fenced, versioned batch to every copy at once. Copy rows, leases.          |
| [5](#5-step-query)     | [`step-query`](#5-step-query)             | **pending.** Merged `@@` with failover, monitor, catch-up from the graph.               |
| [6](#6-step-enqueue)   | [`step-enqueue`](#6-step-enqueue)         | **pending.** `graph_jobs` after flush. M4 closed.                                        |
| [7](#7-step-third-node) | [`step-third-node`](#7-step-third-node)  | **pending.** Third node joins by rendezvous and takes a copy after one node dies. R stays 1. |




## Steps

[Steps summary](#steps-summary). Do them in order. Step 6 waits until [M4](../../M4/README.md) is closed. Step 7 does not wait for step 6. A step is not started until its `dependsOn` steps are done. Test scenarios under each step are the accept rules. Encode them as tests where the How line names a command; do not invent extra scenarios.

<a id="1-step-recon"></a>

### 1. step-recon

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-project) · [5](#5-step-query) · [6](#6-step-enqueue) · [7](#7-step-third-node)


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
- Record flush `jobs` (`workspace_id` primary key, `reason` `idle` | `flush` | `lease`). `graph_jobs` is step 6.



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
| Copies     | [scale.md](../scale.md), the README cluster, and steps 4–7 describe equal copies: rendezvous placement, `fence`, `seq`, `applied_seq` catch-up from the graph. None of them names a leader copy or a promotion. |


- **How:** `cargo test -p venus-graph --test recon`. Fail if the client enables `kv-rocksdb`, if exit 7 sets `replica_count = 2`, if flush `jobs` is not keyed by `workspace_id`, or if the design or steps 4–7 bring back a leader copy.




<a id="2-step-compose"></a>

### 2. step-compose

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · **2** · [3](#3-step-schema) · [4](#4-step-project) · [5](#5-step-query) · [6](#6-step-enqueue) · [7](#7-step-third-node)


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


| Name             | Pass                                                                    |
| ---------------- | ----------------------------------------------------------------------- |
| Profile off      | `docker compose up` without `--profile graph` starts none of the three. |
| Three processes  | With the profile, :8000, :8001, and :8002 are healthy, 1g each.         |
| Independent disk | A row on :8001 survives a recreate of :8002.                            |


- **How:** `cargo test -p venus-graph --test compose`. Fail if default `docker compose config --services` lists a surreal service, if a host port is `0.0.0.0`, or if :8001 and :8002 share a volume.




<a id="3-step-schema"></a>

### 3. step-schema

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · **3** · [4](#4-step-project) · [5](#5-step-query) · [6](#6-step-enqueue) · [7](#7-step-third-node)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 3                                                       |
| **id**        | `step-schema`                                           |
| **title**     | Graph schema, search schema, allocation rows            |
| **dependsOn** | `step-compose`                                          |
| **kind**      | implement                                               |
| **status**    | **done** ([board](./SS.state.yaml); breakpoint `human`) |


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


| Name                | Pass                                                                      |
| ------------------- | ------------------------------------------------------------------------- |
| Graph has no index  | :8000 has `links_to` and no full-text index.                              |
| Search has no graph | :8001 and :8002 have `heading_text` and `mention_text` and no `links_to`. |
| Allocation          | The four rows above. A second migrate does not add a second primary.      |


- **How:** `cargo test -p venus-graph --test schema`. Fail if :8000 has a `SEARCH` index, if a search node has `links_to` or namespace `graph`, or if a second migrate adds a primary.




<a id="4-step-project"></a>

### 4. step-project

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · **4** · [5](#5-step-query) · [6](#6-step-enqueue) · [7](#7-step-third-node)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 4                                                       |
| **id**        | `step-project`                                          |
| **title**     | Fenced, versioned batch to every copy at once           |
| **dependsOn** | `step-schema`                                           |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Migrate the Postgres map to [copies](../scale.md#postgres-tables), idempotent, in `crates/venus-graph`: add `graph_lease`; `search_cluster` gains `epoch` and `shard_count_next`; `search_node` gains `zone` (dev: equal to `node_id`), `weight`, `state`; `search_allocation` drops the step-3 role column and its one-per-shard unique index, gains `epoch` and `applied_seq`, and `state` becomes `joining` | `in_sync` | `lagging`. Seed two copies per shard, one on each node. Step 3’s `allocation` test moves to this shape.
- Graph schema ([store.md](../store.md#schema)) gains `graph_meta.fence`, `graph_meta.search_seq`, `page.search_seq`, `page.hkey`, `page.deleted`, and a plain index on `page.search_seq`. Still no `SEARCH` index.
- Search schema ([README](./README.md#schema)) moves to one database per shard copy, `⟨{workspace_id}_0_{shard}⟩`, with `meta:shard`, `page.seq`, and `page.deleted`. Migrate creates both shard databases on both nodes.
- Placement: a pure rendezvous function over `(workspace_id, epoch, shard, node_id)` with the zone rule ([scale.md — placement](../scale.md#placement)).
- Writer claims lease `writer:{workspace_id}` and holds its `fence`.
- Two fixture docs on `surreal-graph`, chosen so `hkey % 2` differs. Each: one heading with `body`, one `mention` kind `uuid`. No file read.
- One graph transaction checks the fence and assigns `seq` and `hkey`. One batch per shard goes to both copies in parallel over the WebSocket client. Inside each batch: fence check, `seq` guard, delete by id range on `docId`, upsert, `applied_seq`.
- Stamp `search_sha` after one copy of that shard commits. A failed copy becomes `lagging`. No copy committed: `search_sha` unset, `search_error` set.



#### Do not

- Parse `wiki/`.
- Stamp before a copy of that shard commits.
- Roll back graph rows because a copy failed.
- Send a live batch to a `lagging` copy.
- Delete a document’s rows with a field scan.



#### Test scenarios


| Name            | Pass                                                                                                                         |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Placement       | For N = 2…10 and `replica_count` = 0…2: homes are distinct zones and stable across runs. Adding a node moves only the shards whose homes now include it. |
| Both copies     | Doc 0 and doc 1 are on :8001 and :8002, each in its shard database. Every copy `in_sync`. `search_sha` set. Green.          |
| One copy down   | Stop :8002, then project. :8001 has both docs. `search_sha` set. The copies on :8002 are `lagging`. Yellow.                  |
| All copies down | Stop both search nodes. Graph heading remains. `search_sha` empty. `search_error` set.                                       |
| Fenced          | A second claimant takes the lease. A batch with the old fence is refused by both copies and by the graph.                    |
| Older seq       | Replaying doc 0 with an older `seq` leaves the newer body on both copies.                                                    |




<a id="5-step-query"></a>

### 5. step-query

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-project) · **5** · [6](#6-step-enqueue) · [7](#7-step-third-node)


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 5                                                       |
| **id**        | `step-query`                                            |
| **title**     | Merged search with failover, monitor, catch-up          |
| **dependsOn** | `step-project`                                          |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Router ([scale.md — read path](../scale.md#read-path)): map cached from Postgres, reloaded on `NOTIFY search_map`. Per shard, one `in_sync` copy (two random choices, fewer requests in flight). Fail over to the next copy in the same request. Merge by score, remove duplicate ids. Hydrate from the graph within 100 ms; mark hits `unverified` when the graph does not answer. Do not run `@@` on :8000.
- `mention.norm` on the graph only.
- Monitor lease `monitor`: probe each node every second. `suspect` after 3 misses, `down` after 10 s, `up` on the first good probe. Writers mark copies on a `down` node `lagging`.
- Catch-up ([scale.md](../scale.md#catch-up-join-restore)): read `applied_seq` from the copy’s own `meta:shard`, replay graph pages with `search_seq` above it and `hkey % 2` equal to that shard, in `seq` order, on that shard’s queue. Then `in_sync`.
- Wiped volume: recreate the shard databases and schema, catch up from 0.



#### Do not

- Read from a `lagging` or `joining` copy.
- Catch up from a sibling shard or from another search node.
- Move a copy to another node (step 7).
- Serve the browser.



#### Test scenarios


| Name         | Pass                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| Merged words | The query returns both fixture headings. :8000 has no `SEARCH` index.                                                           |
| Exact        | `norm` on :8000 finds the UUID while both search nodes are stopped.                                                             |
| Failover     | Stop :8001. `@@` returns both docs from :8002 in the same request. No copy state is changed to make that happen.                |
| Graph down   | Stop :8000. `@@` returns both headings, marked `unverified`.                                                                    |
| Monitor      | Stop :8002. It is `suspect` after about 3 s and `down` after 10 s, and its copies are `lagging`. Start it: `up`.                |
| Catch-up     | Stop :8002, project a new body for doc 0, start :8002. Only pages above its `applied_seq` are replayed. Copy `in_sync`. Green. |
| Wiped        | Remove :8002’s volume. Its copies return from the graph with the same rows as :8001. Shard 0’s catch-up reads only `hkey % 2 = 0` pages. |




<a id="6-step-enqueue"></a>

### 6. step-enqueue

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-project) · [5](#5-step-query) · **6** · [7](#7-step-third-node)


|               |                                                              |
| ------------- | ------------------------------------------------------------ |
| **n**         | 6                                                            |
| **id**        | `step-enqueue`                                               |
| **title**     | Enqueue `graph_jobs` after flush                             |
| **dependsOn** | `step-query`; [M4](../../M4/README.md) closed                |
| **kind**      | implement                                                    |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`)   |


**Gate:** M4 closed.

#### Work

- Table `graph_jobs`: `workspace_id` primary key, `wiki_sha`, dirty doc ids, claim fields, `state` `pending` | `failed`. Hub does not insert or claim it.
- Sidecar, after git and `last_flushed` have committed: upsert `graph_jobs`. Coalesce a pending row. A failed upsert does not fail the flush.
- `venus-graph` claims with `SKIP LOCKED` while it holds lease `writer:{workspace_id}`, and runs the projector with that `fence`. No markdown parse yet: a claim with no fixture records the SHA and exits.



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




<a id="7-step-third-node"></a>

### 7. step-third-node

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon) · [2](#2-step-compose) · [3](#3-step-schema) · [4](#4-step-project) · [5](#5-step-query) · [6](#6-step-enqueue) · **7**


|               |                                                         |
| ------------- | ------------------------------------------------------- |
| **n**         | 7                                                       |
| **id**        | `step-third-node`                                       |
| **title**     | Third search node joins and takes a replacement copy    |
| **dependsOn** | `step-query`                                            |
| **kind**      | implement                                               |
| **status**    | **pending** ([board](./SS.state.yaml); breakpoint `human`) |


#### Work

- Service `surreal-search-2`: `127.0.0.1:8003`, its own volume, same image, cap, and search schema. Zone `2`. `shard_count` and `replica_count` stay 2 and 1.
- Join ([scale.md](../scale.md#catch-up-join-restore)): insert `search_node` `2`. Recompute rendezvous homes. Where node 2 is now a home, a `joining` copy catches up from the graph from 0, becomes `in_sync`, and only then the copy that dropped out is removed and its database dropped.
- Stop `surreal-search-0`. Inside `SEARCH_REALLOCATE_DELAY_MS` nothing moves. After it, each shard with a copy on node 0 gets a `joining` copy on the next ranked live node, filled from the graph.
- Health returns to green without starting node 0. Start node 0: its copies catch up, then the replacements are removed.



#### Do not

- Change `shard_count` or rehash `doc_id`s.
- Set `replica_count` to 2.
- Put two copies of one shard on one node.
- Copy a shard from another search node.
- Remove a copy before its successor is `in_sync`.



#### Test scenarios


| Name             | Pass                                                                                                                       |
| ---------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Third node       | :8003 is healthy and has the search schema. `search_cluster.replica_count` is still 1.                                     |
| Join moves few   | The copies that moved are exactly the rendezvous difference between two and three nodes. No other copy is rewritten.      |
| Restart in delay | Stop and start :8001 inside the delay. No new copy is placed. Its copies catch up.                                         |
| Re-replicate     | With :8001 stopped past the delay, each shard has two `in_sync` copies, on nodes 1 and 2. Green.                           |
| No rehash        | The same fixture `doc_id` still maps to the same shard number.                                                             |




## After this board

Next board: read committed markdown and the sidecar into the graph, then project each doc with this writer. NER and the semantic model stay behind that.