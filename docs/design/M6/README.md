# M6 — Semantic graph: extraction, view, vectors, model edges

M6 builds the wiki graph on the [M5](../M5/README.md) cluster and runs it on Venus’s own `docs/`. That folder becomes one wiki workspace, written by Venus. Each committed page is then indexed by the [semantic graph pipeline](../SemanticGraph/README.md#pipeline). Indexing covers titles and headings, cross-doc links, folders, a dictionary pass (glossary, titles, headings, keywords), tree-sitter on code snippets, regex patterns, optional NER, full-text search, and vector embeddings. Every one of those passes is deterministic except the two ONNX models. After them, a background LLM writes typed edges with a required quote, and the browser draws the graph.

**Status:** not started. **Gate:** [M4](../M4/README.md) closed and M5 [step 7](../SemanticGraph/search-scale/plan.md#7-step-enqueue) done ([plan — gate](./plan.md#gate)). **Plan:** [plan.md](./plan.md). Plan slice: [venus-implementation-plan — M6](../venus-implementation-plan.md#m6--semantic-graph-extraction-view-vectors-model-edges-7-weeks). Pass rules: [extract.md](../SemanticGraph/extract.md). Links and edges: [connect.md](../SemanticGraph/connect.md). Tables: [store.md](../SemanticGraph/store.md).

## Boards

M6 runs four boards of the [semantic graph](../SemanticGraph/README.md#boards). Their design and steps stay in that folder. The order is [plan.md](./plan.md).

| # | Board | M6 lands |
| --- | --- | --- |
| 2 | [extraction](../SemanticGraph/extraction/README.md) | Passes 1–5 below on every uploaded page, and `links_to` across the Venus docs |
| 3 | [graph-view](../SemanticGraph/graph-view/README.md) | The Venus docs graph drawn in the browser with Sigma.js |
| 5 | [vectors](../SemanticGraph/vectors/README.md) | Pass 7 below: BGE-M3 heading vectors, HNSW on the search shards |
| 6 | [semantic](../SemanticGraph/semantic/README.md) | Model edges between Venus docs headings, each with a quote |

## No CodeGraph and no Aider on the wiki

M6 does not run CodeGraph or Aider. Both analyze the **product** git tree at `productSha`, for PR review in AB5 ([code-bind](../Agents/code-bind.md)). The wiki is markdown. Its code is fenced snippets that illustrate a spec, not a program, so there is no call graph to build. Wiki code snippets get tree-sitter symbol extraction in the graph worker, nothing more.

## What the Venus docs contain

Measured on `docs/` on 2026-10-07. These numbers decide what each pass is worth.

| Item | Count | Consequence |
|---|---|---|
| Markdown files | 88 | One catalog page each |
| ATX headings | 1 332 | Heading nodes. Many repeat across files (`Work`, `Do not`, `Test scenarios`), so headings do not all go into the dictionary |
| Fences `text` | 194 | No tree-sitter pass. Inline code and regex still run |
| Fences `bash` | 40 | tree-sitter bash |
| Fences `surql` | 9 | No grammar. Regex only |
| Fences `yaml`, `sql`, `json`, `ts`, `tsx` | 6, 4, 3, 3, 1 | tree-sitter |
| Fences `mermaid`, `markdown`, `venus-step` | 6, 3, 2 | No pass |
| UUIDs | 94 | Regex `uuid` |
| Emails | 36 | Regex `email` |
| Glossary | `design/glossary.md`; terms are bold first cells of table rows | Glossary source rule below |

Most API names (`layout.write`, `ensure`, `fence`) appear as inline code in prose or in `text` fences, not in parsed fences. They are found by the dictionary pass and by full text, not by tree-sitter.

## Upload

- Catalog folders mirror the `docs/` directories. One catalog page per `.md`, named from its first `#` heading, with `gitPath` set to its relative path.
- Each file enters through `MarkdownAdapter.toDoc` into a new space (the [MDGate](../MDGate/README.md) adapter). Relative links between the files become `affine:embed-linked-doc` / `venus:doc:` links. Links that do not resolve stay plain text and are reported.
- One Flush writes `wiki/` and `.venus/ids/` through the M3 snapshotter. The import can be rerun: an unchanged file makes no new page and no git change.
- `img/` assets are copied as blobs. `drafts/` is uploaded too, in its own folder.

## Pipeline

`graph_jobs` from that Flush runs on the committed SHA, one page per job, one `layout.write` per doc ([search-scale plan step 6](../SemanticGraph/search-scale/plan.md#6-step-project)). The passes run in this order on sidecar blocks. A later pass does not overwrite a span an earlier pass claimed, and the longest match wins inside a pass ([extract — pass order](../SemanticGraph/extract.md#pass-order)).

| # | Pass | Tool | Looks at | Emits |
|---|---|---|---|---|
| 1 | Structure | Sidecar walk | Title, ATX headings, links, folder chain | `page`, `heading`, `contains`, `same_page`, `links_to`, folder → page `contains` |
| 2 | Dictionary | `daachorse` (double-array Aho–Corasick) | Prose and inline code, not fences | `mention` kinds `term`, `title`, `heading`; `page.keywords` |
| 3 | Code snippets | `tree-sitter` | Fences with a known info string | `symbol` nodes, `mention` kind `symbol` |
| 4 | Patterns | `regex` | Prose, inline code, and fences, skipping claimed spans | `mention` kinds `email`, `phone`, `uuid`, `hash`, `endpoint` |
| 5 | NER (optional) | `tokenizers` + BERT-class model in ONNX via `ort` | Leftover prose sentences only | `mention` kinds `person`, `org` |
| 6 | Full text | SurrealDB BM25 on the search shards | Title, heading body, keywords, mention text | `@@` rows |
| 7 | Vectors | BGE-M3 in ONNX via `ort` + `tokenizers` ([board 5](../SemanticGraph/vectors/plan.md)) | Heading body | 1 024-dim vector, HNSW index on the search shards |
| 8 | Semantic edges | One JSON completion per dirty page on a Venus model key ([board 6](../SemanticGraph/semantic/plan.md)) | Dirty heading body and its candidate list | `defines`, `depends_on`, `constrains`, `contradicts`, `supersedes`, each with a quote |

Passes 1–7 are the graph job. Pass 8 runs after that job has written, and git and `last_flushed` never wait on it. A heading whose `body_hash`, dictionary id, and fence-language set are unchanged is not re-extracted and not re-embedded. A heading whose body hash is unchanged gets no model call.

### 1. Structure, links, folders

As in [extract — structure](../SemanticGraph/extract.md#1-structure) and [connect — direct](../SemanticGraph/connect.md#direct--no-model). The page title comes from the catalog name. Each folder is a node, and `contains` runs folder → subfolder → page. A cross-doc link is a `links_to` edge at page and heading level when the target resolves (`venus:doc:` first, then path). Otherwise it is stored with `resolved = false` and never pointed at a page by guess.

### 2. Dictionary — `daachorse`

One automaton per workspace, compiled with `daachorse` (`CharwiseDoubleArrayAhoCorasick`, `MatchKind::LeftmostLongest`). Each pattern’s value indexes a pattern table that holds kind and target. The double array keeps several thousand patterns compact, and one scan finds all of them.

| Pattern source | Kind | Target | Taken when |
|---|---|---|---|
| Glossary terms | `term` | Definition site (`blockId` of the row or heading) | Always |
| Page titles | `title` | `docId` | Title is 2+ words or 6+ characters, and no other page has the same title |
| Heading texts | `heading` | Heading `blockId` | 2+ words, unique across the workspace, and not in the stop list (`Work`, `Do not`, `Test scenarios`, `Status`, `Exit`, …) |

- **Glossary source.** The catalog page whose `gitPath` basename is `glossary.md`, case-insensitive. A term is an ATX heading under it, a definition-list term, or the bold first cell (`**Term**`) of a table row. Leading and trailing backticks are stripped and the bare form is added too, so `` `T0` `` and `T0` both match.
- **Case.** Patterns and a scan copy of the text are lower-cased in ASCII only. That keeps byte offsets equal to the original. A glossary term marked `` `case-sensitive` `` goes into a second, exact automaton.
- **Word boundary.** A hit is dropped when the character on either side is a letter or a digit.
- **Priority.** When one surface form has several sources, the order is `term`, then `title`, then `heading`.
- **Dictionary id.** Hash of the glossary blob plus the sorted title and heading pattern sets. The automaton is rebuilt only when this hash changes. A change re-runs pass 2 on every page, not passes 1, 3, 4, or 5.
- **Mentions of titles and headings** are not links. [connect](../SemanticGraph/connect.md) uses them as candidates for the background model. Only explicit links are `links_to`.
- **Keywords.** Per page, the 10 dictionary targets with the highest `count × idf`, where idf is taken across the workspace’s pages. Stored on `page.keywords` and projected to search as a boosted field. Not an edge.

### 3. Code snippets — tree-sitter

As in [extract — code symbols](../SemanticGraph/extract.md#3-code-symbols--tree-sitter). Grammars for M6, matched to the docs above: `bash`/`sh`/`zsh`, `yaml`, `sql`, `json` (keys only, as `type` symbols at depth 1), `ts`/`typescript`, `tsx`, `rust`/`rs`. An unknown or missing info string (`text`, `surql`, `mermaid`) gets no symbol pass. A parse error skips that fence only. A symbol defined in a fence is linked to prose on the same page by the per-page symbol automaton. Cross-page symbol hits need a name of 4 or more characters with exactly one definition. No call graph.

### 4. Patterns — regex

As in [extract — regular patterns](../SemanticGraph/extract.md#4-regular-patterns), crate `regex`.

| Kind | Accept | Reject |
|---|---|---|
| `email` | `name@domain` with a dotted domain | Bare `@handle` |
| `phone` | `+` and 8–15 digits, spaces and dashes allowed | A digit run without `+` |
| `uuid` | 8-4-4-4-12 hex, optional braces | Short hex |
| `hash` | 64 hex anywhere; 40 hex in a fence or inline code | 40 hex in plain prose; 7–12 hex short SHAs |
| `endpoint` | `` `/api/…` `` or `` `/v1/…` `` in inline code or a fence | Slash-words in prose |

Join keys are normalized: UUID, email, and hash lower-case; endpoint without its trailing slash. Two pages with the same UUID meet through the mention’s `norm`, not through an edge.

### 5. NER — optional

As in [extract — NER](../SemanticGraph/extract.md#5-ner--optional-last).

| Piece | Choice |
|---|---|
| Runtime | `ort` (ONNX Runtime) inside the graph worker. No Python, no HTTP sidecar |
| Tokenizer | `tokenizers` with the model’s own `tokenizer.json`. Its offsets map each word piece back to byte offsets in the block |
| Model | A distilled BERT token classifier trained on CoNLL-03 labels, exported to ONNX and int8-quantized, about 40–70 MB. Pinned in the image and checked by sha256, never downloaded per job |
| Labels kept | `PER` → `person`, `ORG` → `org`. `LOC` and `MISC` dropped |
| Decoding | BIO merge on the first word piece of each word, at most 128 tokens per sentence, score ≥ 0.85 |
| Scope | Sentences with a capitalized token no earlier pass claimed, at most 32 per page |
| Default | Off (`ner: false`). The Venus docs workspace keeps it off |

M6 ships NER behind the flag and tests it on fixture sentences. It stays off on the Venus docs because they name few people or organizations, and most capitalized words there are product terms the dictionary already claims. A person or org is a mention only. It never merges with another mention and never becomes an edge.

### 6. Full text

The search shards hold BM25 rows for title, heading body, `page.keywords`, and mention text. The analyzer is the one from the [search-scale README schema](../SemanticGraph/search-scale/README.md#schema). Fresh reads pass the job’s commit as `min_commit` ([scale — read path](../SemanticGraph/scale.md#read-path)).

### 7. Vectors

| Piece | Choice |
|---|---|
| Model | BGE-M3 dense output, ONNX, run by the same `ort` runtime and a `tokenizers` XLM-R tokenizer |
| Input | Heading title plus body, cut to 512 tokens |
| Stored | `vector` (1 024 floats) on the heading’s search row, with `DEFINE INDEX … HNSW DIMENSION 1024 DIST COSINE` in the search namespace only |
| Recompute | Only when `body_hash` changes |
| Use | Candidate search and the hybrid query. A near neighbor is never an edge |

The graph namespace gets no vector field and no index, the same rule as its no-`SEARCH`-index rule.

### 8. Semantic edges

As in [semantic graph](../SemanticGraph/semantic/README.md) and [connect — semantic](../SemanticGraph/connect.md#semantic--background-model). Candidates for one dirty page are its 1-hop `links_to`, glossary definition headings, headings that share a mentioned term, and HNSW neighbors once pass 7 has indexed them ([plan — from board 5 to board 6](./plan.md#from-board-5-to-board-6)). An edge is kept only when its quote is an exact substring of the dirty heading body and its confidence clears the floor for its type. A model failure keeps the previous model edges and sets `semantic_error`. The upload makes every page dirty, so each page gets one call. After that, one edited page is one call.

## Graph view

As in [graph-view](../SemanticGraph/graph-view/README.md). The browser draws pages, headings, `contains`, `same_page`, and `links_to` of the Venus docs. Model edges are drawn once pass 8 has written them. Mentions stay a zoom-in. The view is not a wiki page.

## Query API

`venus_graph::search` and `venus_graph::exact`, all hydrated from the graph:

- title lookup, `@@`, vector neighbors, and a hybrid (reciprocal rank fusion of BM25 and vector ranks);
- outbound and inbound links of a page, and unresolved links;
- the folder path of a page;
- pages that mention a glossary term, title, symbol, UUID, email, or endpoint (`exact` on `norm`);
- a page’s keywords.

## Steps

[plan.md](./plan.md#steps): recon, upload, the extraction, graph-view, vectors, and semantic boards, then query and eval.

## Exit

- The Venus docs can be browsed in the tree and read in a `wiki/` clone.
- Every relative link in `docs/` is a `links_to` edge or a reported unresolved target.
- Every glossary term in `glossary.md` has a `defines` site, and the pages that use it have `term` mentions.
- For a fixed list of questions, the right page is in the top 5 for title, full-text, vector, and hybrid search. Examples: “where is the lease fence checked”, “which pages link to `scale.md`”, “which pages mention `layout.write`”, “what is a pin”.
- `exact` on each of the 94 UUIDs returns every page that holds it.
- Re-indexing an unchanged heading changes no row, edge, or vector, and makes no model call.
- The graph view draws the Venus docs graph, and a picked page shows its in-document and cross-page links.
- Every stored model edge has a quote that is an exact substring of its source heading, and a confidence at or above the floor for its type.
- Killing SurrealDB or the model does not fail a Flush. M1–M5 tests stay green.

## Do not

- Run CodeGraph or Aider over the wiki.
- Call an LLM anywhere except pass 8, and never on the Flush path.
- Run tree-sitter on the markdown itself, or guess a fence’s language.
- Run the dictionary inside fences.
- Turn a title, heading, keyword, NER, or vector hit into a `links_to` edge.
- Download a model at run time.
- Put vectors or `SEARCH` indexes in the graph namespace.
- Turn a vector neighbor into an edge without a model quote.

## Not in M6

Bound chat (AB2), code-bind on the product repo (AB5), and the optional [Tantivy](../SemanticGraph/tantivy/README.md) board.
