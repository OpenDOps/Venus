# Semantic graph

**Status:** design. Store and extractors for the **spatial** wiki graph ([LifeIndexing](../Agents/LifeIndexing.md) AB1). The boards below are built by two milestones: [M5](../M5/README.md) builds the search cluster, and [M6](../M6/README.md) builds the rest of the graph on it ([M6 plan](../M6/plan.md)). Needs [M4](../M4/README.md): catalog `docId`, `gitPath`, `<!-- venus:doc:… -->`, and `.venus/links.json`. Does **not** replace [M7 — lease](../venus-implementation-plan.md#m7--lease--freeze-week). Does **not** delay `last_flushed`.

Contract of *what* is linked (heading binds, edge types, clocks, pack): [LifeIndexing](../Agents/LifeIndexing.md). This folder is *how* that graph is built and where it is stored.

| File | Role |
|---|---|
| [README.md](./README.md) | Place in the spine, boards, tool decision, pipeline |
| [extract.md](./extract.md) | In-document graph: tree-sitter, Aho–Corasick, regex, optional ONNX NER |
| [connect.md](./connect.md) | Direct references between pages, then background semantic binds |
| [store.md](./store.md) | SurrealDB graph schema, clocks, Compose |
| [scale.md](./scale.md) | Graph store vs search projection. **Surrealastic** replicates each database and places integer keys across shards. Venus writes once per job. |

## Boards

Each board has its own design (`README.md`) and step-by-step plan (`plan.md`). The number is the board’s id and does not change when milestones move. Each milestone folder holds the order its boards run in.

| # | Board | Milestone | Lands |
| --- | --- | --- | --- |
| 1 | [search-scale](./search-scale/README.md) ([plan](./search-scale/plan.md)) | [M5](../M5/README.md) | `surreal-search` cluster on surrealastic: shards, copies, HA default (3 nodes, `replica_count = 1`), router, recovery, backups, split. `graph_jobs` after `last_flushed`. |
| 2 | [extraction](./extraction/README.md) ([plan](./extraction/plan.md)) | [M6](../M6/plan.md) | Pages, headings, mentions, and `links_to` inside a page and across pages. |
| 3 | [graph-view](./graph-view/README.md) ([plan](./graph-view/plan.md)) | [M6](../M6/plan.md) | Sigma.js WebGL view of that graph. |
| 4 | [tantivy](./tantivy/README.md) ([plan](./tantivy/plan.md)) | Optional, no milestone yet | Tantivy replaces SurrealDB `SEARCH` on the search shards. |
| 5 | [vectors](./vectors/README.md) ([plan](./vectors/plan.md)) | [M6](../M6/plan.md) | One BGE-M3 vector per heading by the grain rule, HNSW on the search shards, bound through the existing graph. |
| 6 | [semantic](./semantic/README.md) ([plan](./semantic/plan.md)) | [M6](../M6/plan.md) | LLM edges with a required quote. |

## What this is

One hidden graph of the **published wiki** at a git SHA:

| Layer | Built by | Connects |
|---|---|---|
| **In document** | Deterministic Rust extractors. No LLM. | Headings, code symbols, glossary terms, emails, UUIDs, API paths, and (only if enabled) named entities **inside one page** |
| **Direct** | Same pass. No LLM. | Pages and headings that **cite** each other (`venus:doc:`, markdown links, catalog containment) |
| **Semantic** | Background model, after the two layers above are written | Headings on **different** pages (and the rare in-page logical bind) with required evidence: `defines`, `depends-on`, `constrains`, `contradicts`, `supersedes` |

The graph is not the spec. Git markdown + sidecar remains the share format. Postgres remains the live CRDT store. The graph is a photograph of **committed** `wiki/*.md` + `.venus/ids/`.

Temporal why (comment-commits, `decided-in`) stays [AB4](../Agents/agentic-binding.md#ab4--history--why-pack) and is **not** this folder. Product-repo call graphs stay [AB5](../Agents/code-bind.md) (CodeGraph CLI / Aider). This indexer does not read the product remote.

## Invariant

Live collaboration never waits on extract, SurrealDB, or the semantic model. Same rule as pin convert ([LiveSnapshot](../LiveSnapshot/README.md#invariant)).

```text
pin cut → fromDoc + sidecar → wiki/*.md → git commit → last_flushed
                                                         │
                                                         ▼  enqueue (SHA + dirty docIds)
                                              venus-graph (async)
                                                1. extract + direct edges  → SurrealDB
                                                2. semantic job            → SurrealDB
```

Failure of either step leaves git HEAD valid. A poison page fails that `docId` only.

## Tool decision

In-document work must stay autonomous and fast. Do not call an LLM to find a function name, a glossary word, or a UUID.

| Job | Choice | Rejected for this job |
|---|---|---|
| **Graph store** | **SurrealDB 2**, namespace `graph`. `RELATE` edges and exact mention keys. Rust SDK. | Postgres `crdt_*` (live Yjs, not a graph). A second graph engine. Embedding cosine stored as a bind. |
| **Lexical search** | `surreal-search` cluster: a projection of heading and mention text, `SEARCH` only on those nodes, sharded and copied by surrealastic ([scale](./scale.md), [search-scale — cluster](./search-scale/README.md#cluster)). Planned replacement of that index: [Compared with Elasticsearch](#compared-with-elasticsearch). | Elasticsearch as a second engine. SurrealDB Enterprise as the shard manager. Full-text indexes on the graph process. |
| **In-doc code names** | **tree-sitter** on fenced code only | CodeGraph CLI and Aider. Those analyze the **product** repo at `productSha` ([code-bind](../Agents/code-bind.md)). They are not the wiki indexer. |
| **Glossary / slang** | **`daachorse`** Aho–Corasick automaton compiled from `glossary.md` (plus page titles and unique headings) | An LLM pass over every paragraph. |
| **Emails, phones, UUIDs, hashes, API paths** | **`regex`** (Rust engine, linear time) | NER for patterns that are regular. |
| **Person / org / team** | Optional **ONNX** NER (`ort` + `tokenizers`), tiny model, **off** until a fixture shows the three tools above miss it | A Python NER service. A general LLM. |
| **Semantic page↔page binds** | Cheap JSON completion, separate model key, **after** extract ([board 6](./semantic/plan.md)) | BGE-M3 vectors as the edge. LanceDB as a second store. Cursor subscription as the completions API. |

**Vectors are [board 5](./vectors/plan.md).** Cosine on embeddings is candidate **search**, not a bind ([LifeIndexing — logical](../Agents/LifeIndexing.md#logical-llm--background)). Store one BGE-M3 (BAAI) vector per heading on the **search** namespace (HNSW). A query returns nearest headings, then one hop of the graph extraction already wrote binds those hits to word hits. The semantic job may use the same neighbors as candidates ([board 6](./semantic/plan.md)). Do not add LanceDB while that namespace can hold those vectors. Do not create a `related` edge from a neighbor in vector space. A fine-tuned DistilBERT can later tag a heading against a fixed theme list; that classification is future work on [extraction](./extraction/README.md), and it is not this index.

## Compared with Elasticsearch

Surrealastic takes Elasticsearch’s cluster and leaves Lucene. The process is Rust. The index on this board is SurrealDB `SEARCH` (BM25 on RocksDB), not Lucene. Rust removes the JVM. It does not, by itself, make a query faster than Lucene.

| | Elasticsearch | Surrealastic |
|---|---|---|
| Unit of scale | A shard of documents | A shard of integer keys. Venus passes `hkey`. |
| Copies | One extra copy by default. Two copies of the shard. | `replica_count = 1`. Two copies, distinct zones. |
| Who takes a write | One copy accepts the index operation. The others apply it after. | Every in-sync copy applies the same log entry. Search returns when one copy has committed. Copies stay equal. |
| Where a copy sits | Allocation, with a delay before a shard moves off a dead node. | Rendezvous. `REPL_REALLOCATE_DELAY_MS` (60 s) before a replacement copy. |
| Query | A coordinator asks every shard and merges hits. | The router asks every shard, hedges one slow copy, merges hits. |
| Score | BM25 inside one shard. Scores from two shards are not one scale. | The same. A new wiki has one shard, so the scale is the wiki. |
| When a hit is visible | After the next refresh, about 1 s by default. | In the transaction that committed the log entry. |
| Growing the index | Split, or build the index again, when the shard count must change. | Split by doubling. The layout replays current items onto the new shard sets. |
| Membership | Master-eligible nodes publish cluster state. | Postgres holds the lease and the copy map. A search does not read Postgres. |
| If every copy of a shard is gone | Restore a snapshot, or build the index again. | Replay `_layout_item` from the graph commit set. |

**Faster here.** No JVM warmup and no garbage-collection pause on the router. A committed entry is searchable immediately. Search ack waits for the fastest copy, and the copies are written in parallel. A wiki that still has one shard pays no scatter across shards. Each search node has an explicit cap (1 GB, 64 MB block cache) instead of a JVM heap plus the page cache.

**Still decided by the index.** Elasticsearch batches documents into immutable Lucene segments, then walks postings at query time. Surrealastic applies a guarded transaction of at most 256 docs on every in-sync copy. That costs more per document and buys a fresher, recoverable index. On a large corpus a Lucene query is still the faster query. This board uses SurrealDB BM25 because the same engine holds the graph and a shard is an ordinary SurrealDB database that the layout can refill.

**Search backend replacement.** The search-node index is planned to be replaced. The layout does not read the body. It stores an item key, a tag, and statements, and it replicates those. A later shard applies the same item to a Tantivy index instead of SurrealDB `SEARCH`. The graph process stays SurrealDB. The commit set, the copies, the router, and the split stay. That is the upgrade that can pass Elasticsearch on the query. [search-scale](./search-scale/plan.md) keeps SurrealDB `SEARCH` until that swap. Step-by-step: [tantivy/plan.md](./tantivy/plan.md).

The current `SEARCH` indexes are whole-term BM25 after the `wiki` analyzer (`class` tokenizer, lowercase, ascii). Prefix search and fuzzy / edit-distance search are unsupported. Both arrive with Tantivy.

## Pipeline

Worker: new crate **`crates/venus-graph`**. Compose service **`graph`** (profile, same idea as `sidecar`). It reads `wiki/` at the committed SHA. It does not import the hub, does not `fromDoc`, and does not hold the pin.

Queue grain stays the wiki, on Postgres `graph_jobs` (not the flush `jobs` row), inserted **after** `last_flushed` commits: `{ wikiSha, dirtyDocIds }`. Claim with `SKIP LOCKED`. One writer per workspace. Postgres holds the **job**. SurrealDB holds the **graph** and the **search** projection.

```text
1. Load     committed markdown + .venus/ids/<docId>.json + catalog path
            at wikiSha. Not the live Store. Not the pane splice.
2. Extract  structure, then glossary, tree-sitter, regex, optional NER
            ([extract.md](./extract.md))
3. Direct   links-to + catalog contains ([connect.md](./connect.md))
4. Upsert   graph namespace. Replace this doc’s extractor edges only.
4b. One `layout.write` for that doc ([scale.md](./scale.md)).
            A shard that lags stays queued in the layout. It does not roll back step 4.
5. Semantic enqueue dirty headings whose body hash changed
6. Model    compact map + those headings → evidence edges on the graph
            Replace source = 'model' outbound only.
            Then project gist text onto the search heading.
```

Skip step 6 when the heading sidecar-slice hash is unchanged. A dirty **page** is not dirty **meaning**.

Glossary file changed: rescan **mentions** on every page (CPU, no model). Run the model only for headings whose mention set changed, plus the glossary page itself.

## Accept bar

- After idle/flush, dirty pages land in SurrealDB without moving `last_flushed`.
- A page’s headings, fenced symbols, glossary hits, and `venus:doc:` targets are queryable. Unresolved hrefs are stored as unresolved, not guessed.
- Semantic edges, when the model job runs, carry a quote that is a substring of the source heading. Edges without that quote are dropped.
- Re-index of an unchanged heading does not churn edges.
- Snapshot git message is still `snapshot: <title>`. No graph text in the `.md` body.
- Killing SurrealDB or the model does not fail the flush.

## Do not

- Put the indexer in the hub process or on the pin cut.
- Teach the indexer Yjs.
- Store markdown bodies as the spec copy (a heading **slice** may be stored so evidence can be checked; git is still the file).
- Treat phone numbers and UUIDs as semantic binds.
- Merge two pages because they embed near each other.
- Run CodeGraph or Aider over `wiki/`.
- Publish the graph as a wiki page. The view is [board 3](./graph-view/plan.md). These boards do not ship chat.
- Shard documents or place copies with `hash %` the node count ([scale.md — placement](./scale.md#placement)).
