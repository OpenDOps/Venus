# Search scale

**Status:** step-schema done. Design moved to **equal copies** before step 4: every copy of a shard takes writes and reads, writes carry a fence and a `seq`, placement is rendezvous hashing ([scale.md](../scale.md)). Step 4 migrates the step-3 allocation rows to that shape. Profile `graph` starts the three processes. Image `surrealdb/surrealdb:v2.7.0`. First board: [plan.md](./plan.md).

Word search is a projection of heading and mention text. It is not the graph. Edges, `mention.norm`, and model binds stay on `surreal-graph`.

## What runs on a search node

The same **SurrealDB server binary** as the graph (`surrealdb/surrealdb:v2.7.0`, `surreal start rocksdb:…`). SurrealDB does not ship a search-only build. A search node does **not** load the graph.

| On a search node | Not on a search node |
|---|---|
| Namespace `search`, one database per shard copy: `⟨{workspace_id}_{epoch}_{shard}⟩` | Namespace `graph` |
| `page`, `heading`, `mention` text fields, `meta:shard` | `RELATE`, `links_to`, terms, symbols |
| `SEARCH` indexes (`heading_text`, `mention_text`) | Model edges, evidence quotes |
| Its own RocksDB volume and memory cap | The wiki git tree, Yjs, Postgres `crdt_*` |

`RELATE` is unused here. The full server is the process. The schema is search-only. Do not point `GRAPH_URL` at a search node.

## Cluster

SurrealDB does not shard or replicate a RocksDB file. Venus places copies above it. Postgres holds the map. Search nodes never talk to each other.

A **shard** is a slice of `doc_id`s. A **copy** is one database holding that slice on one node. All copies of a shard are equal. Sibling shards are not backups of each other.

```text
venus-graph (writer lease for this wiki)
    │  one batch per shard, to every in_sync copy at once
    ▼
surreal-search-0              surreal-search-1
  copy of shard 0               copy of shard 0
  copy of shard 1               copy of shard 1
```

Dev shape, and [the first board](./plan.md): `shard_count = 2`, `replica_count = 1`, two search nodes. Two copies per shard on two nodes puts both shards on both nodes.

