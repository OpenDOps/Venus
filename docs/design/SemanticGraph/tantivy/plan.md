# Tantivy search


|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **planId**    | `sg-tantivy`                                                    |
| **Milestone** | None yet. Optional board 4 of the [semantic graph](../README.md#boards). |
| **Duration**  | About 3 weeks once the router and the extractor exist. |
| **Board**     | State file added when this board opens.                        |


SurrealDB `SEARCH` on this cluster is whole-term BM25 (`wiki` analyzer: `class` tokenizer, lowercase, ascii). Prefix search and fuzzy / edit-distance search are unsupported. Both arrive here, because the shard index becomes Tantivy ([semantic graph README](../README.md#compared-with-elasticsearch)).

The graph process stays SurrealDB. The commit set, the copies, the router, and the split stay. A search node stops being a SurrealDB process.

## Where the board is

Not started. Next is [step 1](#1-step-index), after [search-scale step 8](../search-scale/plan.md#8-step-query) and [extraction](../extraction/plan.md).

## Gate

| Steps | When |
| --- | --- |
| **1–5** | When `dependsOn` is done. Step 3 needs the router. Step 5 needs extraction writing real rows. |


## Exit

1. Each search shard copy is a Tantivy index of page, heading, and mention fields, addressed by the same ids the SurrealDB search schema uses.
2. One `layout.write` still commits on the graph. The shard apply updates Tantivy. Entry caps stay 256 docs and 4 MiB.
3. Whole-term search, prefix search, and fuzzy / edit-distance search merge across shards. Hedge, failover, `min_commit`, `partial`, and hydrate behave as in search-scale step 8.
4. A second copy of a shard can be rebuilt from the commit set. Search nodes do not run SurrealDB.
5. The graph node is unchanged. `mention.norm` exact lookup still reads it.


## Non-goals

| Later | Why not here |
| --- | --- |
| HNSW / BGE-M3 | [Vectors](../vectors/plan.md). A different index. Not required for prefix or fuzzy. |
| Forking SurrealDB to embed Tantivy | The search process is replaced. The graph server is not patched. |
| Semantic edges | [Semantic graph](../semantic/plan.md). |
| Changing `hkey`, shard count, or ack | The layout already decided those. |


## Steps summary

| # | id | Proves |
| --- | --- | --- |
| [1](#1-step-index) | `step-index` | **pending.** Tantivy schema per shard copy. BM25. Stable ids. |
| [2](#2-step-apply) | `step-apply` | **pending.** Shard apply writes Tantivy. Commit set stays SurrealDB. |
| [3](#3-step-query) | `step-query` | **pending.** Whole-term, prefix, and fuzzy through the router. |
| [4](#4-step-copies) | `step-copies` | **pending.** Two copies. Refill from the commit set. No SurrealDB on the search node. |
| [5](#5-step-cutover) | `step-cutover` | **pending.** Fixture wiki reads from Tantivy. Graph exact lookup unchanged. |


<a id="1-step-index"></a>

### 1. step-index

| | |
| --- | --- |
| **n** | 1 |
| **id** | `step-index` |
| **title** | Tantivy schema for one shard copy |
| **dependsOn** | search-scale `step-query` |
| **kind** | implement |
| **status** | **pending** |

#### Work

One Tantivy index per shard copy, on that node’s volume. Documents are pages, headings, and mentions. Fields match the search projection: title, `git_path`, heading `text` and `body`, mention `text` and `norm`, `doc_id`, `block_id`, `indexed_sha`.

Text fields use an analyzer that folds ASCII case and splits on Unicode word boundaries, the same cuts the `wiki` analyzer makes for whole-term BM25. Index record ids are the search schema ids (`page:⟨docId⟩`, `heading:[docId, blockId]`, `mention:[docId, blockId, kind, start]`), so a delete by `docId` is a delete by those ids.

BM25 is the score. A term in half or more of the headings in that shard still scores 0, matching the current shard clamp.

#### Do not

- Open a SurrealDB `SEARCH` index in this process.
- Put edges or evidence quotes in the index.
- Change `hkey` or `shard_count`.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Round trip | A heading indexed by id is fetched by that id and removed by that id. |
| Terms | `Access` and `access` are one term. A prefix of the term is a different query, covered in step 3. |

- **How:** `cargo test -p venus-graph --test tantivy`.


<a id="2-step-apply"></a>

### 2. step-apply

| | |
| --- | --- |
| **n** | 2 |
| **id** | `step-apply` |
| **title** | Shard apply writes Tantivy |
| **dependsOn** | `step-index` |
| **kind** | implement |
| **status** | **pending** |

#### Work

The commit set stays the graph SurrealDB database. After it acks, the layout applies each item to the shard copies. The item body becomes Tantivy adds and deletes instead of SurrealQL `SEARCH` statements. A delete item removes that doc’s page, headings, and mentions from the index.

Packs stay at 256 docs and 4 MiB. Ack is still one copy. A copy that does not ack stays queued and is applied later from `_layout_commit`. The caller still hears only the commit lsn.

#### Do not

- Write the Tantivy update outside the layout apply.
- Put the Tantivy index on the graph node.
- Dual-write SurrealDB `SEARCH` and Tantivy for the same shard once this step is on.

#### Test scenarios

| Name | Pass |
| --- | --- |
| One commit | A job’s commit lsn is on the graph. The shard index contains that doc’s headings. |
| Delete | A removed doc is absent from the shard index. |
| Queued | A stopped shard copy catches the items after it returns, without a second job. |

- **How:** `cargo test -p venus-graph --test tantivy` with profile `graph` up.


<a id="3-step-query"></a>

### 3. step-query

| | |
| --- | --- |
| **n** | 3 |
| **id** | `step-query` |
| **title** | Whole-term, prefix, and fuzzy through the router |
| **dependsOn** | `step-apply` |
| **kind** | implement |
| **status** | **pending** |

#### Work

The router from search-scale step 8 keeps map cache, two choices, one hedge, failover, `min_commit`, merge, duplicate removal, `partial`, and hydrate. The per-shard read is a Tantivy collector instead of `@@`.

Three query kinds:

| Kind | Tantivy | Example |
| --- | --- | --- |
| Whole term | BM25 term query | `lease` matches `lease` |
| Prefix | `FuzzyTermQuery::new_prefix` with distance 0 | `lea` matches `lease` |
| Fuzzy | `FuzzyTermQuery` within a Levenshtein distance | `lease` at distance 1 matches `leaes` |

Scores stay per shard. The router merges by score and cuts to `k + offset`. Hydrate still reads the graph. A missing graph row drops the hit. No graph answer: hits from the index fields, marked `unverified`.

`venus_graph::exact` is unchanged and still reads `mention.norm` on the graph.

#### Do not

- Run fuzzy or prefix by scanning SurrealDB.
- Score a fuzzy hit lower inside Tantivy than Tantivy already scores it. The distance is the match rule; the rank is BM25 of the matched term.
- Serve the browser from this step.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Words | Both fixture headings return, one from each shard, no duplicates. |
| Prefix | A prefix of a fixture term returns that heading. A non-prefix does not. |
| Fuzzy | A one-edit misspelling returns the heading. A three-edit string does not, at distance 1. |
| Hedge | Pause the picked copy. The other copy answers. One hedge. |
| Fresh | `min_commit` of a new job never returns the previous heading text. |
| Graph down | Both headings return, marked `unverified`. |
| Exact | `norm` on the graph still resolves while both search indexes are stopped. |

- **How:** `cargo test -p venus-graph --test tantivy`.


<a id="4-step-copies"></a>

### 4. step-copies

| | |
| --- | --- |
| **n** | 4 |
| **id** | `step-copies` |
| **title** | Equal Tantivy copies, refilled from the commit set |
| **dependsOn** | `step-query` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Each shard still has two copies in distinct zones, acked on one. The copy is the Tantivy directory plus the layout cursor the writer already tracks.

A copy that missed entries replays the shard log into its index. A copy with no index, or a cursor behind the retained commit log, is refilled from `_layout_item` and `_layout_commit` on the commit set. A sibling shard is not a source.

The search process is the Tantivy writer and reader. It does not start `surrealdb`.

#### Do not

- Refill a shard from the other shard’s index.
- Let search nodes connect to each other.
- Move the graph off SurrealDB.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Two copies | 1 000 reads of one shard split 40–60% across its two indexes. |
| Wipe | Delete one copy’s directory. Refill restores the same headings from the commit set. The other shard is not read. |
| No server | The search node process has no SurrealDB port. |

- **How:** `cargo test -p venus-graph --test tantivy` with profile `graph` up.


<a id="5-step-cutover"></a>

### 5. step-cutover

| | |
| --- | --- |
| **n** | 5 |
| **id** | `step-cutover` |
| **title** | Fixture wiki reads Tantivy |
| **dependsOn** | `step-copies` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Point the fixture wiki’s search sets at the Tantivy copies. Drop the SurrealDB `SEARCH` indexes from the search path. Whole-term queries that search-scale step 8 defined keep the same hits. Prefix and fuzzy are available on that wiki.

The graph schema, `layout.write`, and `mention.norm` stay. `HIGHLIGHTS` stay off.

#### Do not

- Dual-read SurrealDB `SEARCH` and Tantivy for the same query.
- Change extractor rules in this step.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Cutover | The fixture headings come back from Tantivy. No search query opens a SurrealDB `SEARCH` index. |
| Prefix live | A prefix query on the fixture wiki returns the heading the whole-term query returns for the full word. |
| Graph | Exact `norm` and a graph traversal still answer from `:8000`. |

- **How:** `cargo test -p venus-graph --test tantivy` with profile `graph` up.
