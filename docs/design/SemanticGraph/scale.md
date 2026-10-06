# Scale — graph store and search store

**Status:** design. Same engine as [store.md](./store.md): **SurrealDB**. Two roles. Word search is a **projection** of the graph, written after the graph commit. `surreal-graph` is one writer. `surreal-search` is a **cluster of SurrealDB processes**: fixed primary shards and replicas, placed by Venus ([search cluster](#search-cluster)). The first board ([plan](./plan.md)) starts that cluster. SurrealDB itself does not shard or replicate a RocksDB file.

Traversal, mentions, and model edges stay in the graph namespace ([store](./store.md)). A `@@` query hits the search namespace only, then hydrates ids from the graph.

## Why two stores of the same engine

A graph write and a search posting cannot share one transaction once they are different processes. Venus accepts that gap because git is the spec and the graph can be rebuilt. The gap is one way: search is behind the graph, or equal to it. Search never knows an edge the graph has dropped.

Exact lookups stay on the graph and do not wait for the projection: `norm` (UUID, email, endpoint), `RELATE`, unresolved `links_to`. Those are the pack. Word search is the leftover (a heading body, a rare word, later a person typing in a box). That leftover is what moves to the second store when readers or a large wiki make postings heavier than edges.

| Role | Holds | Does not hold |
|---|---|---|
| **Graph** | Pages, headings (including `body` for evidence), mentions, terms, symbols, all `RELATE` edges, model binds | A `SEARCH` / full-text index |
| **Search** | A copy of page title, heading `text` + `body` + gist, mention `text` + `norm`, and the full-text indexes | Edges, quotes, confidence, the semantic model |

Both use the same namespace and database names. A search **shard** is that database on the node which holds that shard. One RocksDB file is one process and holds only the shards allocated to it.

| | Graph | Search shard |
|---|---|---|
| Namespace | `graph` | `search` |
| Database | `workspace_id` | `workspace_id` |
| Process | One per wiki (`GRAPH_URL`) | One process per **node**. A node holds some primary shards and some replica shards. |

## Write order

The workspace claim ([store — one writer](./store.md)) is the only writer of **both** databases for that wiki. Search has no second ingester.

```text
1. Graph transaction     headings, mentions, edges for this pass
2. Primary shard         project that doc onto shard = hash(doc_id) % P
3. Replicas              apply that same search transaction to each replica of the shard
4. Graph stamp           page.search_sha = page.indexed_sha
                         clear page.search_error
```

Step 2 failing leaves the graph committed and `search_sha` unchanged. Set `page.search_error`. Retry **projection only** when `body_hash` still matches. Do not re-run the model because search lagged. Do not roll back the graph.

Step 3 failing leaves the primary in place and the stamp allowed. That shard’s replica is `stale`. The cluster is **yellow**. Retry the replica apply. Do not roll back the primary or the graph. The stamp means every **primary** has the doc. A replica may trail.

Step 3 failing is the same gap: graph ahead, stamp old, retry the stamp (and step 2 if the search rows are missing).

Invariant: every search row’s `indexed_sha` equals the graph `indexed_sha` it was copied from, and `page.search_sha` ≤ `page.indexed_sha`. A pack may say that word search lags, the same way AB2 says the index lags the live page.

Gist updates project again after the semantic pass (heading gist field only). Model edges stay on the graph. The semantic call does not read the search store.

Catalog delete of a `docId`: delete graph rows, then search rows for that `docId`. A search hit whose graph heading is gone is dropped at hydrate time.

## Search schema

Ids match the graph (`heading:[docId, blockId]`, `mention:[docId, blockId, kind, start]`) so a hit is a graph key.

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

SurrealDB 3 spells `SEARCH` as `FULLTEXT`. Same index, same place: the search namespace only.

`HIGHLIGHTS` stays off until a person sees snippets. No HNSW in this slice. No `RELATE`. Stemming and synonyms, when wanted, are analyzer filters here ([the word-search slice](./README.md#tool-decision)), still not a second engine.

Replace this `docId`’s search rows in one transaction on the **primary** after the graph pass, then insert the new set. Apply that same transaction to each replica. Stamp `search_sha` only after the primary commits. A crash mid-primary must not stamp success.

## Queries

| Question | Store |
|---|---|
| Neighborhood, inbound links, glossary definition, model binds | Graph `RELATE` ([store — queries](./store.md#queries)) |
| Where is this UUID / email / path | Graph `mention.norm` (exact). Works while search lags. |
| Which headings contain these words | `@@` on **one in-sync copy of each shard**, scores merged by the worker. Returns `doc_id` + `block_id`. |
| Pack that needs both | Those hits, then graph hydrate. Drop hits the graph no longer has. |

```surql
SELECT id, doc_id, block_id, search::score(1) AS score
FROM heading
WHERE text @1@ $q OR body @1@ $q
ORDER BY score DESC;
```

BM25 on a small wiki still gives a **zero score** to a term that appears in half or more of the indexed headings in **that shard**. More shards make the clamp easier to hit, because each shard sees a smaller corpus and inverse document frequency is local to the shard. Exact `norm` and links stay on the graph. Start `shard_count` at 2 only so the cluster is real; raise it when one shard’s postings no longer fit one node, and expect scores from different shards to be only roughly comparable.

## Search cluster

An Elasticsearch cluster shards the inverted index and keeps replica copies of each shard on other nodes. SurrealDB community keeps the index inside one RocksDB process and does not do that. Venus does it **above** SurrealDB: each cluster node is a normal `surreal start rocksdb:…` process. Postgres stores which node holds which shard. The graph worker is the only writer, and it is also the coordinator that scatters a word query.

```text
venus-graph  (one claim per wiki)
    │
    │  writes the primary of shard(doc_id), then each replica
    ▼
surreal-search-0              surreal-search-1
  primary shard 0               primary shard 1
  replica shard 1               replica shard 0
```

Dev cluster, and the first board: `shard_count = 2`, `replica_count = 1`, two search nodes. A node never holds a shard’s primary and that same shard’s replica.

| Piece | Rule |
|---|---|
| **Shard key** | `shard = sha256(doc_id) % shard_count`. The modulus is `shard_count`, not the number of live nodes. |
| **Primary** | The only copy that takes a new projection for that shard. |
| **Replica** | Same search transaction, applied after the primary commits. Read-only to everyone except the worker. |
| **Coordinator** | The worker. For `@@`, query one in-sync copy of every shard (a replica if it is `in_sync`, otherwise the primary). Merge by `search::score`. |
| **Adding a node** | Allocate a replica or move one shard onto it. Do not rehash existing `doc_id`s. |
| **Changing `shard_count`** | Rebuild every search shard from the graph under the new modulus, then flip the allocation. Not an online split of the RocksDB file. |

Allocation (Postgres, not inside SurrealDB):

```text
search_cluster
  workspace_id     primary key
  shard_count      int          -- P, fixed until an explicit rebuild
  replica_count    int          -- R

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

One primary row per `(workspace_id, shard)`. Up to `replica_count` replica rows, each on a different `node_id` from the primary and from each other.

| Health | Meaning |
|---|---|
| **Green** | Every shard has a primary `in_sync`, and every configured replica is `in_sync` at that primary’s `synced_sha`. |
| **Yellow** | Every shard has a primary. A replica is `stale` or `down`. Word search still runs on the primary. |
| **Red** | A shard has no primary and no `in_sync` replica. That shard’s docs are absent from `@@`. The graph still answers links and `norm`. |

Promotion: the primary node is `down` and a replica of that shard is `in_sync`. Flip that replica’s role to `primary`. The old primary, when it returns, comes back as a `stale` replica and catches up by re-applying from the graph (or from the new primary). No Raft between SurrealDB processes. The allocation row is the cluster state. One graph claim is the only process that flips it.

Cross-wiki search fans out per workspace, then per shard of that workspace, and only for workspaces on the caller’s allow-list. Shards of two wikis never share a SurrealDB database.

## Many wikis

The scale unit is the **workspace**, the same unit as a hub owner ([hub fleet](../../devops/hub-fleet.md)). More users on one wiki add hub sockets. They do not add graphs. The graph grows on flush.

| Load | Move |
|---|---|
| Another wiki | Its own graph process and its own `search_cluster` rows (`shard_count`, `replica_count`). New shards land on nodes that have room. |
| Heavier word search on one wiki | Raise the cap on the nodes that hold that wiki’s shards, add a search node and move a replica onto it, or raise `shard_count` by rebuilding from the graph. The graph process stays. |
| Heavier extract on one wiki | That wiki’s graph process. One claim, one writer. |
| A person who can see several wikis | Fan-out `@@` across each allowed wiki’s shards, merge scores, hydrate each hit on **that** wiki’s graph. |

Graph placement is one URL per wiki. Search placement is the allocation table above. Adding a search node does not remap `doc_id`s. Same rule as the hub fleet: do not use `hash %` the current number of nodes.

## Rebuild

A dropped **copy** is rebuilt from another copy of **that same shard**. Shard 0 and shard 1 hold different `doc_id`s. Shard 1 cannot recreate shard 0. That is the same rule as Elasticsearch: replicas are copies of one primary, and the other primaries are the rest of the index, not backups of each other.

Three copies of one shard (`replica_count = 2`) means any one of them can die and the remaining two rebuild it. This board runs `replica_count = 1`: two copies. Lose one, rebuild from the one that is left. Lose both, rebuild that shard from the graph. Elasticsearch cannot do that last step unless a snapshot exists, because the index is its source of truth. Here the graph is.

| Lost | Restore from | Then |
|---|---|---|
| One replica | Its primary, or the graph rows in that shard | Replica `in_sync`. Cluster returns to green. |
| One primary, replica `in_sync` | Promote the replica | That node is the primary. Old node returns as a stale replica. |
| One primary, no in-sync replica | Graph rows whose `shard` matches | New primary. No git read. |
| All search nodes | The graph, every shard | `search_sha` catches up. |
| Graph | Git SHA + sidecar (re-extract, [README pipeline](./README.md#pipeline)) | Project every shard again. |

Do not rebuild a shard from markdown while the graph for that SHA exists. One projector. Two primaries for the same shard are a bug in the allocation table, not a replication strategy.

## Memory

Each process has its own cap and its own `SURREAL_ROCKSDB_BLOCK_CACHE_SIZE`, derived from **that** cap. An uncapped host makes RocksDB request about half of machine RAM minus 1 GB. The graph process and every search node set the cap (start 1 GB, cache 64 MB). A hot shard’s node can be raised without raising the graph.

## Do not

- Put `SEARCH` indexes on the graph process.
- Ask SurrealDB to replicate RocksDB or to pick shard counts. The allocation table is the cluster.
- Set `shard` with `hash %` the live node count.
- Run two primaries for one `(workspace_id, shard)`.
- Put a shard’s primary and its replica on the same node.
- Store model edges in a search shard.
- Block `last_flushed`, or the semantic model, on the search transaction.
- Mix two workspaces in one search database.
