# Search scale

**Status:** design. **Not started.** The `surreal-search` cluster. Split from the graph: [scale.md](../scale.md). First board: [plan.md](./plan.md).

Word search is a projection of heading and mention text. It is not the graph. Edges, `mention.norm`, and model binds stay on `surreal-graph`.

## What runs on a search node

The same **SurrealDB server binary** as the graph (`surrealdb/surrealdb:v2`, `surreal start rocksdb:…`). SurrealDB does not ship a search-only build. A search node does **not** load the graph.

| On a search node | Not on a search node |
|---|---|
| Namespace `search`, database = `workspace_id` | Namespace `graph` |
| `page`, `heading`, `mention` text fields | `RELATE`, `links_to`, terms, symbols |
| `SEARCH` indexes (`heading_text`, `mention_text`) | Model edges, evidence quotes |
| Its own RocksDB volume and memory cap | The wiki git tree, Yjs, Postgres `crdt_*` |

`RELATE` is unused here. The full server is the process. The schema is search-only. Do not point `GRAPH_URL` at a search node.

## Cluster

SurrealDB does not shard or replicate a RocksDB file. Venus places copies above it. Postgres holds the map. The graph worker is the only writer and the query coordinator.

A **shard** is a slice of `doc_id`s. A **replica** is another copy of that same shard. Sibling shards are not backups of each other.

```text
venus-graph
    │  write primary of shard(doc_id), then each replica
    ▼
surreal-search-0              surreal-search-1
  primary shard 0               primary shard 1
  replica shard 1               replica shard 0
```

Dev shape, and [the first board](./plan.md): `shard_count = 2`, `replica_count = 1`, two search nodes. A node never holds a shard’s primary and that shard’s replica.

| Piece | Rule |
|---|---|
| **Shard key** | `shard = sha256(doc_id) % shard_count`. The modulus is `shard_count`, not the number of live nodes. |
| **Primary** | The only copy that takes a new projection for that shard. |
| **Replica** | The same search transaction, after the primary commits. Read-only except to the worker. |
| **Coordinator** | The worker. `@@` hits one in-sync copy of every shard (replica if `in_sync`, else primary). Merge by `search::score`. |
| **Adding a node** | Place a replica or move one shard. Do not rehash existing `doc_id`s. |
| **Changing `shard_count`** | Rebuild every shard from the graph under the new modulus, then flip the map. |

```text
search_cluster
  workspace_id     primary key
  shard_count      int
  replica_count    int

search_node
  node_id          primary key
  url

search_allocation
  workspace_id
  shard            int
  role             primary | replica
  node_id
  state            in_sync | stale | down
  synced_sha       text
  primary key (workspace_id, shard, role, node_id)
```

One primary per `(workspace_id, shard)`. Up to `replica_count` replicas, each on a different node from the primary and from each other.

| Health | Meaning |
|---|---|
| **Green** | Every primary is `in_sync`, and every configured replica matches that `synced_sha`. |
| **Yellow** | Every shard has a primary. A replica is `stale` or `down`. `@@` uses the primary. |
| **Red** | A shard has no primary and no `in_sync` replica. Those docs are absent from word search. The graph still answers links and `norm`. |

Promotion: primary is `down` and a replica of that shard is `in_sync`. Flip the replica’s role to `primary`. The old node returns as a `stale` replica and catches up from the new primary or from the graph. No Raft between SurrealDB processes. The allocation row is the cluster state.

## High availability

`replica_count` counts extra copies. `1` means two copies total (primary + replica). Elasticsearch’s default replica count is 1.

| Nodes | `replica_count` | One node dies | A new replica while it is still dead |
|---|---|---|---|
| 2 | 1 | Search stays up. The remaining copies sit on the survivor. | No. A second death rebuilds from the graph. **This board.** |
| **3** | **1** | Search stays up. The other copy is on a survivor. | **Yes.** The third node takes a new replica. **HA default.** |
| 3 | 2 | Two copies stay up. | Yes, and the index was stored three times. Not the default. |

