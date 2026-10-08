# Semantic graph


|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **planId**    | `m6-semantic`                                                   |
| **Milestone** | [M6](../../M6/plan.md). Board 6 of the [semantic graph](../README.md#boards). |
| **Duration**  | About 3 weeks after extraction writes links. Vector neighbors are used when board 5 is up. |
| **Board**     | State file added when this board opens.                        |


The model connects documentation that does not already link, when one heading defines, depends on, constrains, or contradicts another ([connect.md](../connect.md#semantic--background-model)). Git and `last_flushed` do not wait on the call.

## Where the board is

Not started. Next is [step 1](#1-step-candidates).

## Gate

| Steps | When |
| --- | --- |
| **1–5** | When `dependsOn` is done. Step 1 needs extraction `links_to`. Step 1 reads HNSW only when [vectors](../vectors/plan.md) has indexed headings. |


## Exit

1. A dirty page is one model call. Candidates are 1-hop links, glossary definitions, shared terms, and HNSW neighbors when the vector index exists.
2. An accepted edge has a quote that is an exact substring of the dirty heading body, a target in the compact map, and a confidence at or above the floor for its type.
3. The pass replaces that heading’s outbound `source = model` edges and can store a gist. Link, glossary, and symbol edges stay.
4. A model failure keeps the previous model edges and sets `semantic_error`. The flush has already committed.
5. A full rescan runs for a schema or edge-type change, for gist drift, and on the nightly timer. One edited page does not rescan the wiki.


## Non-goals

| Later | Why not here |
| --- | --- |
| `related` / `see_also` / `similar` | Dropped even when the model returns them. |
| Theme tags | Future DistilBERT classification. Not this call’s edges. |
| Rediscovering `venus:doc:` | Extraction already wrote `links_to`. |
| Cursor agent harness | The call uses a Venus model key. |


## Steps summary

| # | id | Proves |
| --- | --- | --- |
| [1](#1-step-candidates) | `step-candidates` | **pending.** Candidate headings for one dirty page, including a vector neighbor when the index exists. |
| [2](#2-step-call) | `step-call` | **pending.** One JSON completion per dirty page. |
| [3](#3-step-accept) | `step-accept` | **pending.** Quote, target, type, and confidence floors. |
| [4](#4-step-write) | `step-write` | **pending.** Replace `source = model` outbound edges. Keep the others. |
| [5](#5-step-rescan) | `step-rescan` | **pending.** Full wiki pass on schema change, gist drift, and the nightly timer. |


<a id="1-step-candidates"></a>

### 1. step-candidates

| | |
| --- | --- |
| **n** | 1 |
| **id** | `step-candidates` |
| **title** | Candidate headings for one dirty page |
| **dependsOn** | extraction `step-links` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Build the compact map: `git_path`, title, heading `block_id`, `level`, `text`, and the last gist. Unchanged pages are targets only. Their bodies are not loaded.

For each dirty heading, candidates are, in order:

1. Direct 1-hop (`links_to` out and in).
2. Glossary terms this page mentioned, and those terms’ definition headings.
3. Compact-map headings whose title or gist shares a mentioned term.
4. HNSW neighbors of this heading, when [board 5](../vectors/plan.md) has a vector for it. Absent index: this step is empty, and the call still runs on 1–3.

A heading outside 1–4 is not a target. The model does not receive the whole wiki body.

#### Do not

- Send one call per page pair.
- Treat a vector neighbor as an edge at this step.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Hop | The `links_to` target of a dirty heading is in the candidate list. An unlinked page is absent. |
| Glossary | A mentioned term adds its definition heading. |
| Vector | With the vector index up, a paraphrase neighbor is a candidate. With the index absent, the list is only 1–3 and the call still proceeds. |

- **How:** `cargo test -p venus-graph --test semantic`.


<a id="2-step-call"></a>

### 2. step-call

| | |
| --- | --- |
| **n** | 2 |
| **id** | `step-call` |
| **title** | One completion per dirty page |
| **dependsOn** | `step-candidates` |
| **kind** | implement |
| **status** | **pending** |

#### Work

One JSON completion on a Venus model key. The prompt carries the compact map and, for each dirty heading, the body, a mention summary (term names, symbol names, endpoints), and the candidate list. The model returns the edge array from [connect.md](../connect.md#output) and may return a one-paragraph gist per dirty heading.

Not on convert. Not on `last_flushed`. Not the Cursor agent harness.

#### Do not

- Call once per sentence.
- Put the model text into the git message or the markdown body.

#### Test scenarios

| Name | Pass |
| --- | --- |
| One call | A page with two dirty headings produces one HTTP request. |
| Unchanged | A heading whose `body_hash` matches is not in the prompt body. Its model edges are left for step 4 to keep. |

- **How:** `cargo test -p venus-graph --test semantic`.


<a id="3-step-accept"></a>

### 3. step-accept

| | |
| --- | --- |
| **n** | 3 |
| **id** | `step-accept` |
| **title** | Keep only evidenced edges |
| **dependsOn** | `step-call` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Drop an item when the quote is not an exact substring of the dirty heading `body`, `from_block` is not a heading on this page, the target is not in the compact map, the type is outside the five names, or `confidence` is below the floor (`depends_on` 0.55, `defines` / `constrains` / `contradicts` 0.70, `supersedes` 0.85). Drop `supersedes` when the quote has no replace wording. Drop a `depends_on` that only restates an existing `links_to` unless the quote states a dependency beyond the link.

#### Do not

- Accept `related`.
- Accept a quote that is only a gist of the body.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Quote | An edge whose quote is a substring is kept. A paraphrased quote is dropped. |
| Floor | A `supersedes` at 0.84 is dropped. One at 0.85 with “replaces” in the quote is kept. |
| Link | A `depends_on` that names the same target as `links_to`, with a quote that only repeats the href, is dropped. |

- **How:** `cargo test -p venus-graph --test semantic`.


<a id="4-step-write"></a>

### 4. step-write

| | |
| --- | --- |
| **n** | 4 |
| **id** | `step-write` |
| **title** | Replace this heading’s model edges |
| **dependsOn** | `step-accept` |
| **kind** | implement |
| **status** | **pending** |

#### Work

For each dirty heading, delete outbound edges with `source = model`, then insert the accepted set with that source and the job SHA. Do not delete `source = link`, `glossary`, or `symbol`. Store a returned gist on the heading. It is recall for the next compact map.

On HTTP or JSON failure, leave the previous model edges and set `page.semantic_error`. The wiki job continues. Extract failure for this `docId` skips the call.

#### Do not

- Block Flush on this write.
- Rewrite another page’s outbound model edges because this page linked to it.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Replace | A second pass deletes the previous model edge and inserts the new one. The `links_to` edge remains. |
| Failure | A broken JSON body leaves the previous model edge and sets `semantic_error`. |
| Gist | A returned gist is stored on the heading and is not an edge. |

- **How:** `cargo test -p venus-graph --test semantic` with profile `graph` up.


<a id="5-step-rescan"></a>

### 5. step-rescan

| | |
| --- | --- |
| **n** | 5 |
| **id** | `step-rescan` |
| **title** | Full pass when the incremental rule says so |
| **dependsOn** | `step-write` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Enqueue every page for the semantic pass when the edge-type enum or the schema version changes, when a heading’s incremental gist count reaches 8, or when the dirty hunk is at least half the file. A nightly timer enqueues pages that have not had a successful semantic pass. A glossary definition’s text change enqueues pages that `mentions` that term.

A single edited page still runs steps 1–4 for that page only.

#### Do not

- Rescan every page because one page was edited.
- Re-parse direct links inside this step. That remains extraction.

#### Test scenarios

| Name | Pass |
| --- | --- |
| One page | An edit to page A enqueues A and does not enqueue an untouched page B. |
| Drift | The eighth gist update on a heading enqueues a full re-gist of that page. |
| Glossary | Changing a definition enqueues the pages that mention the term, and no others. |
| Schema | A schema bump enqueues every page. |

- **How:** `cargo test -p venus-graph --test semantic`.
