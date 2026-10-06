# Scale — graph store and search store

**Status:** design. Same engine as [store.md](./store.md): **SurrealDB**. Two roles. Word search is a **projection** of the graph, written after the graph commit. `surreal-graph` is one writer. `surreal-search` is a cluster of SurrealDB processes that hold **search rows only**: [search-scale](./search-scale/README.md). The first board is [search-scale/plan.md](./search-scale/plan.md).

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

Step 3 failing leaves the primary in place and the stamp allowed. That shard’s replica is `stale`. The cluster is **yellow**. Retry the replica apply. Do not roll back the primary or the graph. The stamp means every **primary** has the doc. A replica may trail. Copy placement, promotion, and the third node: [search-scale](./search-scale/README.md).

Invariant: every search row’s `indexed_sha` equals the graph `indexed_sha` it was copied from, and `page.search_sha` ≤ `page.indexed_sha`. A pack may say that word search lags, the same way AB2 says the index lags the live page.

Gist updates project again after the semantic pass (heading gist field only). Model edges stay on the graph. The semantic call does not read the search store.

Catalog delete of a `docId`: delete graph rows, then the search rows for that `docId` on its shard. A search hit whose graph heading is gone is dropped at hydrate time.

## Where search scale lives

Shard map, search schema, health, rebuild, memory caps, and the HA default (three search nodes, `replica_count = 1`) are [search-scale](./search-scale/README.md). Implementation is [search-scale/plan.md](./search-scale/plan.md).

Word search runs `@@` on one in-sync copy of each shard, then hydrates ids from this graph. Exact `norm`, links, and model binds stay here. Do not put a `SEARCH` index on the graph process. Do not block `last_flushed` on the search write.