| Piece | Rule |
|---|---|
| **Shard key** | `hkey % shard_count`, `hkey` = first 8 bytes of `sha256(doc_id)`, big-endian, shifted right by 1. Not the node count. [scale.md — shards](../scale.md#shards) |
| **Copies** | `1 + replica_count` per shard, in distinct zones. Each takes writes and reads. |
| **Placement** | Rendezvous hashing over `(workspace_id, epoch, shard, node_id)`. Exact for any N. [scale.md — placement](../scale.md#placement) |
| **Write** | Graph transaction assigns `seq`. The writer sends one batch per shard to every `in_sync` copy in parallel. One commit is enough. [scale.md — write path](../scale.md#write-path) |
| **Fence** | Every graph and search transaction rejects an older lease `fence`. A copy skips a doc whose stored `seq` is equal or newer. |
| **Read** | Any `venus-graph` process. One `in_sync` copy per shard, fewer requests in flight, hedge after p95, fail over in the same request. [scale.md — read path](../scale.md#read-path) |
| **Catch-up** | Replay graph pages with `search_seq > applied_seq` for that shard. Never from another search node. |
| **Adding a node** | Next `node_id`. Only shards whose rendezvous homes now include it move one copy. No `doc_id` changes shard. |
| **Changing `shard_count`** | Split by doubling, keeping half the rows in place. Any other change is a rebuild from the graph under a new `epoch`. |

Tables, exact columns, and defaults: [scale.md — Postgres tables](../scale.md#postgres-tables). In short: `graph_lease` (writer and monitor leases, `fence`), `search_cluster` (`epoch`, `shard_count`, `shard_count_next`, `replica_count`), `search_node` (`zone`, `weight`, `state`), `search_allocation` (one row per copy: `state joining | in_sync | lagging`, `applied_seq`). No role column.

| Health | Meaning |
|---|---|
| **Green** | Every shard has `1 + replica_count` `in_sync` copies on `up` nodes. |
| **Yellow** | Every shard has at least one `in_sync` copy. Some have fewer. Writes and reads continue. |
| **Red** | A shard has no `in_sync` copy. Those docs are absent from word search. The graph still answers links and `norm`. |

## High availability

`replica_count` counts extra copies. `1` means two copies total. Elasticsearch’s default replica count is 1.

| Nodes | `replica_count` | One node dies | A new copy while it is still dead |
|---|---|---|---|
| 2 | 1 | Search stays up. The other copy of every shard is on the survivor. No write gap. | No. A second death rebuilds from the graph. **This board.** |
| **3** | **1** | Search stays up. The other copy is on a survivor. | **Yes.** After the delay, the next node in that shard’s ranking takes a new copy. **HA default.** |
| 3 | 2 | Two copies stay up. | Yes, and the index was stored three times. Not the default. |

Three nodes with `replica_count = 1` is the durable layout: lose one node, then a replacement copy lands on the third node without raising the replica count. `replica_count = 2` is for surviving two search nodes down at once before that replacement exists.

There is no leader and no separate trio of “master” nodes. Cluster state is Postgres. A wiki’s writer changes its copy rows. The monitor changes node state.

## Rebuild

A lost copy is refilled from the **graph**, from that copy’s own `applied_seq`. Shard 1 cannot recreate shard 0, and a search node never copies from another.

| Lost | Restore | Then |
|---|---|---|
| One batch on one copy | Catch-up from its `applied_seq` | `in_sync`. Green. |
| One node, back inside the delay | Catch-up for each copy on it | Green. No copy moved. |
| One node, past the delay | New copy on the next ranked node, from `applied_seq = 0` | Green on the remaining nodes. |
| One volume wiped | Recreate the database and schema, catch up from 0 | Green. |
| Every copy of a shard | Catch up each home from 0 | Red until the first copy is `in_sync`. |
| All search nodes | The graph, every shard | `search_sha` catches up. |

Lose every copy of a shard and the graph still has the rows. Elasticsearch cannot do that step unless a snapshot exists. Do not rebuild a shard from markdown while the graph for that SHA exists.

## Schema

Ids match the graph (`page:⟨docId⟩`, `heading:[docId, blockId]`, `mention:[docId, blockId, kind, start]`). A document’s rows are deleted by an id range on `docId`, not by a field scan.

Each shard copy is one database with this schema. The name is `⟨{workspace_id}_{epoch}_{shard}⟩`.

```surql
DEFINE NAMESPACE search;
DEFINE DATABASE ⟨77e4a2b1-8b40-5979-a73c-fd4477216d00_0_0⟩;

DEFINE TABLE meta SCHEMAFULL;               -- one row, meta:shard
DEFINE FIELD fence       ON meta TYPE int;
DEFINE FIELD applied_seq ON meta TYPE int;

DEFINE TABLE page SCHEMAFULL;
DEFINE FIELD git_path    ON page TYPE string;
DEFINE FIELD title       ON page TYPE string;
DEFINE FIELD indexed_sha ON page TYPE string;
DEFINE FIELD seq         ON page TYPE int;
DEFINE FIELD deleted     ON page TYPE bool DEFAULT false;

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

A batch replaces each document’s rows in one transaction on every `in_sync` copy, after the fence and `seq` checks. Stamp `page.search_sha` on the **graph** after one copy of that shard commits. A copy that failed is `lagging`, and the cluster is yellow until catch-up.

BM25 score is per shard copy. A term in half or more of the headings **in that shard** scores 0. A new wiki has one shard, so that clamp and the score scale are wiki-wide. Exact `norm` stays on the graph.

## Memory

Each search node: cap 1 GB, `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE` 64 MB, derived from **that** cap. An uncapped host makes RocksDB request about half of machine RAM minus 1 GB. A hot node’s cap can rise without touching the graph process. A larger node gets a higher `weight` and takes more copies.

## Do not

- Define namespace `graph` or a `SEARCH` index on `surreal-graph`.
- Ask SurrealDB to replicate RocksDB or to choose `shard_count`.
- Set `shard` with `hash %` the node count, or place copies with `% N`.
- Write a search row without the fence and `seq` checks.
- Put two copies of one shard in one zone.
- Copy a shard from another search node, or from a sibling shard.
- Treat `replica_count = 2` as the default.
- Mix two workspaces in one search database.
- Block `last_flushed` on a search write.
