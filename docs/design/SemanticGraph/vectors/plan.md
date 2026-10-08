# Vector search


|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **planId**    | `m6-vectors`                                                    |
| **Milestone** | [M6](../../M6/plan.md). Board 5 of the [semantic graph](../README.md#boards). |
| **Duration**  | About 2 weeks after extraction writes heading text and the router can read a shard. |
| **Board**     | State file added when this board opens.                        |


Rules: [grain](./README.md#what-gets-a-vector) and [semantic graph README — tool decision](../README.md#tool-decision). A stored vector is a candidate for search and, later, for the [semantic graph](../semantic/plan.md). It is not an edge.

## Where the board is

Not started. Next is [step 1](#1-step-embed).

## Gate

| Steps | When |
| --- | --- |
| **1–4** | When `dependsOn` is done. |


## Exit

1. Each heading is embedded by the [grain rule](./README.md#what-gets-a-vector): one vector for its own prose, and a paragraph vector only when that prose is long, flat, and far from the heading. An unchanged `body_hash` skips the call.
2. Each search shard copy has an HNSW index of those vectors. A copy is refilled from the commit set.
3. A query returns the nearest headings, merged across shards, and drops a hit whose graph row is gone.
4. Vector hits and word hits bind through the existing graph: same heading id, then one hop. No new edge type.


## Non-goals

| Later | Why not here |
| --- | --- |
| `related` from a neighbor | Cosine is not a bind. |
| LanceDB | The search shard holds the vectors. |
| BERT-base as the encoder | Not trained for retrieval. |
| Theme tags | Future classification against a fixed list. [Extraction](../extraction/README.md). |
| Tantivy | Independent. This index is HNSW beside the lexical index. |


## Steps summary

| # | id | Proves |
| --- | --- | --- |
| [1](#1-step-embed) | `step-embed` | **pending.** BGE-M3 vectors by the grain rule, on the same `layout.write`. |
| [2](#2-step-index) | `step-index` | **pending.** HNSW on each shard copy. Refill from the commit set. |
| [3](#3-step-query) | `step-query` | **pending.** Nearest headings across shards. `min_commit`. Hydrate from the graph. |
| [4](#4-step-bind) | `step-bind` | **pending.** Word hits and vector hits join on the preexisting graph. |


<a id="1-step-embed"></a>

### 1. step-embed

| | |
| --- | --- |
| **n** | 1 |
| **id** | `step-embed` |
| **title** | Vectors by the grain rule |
| **dependsOn** | extraction `step-write` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Run BGE-M3 in ONNX with `ort` and `tokenizers` inside the graph process. The model file is pinned on the image. What is embedded follows [What gets a vector](./README.md#what-gets-a-vector): the heading’s own prose, and a paragraph or list block only when that prose is over 512 tokens, has no child heading, and its cosine to the heading vector is below 0.85. An extra vector keeps the parent heading id and the sidecar `blockId`.

The same `layout.write` that projects the heading carries the vectors. `hkey` does not change. An unchanged `body_hash` does not re-embed. A deleted heading deletes its vectors.

#### Do not

- Download the model on each job.
- Embed with BERT-base.
- Write the vector only on the graph node and skip the search shard.
- Block `last_flushed` on the embedding.

#### Test scenarios

| Name | Pass |
| --- | --- |
| One vector | A short fixture heading is stored with one vector of the model’s dimension. Its child heading is a second vector, and the parent vector’s input does not include the child. |
| Split | A flat heading over 512 tokens stores a paragraph whose cosine to the heading is below 0.85, on the parent heading id plus that paragraph’s `blockId`. A paragraph at or above 0.85 is absent. A fence in that heading has no vector. |
| Skip | A second job with the same `body_hash` does not call the model. A stop-list heading has no vector. |
| Delete | Removing the doc removes the vectors from the item. |

- **How:** `cargo test -p venus-graph --test vectors`.


<a id="2-step-index"></a>

### 2. step-index

| | |
| --- | --- |
| **n** | 2 |
| **id** | `step-index` |
| **title** | HNSW on each shard copy |
| **dependsOn** | `step-embed` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Each search shard copy builds an HNSW index over the heading vectors it stores. The id is the heading id. Copies stay equal: the layout replicates the item, and each copy indexes what it applied. A wiped copy refills vectors from `_layout_item` on the commit set, then rebuilds HNSW. A sibling shard is not a source.

#### Do not

- Put two copies’ vectors in one index.
- Add LanceDB.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Shard | A heading’s vector is on the shard `hkey % shard_count` selects, and not on the other. |
| Refill | After the copy’s index is deleted, refill restores the same heading id and a nearest-neighbor query finds it. |

- **How:** `cargo test -p venus-graph --test vectors` with profile `graph` up.


<a id="3-step-query"></a>

### 3. step-query

| | |
| --- | --- |
| **n** | 3 |
| **id** | `step-query` |
| **title** | Nearest headings |
| **dependsOn** | `step-index`, search-scale `step-query` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Embed the query with the same model. Each shard returns its top `k + offset` by cosine. The router merges, drops duplicate heading ids, and cuts. `min_commit` skips a copy whose cursor is behind that commit. Hydrate reads the graph. A hit whose heading row is gone is dropped. No graph answer: the hit stays, marked `unverified`, with the text stored on the search row.

#### Do not

- Run the nearest-neighbor scan on the graph node.
- Turn a neighbor into an edge in this step.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Near | A query that shares no keyword with a fixture heading still returns that heading when the bodies are paraphrases. |
| Far | An unrelated sentence is absent from the top cut. |
| Fresh | `min_commit` of a rewritten heading never returns the previous body. |
| Gone | A heading deleted on the graph is absent from the result. |

- **How:** `cargo test -p venus-graph --test vectors`.


<a id="4-step-bind"></a>

### 4. step-bind

| | |
| --- | --- |
| **n** | 4 |
| **id** | `step-bind` |
| **title** | Bind vector hits to the graph |
| **dependsOn** | `step-query` |
| **kind** | implement |
| **status** | **pending** |

#### Work

A search may run word search and vector search. Both return heading ids. The result is one set: ids that appear in either list, plus one hop along `links_to`, `contains`, and `mentions` from those headings. The hop is read from the graph. In-document links and cross-page links are both included. The response names which query produced the seed and which edge produced the hop.

No `related` row is written.

#### Do not

- Merge by concatenating texts and re-ranking in an LLM.
- Walk more than one hop in this step.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Shared id | A heading in both the word result and the vector result appears once. |
| Hop | A vector hit’s `links_to` target is in the result and marked as a hop, including a target on another page. |
| No edge | The neighbor that was only near in cosine, and not linked, is a seed hit and gains no graph edge. |

- **How:** `cargo test -p venus-graph --test vectors`.
