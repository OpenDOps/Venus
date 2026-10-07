# M5 — Semantic graph

**Status:** in progress — [search-scale](./search-scale/plan.md) steps 1–4 done (step 4, the replication core, 2026-10-07). Board [SS.state.yaml](./search-scale/SS.state.yaml). Store and extractors for the **spatial** wiki graph ([LifeIndexing](../Agents/LifeIndexing.md) AB1). **Gate:** [M4](../M4/README.md) closed — catalog `docId`, `gitPath`, `<!-- venus:doc:… -->`, and `.venus/links.json` exist. Does **not** replace [M7 — lease](../venus-implementation-plan.md#m7--lease--freeze-week). Does **not** delay `last_flushed`.

Contract of *what* is linked (heading binds, edge types, clocks, pack): [LifeIndexing](../Agents/LifeIndexing.md). This folder is *how* that graph is built and where it is stored.

| File | Role |
|---|---|
| [README.md](./README.md) | Place in the spine, tool decision, pipeline |
| [extract.md](./extract.md) | In-document graph: tree-sitter, Aho–Corasick, regex, optional ONNX NER |
| [connect.md](./connect.md) | Direct references between pages, then background semantic binds |
| [store.md](./store.md) | SurrealDB graph schema, clocks, Compose |
| [scale.md](./scale.md) | Graph store vs search projection. **Surrealastic** replicates each database and places integer keys across shards. Venus writes once per job. |
| [search-scale](./search-scale/README.md) | `surreal-search` cluster: shards, copies, HA default (3 nodes, `replica_count = 1`). Plan: [search-scale/plan.md](./search-scale/plan.md). |
| [plan.md](./plan.md) | Points at the search-scale board. Extractors are the next board. |

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
| **Lexical search** | `surreal-search` cluster: a projection of heading and mention text, `SEARCH` only on those nodes, sharded and copied by surrealastic ([scale](./scale.md), [search-scale — cluster](./search-scale/README.md#cluster)). | Elasticsearch as a second engine. SurrealDB Enterprise as the shard manager. Full-text indexes on the graph process. A Tantivy index is a later swap of `SEARCH`, same cluster ([scale — compared with Elasticsearch](./scale.md#compared-with-elasticsearch)). |
| **In-doc code names** | **tree-sitter** on fenced code only | CodeGraph CLI and Aider. Those analyze the **product** repo at `productSha` ([code-bind](../Agents/code-bind.md)). They are not the wiki indexer. |
| **Glossary / slang** | **`daachorse`** Aho–Corasick automaton compiled from `glossary.md` (plus page titles and unique headings) | An LLM pass over every paragraph. |
| **Emails, phones, UUIDs, hashes, API paths** | **`regex`** (Rust engine, linear time) | NER for patterns that are regular. |
| **Person / org / team** | Optional **ONNX** NER (`ort` + `tokenizers`), tiny model, **off** until a fixture shows the three tools above miss it | A Python NER service. A general LLM. |
| **Semantic page↔page binds** | Cheap JSON completion, separate model key, **after** extract | BGE-M3 vectors as the edge. LanceDB as a second store. Cursor subscription as the completions API. |

**Vectors are not this slice.** Cosine on embeddings is candidate **search**, not a bind ([LifeIndexing — logical](../Agents/LifeIndexing.md#logical-llm--background)). If heading-body recall for the semantic job is too weak later, store BGE-M3 (BAAI) vectors on the **search** namespace (HNSW) and use them only to pick candidates. Do not add LanceDB while that namespace can hold those vectors. Do not create a `related` edge from a neighbor in vector space.

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
- Show the graph as a published page. AB2 reads it later; this folder does not ship chat.
- Shard documents or place copies with `hash %` the node count ([scale.md — placement](./scale.md#placement)).