Three nodes with `replica_count = 1` is the durable layout: lose one node, then place a replacement replica on the node that has room, without raising the replica count. `replica_count = 2` is for surviving two search nodes down at once before that replacement exists.

There is no separate trio of “master” nodes. Cluster state is the Postgres allocation table. The graph worker is the only process that flips it.

## Rebuild

A dropped copy is rebuilt from another copy of **that shard**. Shard 1 cannot recreate shard 0.

| Lost | Restore from | Then |
|---|---|---|
| One replica | Its primary, or the graph rows in that shard | Replica `in_sync`. Green. |
| One primary, replica `in_sync` | Promote the replica | That node is the primary. |
| One primary, no in-sync replica | Graph rows whose `shard` matches | New primary. No git read. |
| All search nodes | The graph, every shard | `search_sha` catches up. |

Lose both copies of a shard and the graph still has the rows. Elasticsearch cannot do that step unless a snapshot exists. Do not rebuild a shard from markdown while the graph for that SHA exists. Two primaries for one shard are a bug in the allocation table.

## Schema

Ids match the graph (`heading:[docId, blockId]`, `mention:[docId, blockId, kind, start]`).

```surql
DEFINE NAMESPACE search;
DEFINE DATABASE ⟨77e4a2b1-8b40-5979-a73c-fd4477216d00⟩;

DEFINE TABLE page SCHEMAFULL;
DEFINE FIELD git_path    ON page TYPE string;
DEFINE FIELD title       ON page TYPE string;
DEFINE FIELD indexed_sha ON page TYPE string;

DEFINE TABLE heading SCHEMAFULL;
DEFINE FIELD doc_id      ON heading TYPE string;
DEFINE FIELD block_id    ON heading TYPE string;
DEFINE FIELD level       ON heading TYPE int;
DEFINE FIELD text        ON heading TYPE string;
DEFINE FIELD body        ON heading TYPE string;
DEFINE FIELD gist        ON heading TYPE option<string>;
DEFINE FIELD indexed_sha ON heading TYPE string;

DEFINE TABLE mention SCHEMAFULL;
DEFINE FIELD doc_id      ON mention TYPE string;
DEFINE FIELD block_id    ON mention TYPE string;
DEFINE FIELD kind        ON mention TYPE string;
DEFINE FIELD text        ON mention TYPE string;
DEFINE FIELD norm        ON mention TYPE string;
DEFINE FIELD indexed_sha ON mention TYPE string;

DEFINE ANALYZER wiki TOKENIZERS class FILTERS lowercase, ascii;
DEFINE INDEX heading_text ON heading FIELDS text, body SEARCH ANALYZER wiki BM25;
DEFINE INDEX mention_text ON mention FIELDS text, norm SEARCH ANALYZER wiki BM25;
```

SurrealDB 3 spells `SEARCH` as `FULLTEXT`. `HIGHLIGHTS` stays off until a person sees snippets. No HNSW. No `RELATE`.

Replace this `docId`’s rows on the primary in one transaction, then apply that transaction to each replica. Stamp `page.search_sha` on the **graph** only after the primary commits. A failed replica leaves the stamp, marks the replica `stale`, and the cluster goes yellow.

BM25 score is per shard. A term in half or more of the headings **in that shard** scores 0. More shards make that clamp easier to hit. Exact `norm` stays on the graph.

## Memory

Each search node: cap 1 GB, `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` 64 MB, derived from **that** cap. An uncapped host makes RocksDB request about half of machine RAM minus 1 GB. A hot node’s cap can rise without touching the graph process.

## Do not

- Define namespace `graph` or a `SEARCH` index on `surreal-graph`.
- Ask SurrealDB to replicate RocksDB or to choose `shard_count`.
- Set `shard` with `hash %` the live node count.
- Run two primaries for one `(workspace_id, shard)`.
- Put a shard’s primary and its replica on the same node.
- Treat `replica_count = 2` as the default.
- Mix two workspaces in one search database.
- Block `last_flushed` on a search write.
