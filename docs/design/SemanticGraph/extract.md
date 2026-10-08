# In-document extract

Deterministic pass over **one committed page**. Output is nodes and edges **inside** that page, plus the raw material [connect.md](./connect.md) uses for cross-page links. No network. No model.

Read `wiki/<gitPath>` and `.venus/ids/<docId>.json` at `wikiSha`. Walk **sidecar blocks**, not a second markdown parser that invents ranges. Heading and code-block identity is the sidecar `blockId` ([MDGate subset](../MDGate/subset.md)). Offsets below are **UTF-8 byte offsets inside that block’s sidecar slice** of the committed file. Do not store pane splice `[start,end)`.

## Pass order

One page, one `pass` counter (see [store](./store.md#clocks)):

| # | Pass | Where it looks | Emits |
|---|---|---|---|
| 1 | **Structure** | Sidecar + ATX headings, fenced code, markdown links, `<!-- venus:doc:<id> -->` | `heading` nodes, `contains`, `same_page`, link **candidates** |
| 2 | **Dictionary** | Prose slices and inline code (not inside fences) | `mention` kind `term` / `title` / `heading`, `page.keywords` |
| 3 | **tree-sitter** | Each `affine:code` / fenced block, language from the info string | `symbol` nodes, `mention` kind `symbol` |
| 4 | **regex** | Prose and fence text, skipping spans already claimed | `mention` kind `email` / `phone` / `uuid` / `hash` / `endpoint` |
| 5 | **NER** | Only if the workspace flag `ner` is on, and only leftover sentences | `mention` kind `person` / `org` / `team` |

Later passes do not overwrite an earlier span. Longest match wins inside one pass.

Skip the whole page’s extract when every heading slice hash matches the hashes already on those `heading` rows **and** the glossary automaton id is unchanged **and** the fence-language set is unchanged. Still refresh direct `links-to` if `.venus/links.json` says this page’s hrefs moved (catalog `git mv` without a body change).

## 1. Structure

Index nodes are **headings** (and the page title), not 512-token chunks. Same grain as [LifeIndexing](../Agents/LifeIndexing.md#node-identity).

| Node | Id | Fields |
|---|---|---|
| Page | `docId` | `git_path`, `title`, `indexed_sha` |
| Heading | `docId` + sidecar `blockId` of that ATX heading or title | `level`, `text`, `body` (markdown from this heading through the next same-or-higher heading), `body_hash` |

`contains`: page → top-level heading; heading → child heading (outline). `same_page`: previous / next heading in document order.

Child **blocks** (paragraph, list, code) are not nodes. Their text belongs to the containing heading’s `body`. A mention records the **inner** `blockId` when the sidecar has one, so a later apply can still point at the paragraph.

Code fences are located by sidecar blocks whose markdown is a fence, or by the fence inside the heading body if the sidecar only marked the heading. Prefer the sidecar row.

## 2. Glossary — Aho–Corasick

**Source of terms:** the catalog page whose `gitPath` basename is `glossary.md`, case-insensitive. Each ATX heading, each term in a definition list, or the bold first cell (`**Term**`) of a table row under it is one term. The heading that introduces the term is the definition site (`defines`, source `glossary` — written in [connect.md](./connect.md), not by the model).

If that page does not exist, the automaton is empty. Do not scrape every H1 in the wiki into the glossary.

**Compile** the term list into one automaton with crate `daachorse` (`CharwiseDoubleArrayAhoCorasick`). Page titles and unique multi-word heading texts go into the same automaton as kinds `title` and `heading`; their rules, and `page.keywords`, are in [M6 — dictionary](../M6/README.md#2-dictionary--daachorse).

- `MatchKind::LeftmostLongest`, so `access token` wins over `token`.
- Case-insensitive for glossary terms unless the glossary heading says the term is case-sensitive (a mark in that heading’s body: `` `case-sensitive` `` on the same line as the term). Default is insensitive. Fold ASCII only, on the patterns and on a scan copy of the text, so byte offsets stay those of the original; case-sensitive terms go into a second, exact automaton.
- After each hit, require a Unicode word boundary on both sides. The automaton itself does not know boundaries; drop the hit if either side is a letter or number.
- Store the automaton keyed by the glossary file’s git blob hash (`glossary_id`). Rebuild only when that hash changes.

**Scan** prose heading bodies. Do not scan inside fenced code (tree-sitter owns those names). Do scan inline code spans — `` `lease` `` is often the glossary term.

Each hit is a `mention`:

| Field | Value |
|---|---|
| `kind` | `term` |
| `block_id` | Sidecar block that contains the hit |
| `start`, `end` | Byte offsets in that block slice |
| `text` | Surface form as written |
| `term_id` | Stable id of the glossary term (the definition heading’s `blockId`) |

Hundreds of megabytes per second is the point: one pass, all terms, no model.

## 3. Code symbols — tree-sitter

**Scope:** fenced code blocks in the **wiki** page. Not the product repository. Product symbols at `productSha` stay CodeGraph CLI / Aider ([code-bind](../Agents/code-bind.md)).

Crate `tree-sitter` plus grammars for the fence info string. First set:

| Info string | Grammar |
|---|---|
| `rust`, `rs` | rust |
| `ts`, `tsx`, `typescript` | typescript |
| `js`, `jsx`, `javascript` | javascript |
| `py`, `python` | python |
| `go` | go |
| `sql` | sql (only if the grammar is already a small dependency; otherwise skip) |
| `bash`, `sh`, `zsh` | bash |

Unknown info string, or no info string: **no symbol pass**. Regex still runs. Do not guess the language. A parse error on a fence: skip that fence, index the rest of the page.

**Extract (precision, not a call graph):**

| Capture | `symbol.kind` |
|---|---|
| Function / method name | `function` |
| Type, class, struct, enum, interface | `type` |
| Import / use module path | `import` |

Do **not** emit every local variable. Names shorter than 2 characters are dropped.

The fence’s containing heading `defines` that `symbol` (`source = symbol`). Symbol id is `(lang, kind, name)` **scoped to this `docId`** for the definition. A second fence on the same page with the same triple updates the same node (add a second def site); it does not fork.

**Mentions of those names in prose** on the **same page**: a second, case-sensitive automaton built from this page’s defined names, run on prose and on inline code. Word boundaries apply. This is how a spec sentence “`acquire` freezes the page” links to the fence that defines `acquire`.

**Cross-page symbol hits** are narrower, so short names do not glue the wiki together:

- Inline code whose text equals a symbol defined on **another** page, **and**
- the name length is ≥ 4, **and**
- exactly one definition of that `(lang, kind, name)` exists in the current graph.

Otherwise the inline code stays plain text. Ambiguous names are not edges.

Do not build a call graph from wiki fences (no `calls` edge). A fence is an illustration, not the program.

## 4. Regular patterns

Crate `regex`. Patterns are linear (Rust `regex` refuses ambiguous exponential expressions). Conservative. A hit that overlaps a glossary or symbol span is dropped.

| `kind` | Accept | Reject |
|---|---|---|
| `email` | `name@domain` with a dotted domain | Bare `@handle` |
| `phone` | Leading `+` and 8–15 digits (E.164-like), spaces and dashes allowed | Any 10-digit run in prose |
| `uuid` | 8-4-4-4-12 hex, optional braces | Short hex |
| `hash` | 64 hex digits (SHA-256) or 40 hex digits **inside a fence or inline code** | 40 hex in ordinary prose (too many false shas) |
| `endpoint` | Path starting with `` `/api/ `` or `` `/v1/ `` through the inline-code span, or the same shape inside a fence | Every slash-word in prose |

These mentions answer “where is this error id / this route”. They are **not** `depends-on` edges. Two pages that contain the same UUID are found by a traversal through the mention node ([store — queries](./store.md#queries)), not by a model.

## 5. NER — optional, last

Use this only when the three passes above are the wrong tool: a sentence such as “ask Ivan on the billing team about the gateway settings”, where `Ivan` is a person and `billing` is a team, and neither is in the glossary.

| Piece | Choice |
|---|---|
| Runtime | `ort` (ONNX Runtime) **in the graph process**. No Python, no sidecar HTTP. |
| Tokenizer | `tokenizers` (Hugging Face), same vocab as the ONNX file |
| Model | A small encoder NER (distilbert-class, about 40–60 MB). Pinned file under the graph image, not downloaded on each job. |
| Labels kept | `person`, `org`, `team` (map `ORG` / `MISC` only when the model’s label set has a team-like class; otherwise drop `MISC`) |
| Default | **Off** (`ner: false` on the workspace graph meta) |

**When on, still narrow:**

- Only prose sentences that contain a capitalized token not already covered by a term, symbol, or regex mention.
- At most **32** such sentences per page per pass. The rest wait for a later pass if the page is indexed again.
- Minimum model score **0.85**. Below that, drop.
- Do not let NER override a glossary term or a tree-sitter symbol.

A person, org, or team is a `mention` only (same as a UUID). Two pages that both say `Ivan` are two mentions with the same `norm`. They do not become one entity, and they do not become a page↔page edge. “Where else is this name” is a query on `norm`. A real bind still needs a semantic quote ([connect](./connect.md)).

## Mention record

Every hit, all kinds:

```text
mention
  doc_id, block_id, start, end
  kind          term | title | heading | symbol | email | phone | uuid | hash | endpoint | person | org | team
  text          surface form
  target        term id, docId (title), heading blockId, or symbol id (empty for email, phone, uuid, hash, endpoint, person, org, team — other pages join on `norm`)
  pass
  indexed_sha
```

Normalize join keys: UUID lowercase; email lowercase; endpoint without trailing slash; hash lowercase. Display `text` stays as written.

Heading → mention is `mentions` (edge). The heading `body_hash` covers the slice, so an unchanged heading does not rewrite mentions.

## Do not

- Run tree-sitter on the whole markdown file (headings are not a programming language).
- Run the glossary automaton inside fences.
- Extract every identifier.
- Call the semantic model because a regex missed. Fix the pattern or leave the span unindexed.
- Enable NER to “be safe” on every page. It is the slow path (milliseconds and tens of MB), and the default is off.
- Use pane UTF-16 offsets as `start` / `end`.
