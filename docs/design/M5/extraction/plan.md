# Extraction


|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **planId**    | `m5-extract`                                                    |
| **Milestone** | Second board of [M5](../plan.md). Rules: [extract.md](../extract.md). |
| **Duration**  | About 4 weeks. Structure and the glossary are about half of it. |
| **Board**     | State file added when this board opens.                        |


The cluster already accepts one job and writes it with `layout.write` ([search-scale](../search-scale/plan.md) steps 6 and 7). This board replaces the fixture documents with a parse of the committed page. A poison page sets `page.index_error` and leaves the previous pass in place. `last_flushed` does not move.

## Where the board is

Not started. Next is [step 1](#1-step-load). Board 1’s router, recovery, third node, graph copies, backups, and split can proceed beside this board. They do not change the item this board writes.

## Gate

| Steps | When |
| --- | --- |
| **1–8** | When `dependsOn` is done. Step 1 needs a committed wiki and `graph_jobs`. |


## Exit

1. A dirty page at `wikiSha` becomes `page` and `heading` rows whose ids are `docId` and sidecar `blockId`. Body offsets are UTF-8 bytes in the sidecar slice.
2. Glossary terms, fence symbols, and the five regex kinds are `mention` rows. A later pass does not take a span an earlier pass claimed. Longest match wins inside one pass.
3. `links_to` connects headings inside one page and pages across the wiki. An unresolved href is stored and not guessed.
4. The page’s extractor rows, those edges, and its search projection are one `layout.write`. An unchanged heading slice, an unchanged `glossary_id`, and an unchanged fence-language set skip the page. A move that only changes hrefs still refreshes `links_to`.
5. NER runs only when `graph_meta.ner` is true, and only on leftover sentences, at most 32 per page, score at least 0.85.
6. No LLM, no CodeGraph, no Aider, no semantic edges.


## Non-goals

| Later | Why not here |
| --- | --- |
| Folder `contains`, `transcludes` | [connect.md](../connect.md). This board writes `links_to` for in-document and cross-page hrefs. |
| Semantic edges and gist | [Board 6](../semantic/plan.md). One model call per dirty page, after this board’s links exist. |
| HNSW / BGE-M3 | [Board 5](../vectors/plan.md). Candidate search, not a bind. |
| Theme tags | Future. A fine-tuned DistilBERT can label a heading against a fixed theme list. Not a board until that list exists. BERT-base is not the model. |
| Graph view | [Board 3](../graph-view/plan.md). This board writes the edges. It does not draw them. |
| Tantivy | [Board 4](../tantivy/plan.md). Search stays SurrealDB `SEARCH`. |
| Product-repo symbols | CodeGraph / Aider at `productSha`. Fences here are illustrations. |


## Steps summary

| # | id | Proves |
| --- | --- | --- |
| [1](#1-step-load) | `step-load` | **pending.** One page at `wikiSha`: sidecar blocks in, `page` and `heading` out. Unchanged page skipped. |
| [2](#2-step-outline) | `step-outline` | **pending.** `contains` and `same_page`. Link candidates collected, including `#` links inside the file. |
| [3](#3-step-links) | `step-links` | **pending.** `links_to` inside one document and across pages. Unresolved hrefs kept. |
| [4](#4-step-glossary) | `step-glossary` | **pending.** `daachorse` over `glossary.md`. Term mentions. Automaton rebuilt only when the glossary blob changes. |
| [5](#5-step-symbols) | `step-symbols` | **pending.** tree-sitter on known fences. Same-page name mentions. Cross-page only when the name is unique and at least 4 characters. |
| [6](#6-step-regex) | `step-regex` | **pending.** Email, phone, UUID, hash, endpoint. Overlaps with earlier spans dropped. |
| [7](#7-step-write) | `step-write` | **pending.** One `layout.write` replaces this doc’s rows and outbound `links_to`. `pass` drops the previous pass. |
| [8](#8-step-ner) | `step-ner` | **pending.** Off by default. When on, person / org / team mentions only, within the caps in extract.md. |


<a id="1-step-load"></a>

### 1. step-load

| | |
| --- | --- |
| **n** | 1 |
| **id** | `step-load` |
| **title** | Load one committed page into heading rows |
| **dependsOn** | search-scale `step-enqueue` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Read `wiki/<gitPath>` and `.venus/ids/<docId>.json` at the job’s `wikiSha` ([extract.md — pass order](../extract.md#pass-order)). Walk sidecar blocks. Heading identity is the sidecar `blockId`.

Write `page:⟨docId⟩` (`git_path`, `title`, `indexed_sha`, `hkey`, `pass`) and `heading:[docId, blockId]` (`level`, `text`, `body`, `body_hash`). `body` is the markdown from that heading through the next heading of the same or higher level. Child blocks are not nodes.

Skip the page when every heading `body_hash` already matches, `graph_meta.glossary_id` is unchanged, and the set of fence languages is unchanged. A catalog move that only changes hrefs still refreshes direct links later; this skip does not claim to do that refresh.

#### Do not

- Parse the pane splice or store UTF-16 offsets.
- Invent ranges with a second markdown parser when the sidecar has the block.
- Run tree-sitter, the glossary, or regex in this step.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Headings | A fixture page’s ATX headings become `heading` rows keyed by sidecar `blockId`, with `body` ending at the next same-or-higher heading. |
| Skip | A second job with the same slice hashes, glossary id, and fence languages writes nothing. |
| Poison | A page that fails to load sets `page.index_error` and leaves the previous rows. The wiki job continues. |

- **How:** `cargo test -p venus-graph --test extract`.


<a id="2-step-outline"></a>

### 2. step-outline

| | |
| --- | --- |
| **n** | 2 |
| **id** | `step-outline` |
| **title** | Outline edges and link candidates |
| **dependsOn** | `step-load` |
| **kind** | implement |
| **status** | **pending** |

#### Work

`contains`: page → top-level heading, heading → child heading. `same_page`: previous and next heading in document order.

Record link candidates from markdown links and `<!-- venus:doc:<id> -->` on the heading that contains them. Do not resolve them and do not write `links_to`.

#### Do not

- Guess an unresolved href onto a page title.
- Build folder nodes. That is connect.md.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Outline | Nested headings produce `contains` along the outline and `same_page` in file order. |
| Candidate | A `venus:doc:` comment, a markdown link to another page, and a `#` link to a heading in this file are stored on the heading that contains them. |

- **How:** `cargo test -p venus-graph --test extract`.


<a id="3-step-links"></a>

### 3. step-links

| | |
| --- | --- |
| **n** | 3 |
| **id** | `step-links` |
| **title** | In-document and cross-page links |
| **dependsOn** | `step-outline` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Resolve the candidates from step 2 into `links_to`, using [connect.md — direct](../connect.md#direct--no-model). The edge starts at the heading that contains the link, or at the page when the link is outside a heading. `venus:doc:<docId>` wins over a catalog `gitPath`.

Two targets:

| Link | Target |
| --- | --- |
| Inside this document | A `#` slug that matches a heading in this file, and that heading’s sidecar `blockId`. A `venus:doc:` that names this page and a heading in it. |
| Across pages | Another page’s `docId`, or a heading on that page when the href has a `#` and the target page already has that sidecar id. |

Several pages become one graph because each page’s outbound `links_to` meets the others. Inbound is the reverse (`<-links_to`). A job writes this page’s outbound edges only.

An href that does not resolve stays (`resolved = false`, raw `href`, no `out`). A catalog `git mv` with an unchanged body still updates `git_path` and path-shaped hrefs. Edges that used `venus:doc:` stay valid without a path rewrite.

`via` is `venus_doc`, `path`, or `links_json`. `source` is `link`. Read `.venus/links.json` for page-level endpoints. Do not replace that file.

#### Do not

- Guess an unresolved href onto the nearest title.
- Write a semantic edge because two pages link.
- Rebuild inbound edges by scanning every other page.

#### Test scenarios

| Name | Pass |
| --- | --- |
| In document | A heading link to `#` another heading in the same file is `links_to` that heading’s `blockId`. |
| Across pages | Page A’s `venus:doc:` to page B is `links_to` page B. B’s link back to A is a second edge. A traversal from A reaches B and returns. |
| Unresolved | A missing target is stored with `resolved = false` and no `out`. |
| Move | A `git mv` that does not change the body updates `git_path`. A `venus:doc:` edge still points at the same `docId`. |

- **How:** `cargo test -p venus-graph --test extract`.


<a id="4-step-glossary"></a>

### 4. step-glossary

| | |
| --- | --- |
| **n** | 4 |
| **id** | `step-glossary` |
| **title** | Glossary automaton and term mentions |
| **dependsOn** | `step-links` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Compile `glossary.md` with `daachorse` (`CharwiseDoubleArrayAhoCorasick`, `MatchKind::LeftmostLongest`) as [extract.md](../extract.md#2-glossary--aho-corasick) specifies. Terms are ATX headings, definition-list terms, and the bold first cell of a table row. Missing glossary page: empty automaton.

Case-insensitive ASCII fold unless the glossary heading marks `` `case-sensitive` `` on the term’s line. Case-sensitive terms use a second exact automaton. Drop a hit that lacks a Unicode word boundary on either side. Do not scan fenced code. Do scan inline code.

Page titles and unique multi-word headings enter the same automaton as kinds `title` and `heading`, under the rules in [M6 — dictionary](../../M6/README.md#2-dictionary--daachorse). Store the automaton under the glossary file’s git blob hash (`glossary_id`). Rebuild only when that hash changes.

Each hit is a `mention` (`kind = term`, `block_id`, `start`, `end`, `text`, `term_id` = the definition heading’s `blockId`). Heading → mention is `mentions`.

#### Do not

- Scrape every H1 in the wiki into the glossary.
- Scan inside fences.
- Call a model when the glossary is empty.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Longest | `access token` wins over `token`. |
| Boundary | A hit with a letter or digit on either side is dropped. |
| Fence | A term inside a fence is not a mention. The same term in inline code is. |
| Rebuild | A second job with the same glossary blob does not recompile. A changed blob does. |

- **How:** `cargo test -p venus-graph --test extract`.


<a id="5-step-symbols"></a>

### 5. step-symbols

| | |
| --- | --- |
| **n** | 5 |
| **id** | `step-symbols` |
| **title** | Fence symbols and name mentions |
| **dependsOn** | `step-glossary` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Run `tree-sitter` on each fenced block whose info string is in the [extract.md](../extract.md#3-code-symbols--tree-sitter) table (`rust`, `ts`/`tsx`, `js`/`jsx`, `py`, `go`, `bash`/`sh`/`zsh`, and `sql` only if that grammar is already a small dependency). Unknown or empty info string: no symbol pass on that fence. A parse error skips that fence.

Emit function, type, and import names. Drop names shorter than 2 characters and every local. Symbol id is `(lang, kind, name)` scoped to this `docId`. The containing heading `defines` the symbol (`source = symbol`). A second fence with the same triple adds a def site on the same node.

Same page: a case-sensitive automaton of those names over prose and inline code, with word boundaries, emits `mention` kind `symbol`.

Cross-page: an inline-code span equals a symbol defined on exactly one other page, and the name length is at least 4. Otherwise the span stays plain text.

#### Do not

- Run tree-sitter on the markdown file.
- Emit a `calls` edge.
- Read the product repository.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Fence | A rust fence yields function and type symbols and a `defines` edge from its heading. Locals are absent. |
| Unknown | A `text` fence produces no symbol. Regex may still run later. |
| Same page | Prose that names a symbol defined on that page becomes a `symbol` mention. |
| Cross-page | A unique name of length ≥ 4 in inline code on another page becomes a mention. Two definitions of that name produce none. |

- **How:** `cargo test -p venus-graph --test extract`.


<a id="6-step-regex"></a>

### 6. step-regex

| | |
| --- | --- |
| **n** | 6 |
| **id** | `step-regex` |
| **title** | Email, phone, UUID, hash, endpoint |
| **dependsOn** | `step-symbols` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Crate `regex`, linear patterns, as the table in [extract.md](../extract.md#4-regular-patterns). A hit that overlaps a glossary or symbol span is dropped.

Normalize join keys: UUID and email and hash lowercase, endpoint without a trailing slash. `text` stays as written. These mentions are not edges to other pages. `target` is empty. Other pages join on `norm`.

#### Do not

- Treat a phone or UUID as a semantic bind.
- Accept a bare `@handle`, a 10-digit run in prose, or 40 hex digits in ordinary prose.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Kinds | One fixture page yields one mention of each kind that extract.md accepts, and none of the rejected shapes. |
| Overlap | A UUID inside a glossary span already claimed is not a second mention. |
| Norm | Display `text` keeps case. `norm` is the folded join key. |

- **How:** `cargo test -p venus-graph --test extract`.


<a id="7-step-write"></a>

### 7. step-write

| | |
| --- | --- |
| **n** | 7 |
| **id** | `step-write` |
| **title** | One layout write replaces the page |
| **dependsOn** | `step-regex` |
| **kind** | implement |
| **status** | **pending** |

#### Work

The job claims `graph_jobs` as it does today. The body is this page’s extractor rows, not the fixture list. One `layout.write`: graph rows for the page, headings, symbols, terms, mentions, outline edges, and this page’s outbound `links_to`; search rows for title, heading text and body, and mention text and `norm`, each carrying `indexed_sha`.

`page.pass` increments. Mentions and extractor edges from an older pass are deleted in that same write. A failed page does not advance `pass`.

#### Do not

- Write a search row outside surrealastic.
- Start a second job for the search projection.
- Move `last_flushed`.

#### Test scenarios

| Name | Pass |
| --- | --- |
| One write | One claimed job produces one commit lsn. Graph headings and the search projection share `indexed_sha`. |
| Replace | A second extract of the same page deletes the previous pass’s mentions and leaves the new ones. |
| Cluster down | Search nodes stopped: the graph commit still acks, the shard stays queued, and the flush that enqueued the job has already committed. |

- **How:** `cargo test -p venus-graph --test extract` with profile `graph` up.


<a id="8-step-ner"></a>

### 8. step-ner

| | |
| --- | --- |
| **n** | 8 |
| **id** | `step-ner` |
| **title** | Optional NER mentions |
| **dependsOn** | `step-write` |
| **kind** | implement |
| **status** | **pending** |

#### Work

Default `graph_meta.ner` is false. The job does not load the model.

When the flag is true: `ort` and `tokenizers` in the graph process, pinned model file on the image. Only leftover prose sentences that contain a capitalized token not already covered. At most 32 sentences per page. Minimum score 0.85. Labels `person`, `org`, `team` only. Each hit is a `mention` with an empty `target`. Two pages that share a name share `norm`. They do not share an entity row and they do not gain an edge.

#### Do not

- Download the model on each job.
- Let NER override a glossary term or a tree-sitter symbol.
- Turn the flag on in the fixture workspace.

#### Test scenarios

| Name | Pass |
| --- | --- |
| Off | `ner` false: a sentence with a person name produces no `person` mention, and the model file is not opened. |
| On | `ner` true: one high-score person mention, no mention below 0.85, and the 33rd leftover sentence is left for a later pass. |
| Claimed | A capitalized glossary term is not re-labeled as a person. |

- **How:** `cargo test -p venus-graph --test extract`.
