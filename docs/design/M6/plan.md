# M6 — plan

|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **Milestone** | [M6](./README.md). Boards 2, 3, 5, and 6 of the [semantic graph](../SemanticGraph/README.md#boards), run on the Venus docs. |
| **Duration**  | About 7 weeks. Extraction is about 4. The view, vectors, and the model overlap after extraction writes `links_to`; the model is the longest at about 3. |
| **Board**     | Each board adds its state file when it opens. |

Four boards, in order, plus the Venus docs upload and the evaluation that close the milestone. Graph design: [README](../SemanticGraph/README.md), [store.md](../SemanticGraph/store.md), [extract.md](../SemanticGraph/extract.md), [connect.md](../SemanticGraph/connect.md). The cluster these boards write to is [M5](../M5/README.md).

| # | Board | Status | What it lands | Why it is needed |
| --- | --- | --- | --- | --- |
| 2 | [Extraction](../SemanticGraph/extraction/plan.md) | Not started. Starts once board 1 can take a job, which it can. Does not wait for the router, HA, backups, or shard splits. | Each committed page becomes headings, glossary mentions, code symbols, and regex mentions. `links_to` joins headings inside that page and joins pages to each other, so the wiki is one graph. One `layout.write` per page. NER stays off. | Board 1 writes fixture rows. This board is how the wiki becomes those rows, and how separate pages connect. Without the cross-links the store is a pile of pages. |
| 3 | [Graph view](../SemanticGraph/graph-view/plan.md) | Not started. After board 2 has `links_to`. | A browser view of pages, headings, in-document links, and cross-page links. Drawn with [Sigma.js](https://www.sigmajs.org/) on WebGL. Pan, zoom, and pick stay on the GPU. Layout runs in a worker. | The graph is hidden until someone can see it. A canvas 2D library stalls once the wiki is thousands of edges. WebGL is the fast path, and Sigma is the adopted engine that already draws node-link graphs that way. |
| 5 | [Vectors](../SemanticGraph/vectors/plan.md) | Not started. After board 2 writes heading text and board 1 can read a shard ([M5 step 8](../SemanticGraph/search-scale/plan.md#8-step-query)). Does not wait on Tantivy or the view. | BGE-M3 vectors by the [grain rule](../SemanticGraph/vectors/README.md#what-gets-a-vector), HNSW on each search shard copy, nearest-heading search, then one hop on the graph extraction already wrote. | Word search misses a heading that means the same thing and shares no term. The existing `links_to`, `contains`, and mention edges are what bind a vector hit to a word hit. A neighbor is a candidate, not a new edge. |
| 6 | [Semantic graph](../SemanticGraph/semantic/plan.md) | Not started. After board 2 has `links_to`. Uses board 5 neighbors when that index exists. | An LLM writes `defines`, `depends_on`, `constrains`, `contradicts`, and `supersedes`, each with a quote that is a substring of the source heading. One call per dirty page. A full rescan runs when the schema changes, gist drifts, or the nightly timer fires. | Static extract and real hrefs do not see a heading that depends on another without linking to it. The model supplies that edge, and the quote is the evidence. |

Board 2 fills the store from git and ties pages together. Board 3 draws that graph. Board 5 adds meaning to search. Board 6 adds the edges a link cannot express. Board 4, [Tantivy](../SemanticGraph/tantivy/plan.md), is optional and is not part of M6.

## Gate

[M4](../M4/README.md) closed, and M5 [step 7](../SemanticGraph/search-scale/plan.md#7-step-enqueue) done (`graph_jobs` after `last_flushed`). M5 steps 9–14 run beside this milestone. They do not change the item these boards write. Board 5 also waits for M5 step 8. Each board’s own gate is in its plan.

## Steps

| # | Step | Proves |
| --- | --- | --- |
| 1 | recon | The [Venus docs counts](./README.md#what-the-venus-docs-contain), re-measured; the stop list; the grammar crates and their sizes |
| 2 | upload | The 88 files become catalog pages ([upload](./README.md#upload)); a rerun is a no-op; unresolved links are reported |
| 3 | [extraction](../SemanticGraph/extraction/plan.md#steps-summary), steps 1–8 | Pages, headings, folders, `links_to`; the `daachorse` dictionary (glossary, titles, headings, keywords); tree-sitter symbols and same-page linking; regex kinds and normalization; NER behind the flag on fixture sentences |
| 4 | [graph view](../SemanticGraph/graph-view/plan.md#steps-summary), steps 1–4 | The Venus docs graph drawn in the browser. Starts after [extraction step 3](../SemanticGraph/extraction/plan.md#3-step-links) |
| 5 | [vectors](../SemanticGraph/vectors/plan.md#steps-summary), steps 1–4 | BGE-M3 ONNX, HNSW on search, recompute on `body_hash` only |
| 6 | [semantic](../SemanticGraph/semantic/plan.md#steps-summary), steps 1–5 | Model edges with a quote on the Venus docs. Starts after extraction step 3; reads HNSW once step 5 has indexed headings |
| 7 | query | The [query API](./README.md#query-api), hybrid ranking |
| 8 | eval | The [exit](./README.md#exit) questions |

Steps 4, 5, and 6 can run beside each other once extraction writes `links_to`.

## From board 5 to board 6

Board 5 measures distance. Board 6 may turn a near heading into a typed edge. The neighbor is not stored first and labeled afterwards.

1. Board 5 runs BGE-M3 on each heading the [grain rule](../SemanticGraph/vectors/README.md#what-gets-a-vector) keeps, and writes those vectors into HNSW on the search shard. HNSW is the index that walks those vectors and returns the nearest headings. A nearest heading is a search hit. No similarity edge is written.
2. Board 6 builds the candidate list for one dirty page: direct `links_to`, glossary definition headings, compact-map headings that share a mentioned term, then the HNSW neighbors from board 5 when that index exists. Without the index, the call still runs on the first three.
3. One JSON completion on a Venus model key reads the dirty heading body and that list. It may return `defines`, `depends_on`, `constrains`, `contradicts`, or `supersedes`, with a quote and a confidence. Two headings that are only near each other produce no row.
4. The edge is stored when the quote is an exact substring of the dirty heading and the confidence clears that type’s floor. Otherwise the pair stays a search hit. `links_to` from board 2 is left as it is.

Details: [vectors](../SemanticGraph/vectors/plan.md), [semantic graph](../SemanticGraph/semantic/plan.md), [connect.md — semantic](../SemanticGraph/connect.md#semantic--background-model).

Theme tags are future work, not a board. A fine-tuned DistilBERT can label a heading against a fixed theme list. BERT-base is not that model, and an off-the-shelf encoder cannot invent the wiki’s theme names. The note is on [extraction](../SemanticGraph/extraction/README.md).

## Exit

[M6 — exit](./README.md#exit), with each board’s own exit met: [extraction](../SemanticGraph/extraction/plan.md#exit), [graph view](../SemanticGraph/graph-view/plan.md#exit), [vectors](../SemanticGraph/vectors/plan.md#exit), [semantic](../SemanticGraph/semantic/plan.md#exit).
