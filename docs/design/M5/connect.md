# Connect

Two steps, in order. Both write [SurrealDB](./store.md). Neither runs on the pin cut.

```text
extract one dirty page
    → direct edges (this page’s outbound links, catalog contains)
    → SurrealDB commit
    → if any heading body_hash changed: semantic job (async, same worker, later)
```

Direct edges are complete before the model runs. The model reads them. It does not invent a substitute for a `venus:doc:` link.

## Direct — no model

Precision over recall. Rebuild **outbound** for dirty pages only. Inbound is the reverse of someone else’s outbound (SurrealDB traversal `<-links_to`).

M4 already writes page-level `.venus/links.json` on Flush so href rewrite does not `fromDoc` the whole wiki ([M4 step 7](../M4/plan.md#7-step-links)). The graph **reads** that file for page-level endpoints and **re-parses** the dirty markdown for heading-level anchors. Do not replace `.venus/links.json` with SurrealDB. Flush must not wait on this write.

| Edge | From | To | Rule |
|---|---|---|---|
| `contains` | Catalog folder, or page, or heading | Page or child heading | Outline + catalog parent. Folder nodes exist so a tree query does not need the CRDT. |
| `links_to` | Heading that contains the link (else the page) | Page, or heading if the href has a `#` that matches a heading slug **and** a sidecar id in the target | Resolve like import: `venus:doc:<docId>` first, catalog `gitPath` second ([implementation plan — linked docs](../venus-implementation-plan.md#4-linked-docs)) |
| `transcludes` | Heading | Page | `affine:embed-synced-doc` when the markdown form exists |
| `same_page` | Heading | Heading | Previous / next in the file |
| `defines` (`source = glossary`) | Glossary heading | `term` | The glossary page is the definition. Not a model opinion. |
| `defines` (`source = symbol`) | Heading that owns the fence | `symbol` | tree-sitter definition sites ([extract](./extract.md#3-code-symbols--tree-sitter)) |
| `mentions` | Heading | `mention` | Every extract hit in that heading |

`links_to` fields:

| Field | Meaning |
|---|---|
| `resolved` | `true` when `docId` resolved |
| `href` | Raw target (path or comment). Kept when unresolved |
| `via` | `venus_doc` \| `path` \| `links_json` |
| `source` | `link` |

Unresolved hrefs stay in the graph (`resolved = false`, no `out`). They are how a broken link shows up. Do not attach them to the nearest page title.

Catalog-only `git mv` (body hash unchanged): update `git_path` on the page row and the `href` on edges that stored a path. Do not re-gist. Do not call the model. Convert of inbound markdown stays the M4 `links.json` set.

`links_to` is page identity (`docId`), never the path alone. After a move, edges that used `venus:doc:` stay valid without a path rewrite.

## Semantic — background model

Runs **after** extract + direct upsert for this SHA. Separate step in `venus-graph`. Cheap JSON completions on a **Venus** model key. Not the Cursor agent harness. Not on convert. Not on `last_flushed`.

Purpose: connect documentation that does **not** already link, when one heading **defines**, **depends on**, **constrains**, or **contradicts** another.

### Input (one call per dirty page)

Not one call per sentence. Not one call per page pair.

```text
compact map of the whole wiki (targets):
  git_path, title, headings[] { block_id, level, text }
  gist (last stored; stale until this job writes dirty ones)
  tags if present

plus, for each dirty heading on this page:
  body (the sidecar slice)
  mention summary (term names, symbol names, endpoints — not the raw NER logits)
  direct 1-hop (links_to / transcludes targets, and who links_to here)
```

Unchanged pages are targets in the map. They are not re-extracted and they are not re-sent as full bodies. That keeps the model cost on the dirty set, not on pages² ([LifeIndexing — compact map](../Agents/LifeIndexing.md#compact-map-always-available)).

Candidate headings the model may bind to, in order:

1. Direct 1-hop.
2. Glossary terms this page mentioned → their definition headings.
3. Compact-map headings whose title or gist shares a mentioned term.
4. HNSW neighbors, when [board 5](./vectors/plan.md) has a vector for this heading. If a target is not in 1–4, the model does not get the whole wiki body to hunt.

`constrains` and `contradicts` need the dirty heading **body** (or a claim inside it) against that candidate set. Gist-vs-gist of the whole wiki is not allowed.

### Output

JSON array. Each item:

```text
{
  "type": "defines" | "depends_on" | "constrains" | "contradicts" | "supersedes",
  "from_block": "<sidecar blockId on the dirty page>",
  "to_doc": "<docId>",
  "to_block": "<sidecar blockId>",
  "quote": "<exact substring of the dirty heading body>",
  "confidence": 0.0
}
```

| Type | Meaning |
|---|---|
| `defines` | This heading defines a term that is **not** already a glossary `defines`. Rare. |
| `depends_on` | This heading assumes that other heading |
| `constrains` | A must-clause here binds that heading |
| `contradicts` | Both cannot be current |
| `supersedes` | This heading is meant to replace that one |

Drop the item when:

- `quote` is not an exact substring of the dirty heading’s stored `body`
- `from_block` is not a heading on this dirty page
- `to_doc` / `to_block` is not in the compact map
- `type` is anything else, including `related`, `see_also`, `similar`
- `confidence` is below the floor: `depends_on` 0.55, `defines` / `constrains` / `contradicts` 0.70, `supersedes` 0.85
- it would duplicate a direct `links_to` (the link already exists; do not also emit `depends_on` unless the quote states a real dependency beyond the link)

`supersedes` without wording like “replaces” in the quote is dropped even above 0.85. Prefer `constrains` and `contradicts`.

Persist with `source = 'model'`. On each semantic pass for a heading, **delete that heading’s outbound edges where `source = 'model'`**, then insert the new set. Do not delete `source = 'link'`, `glossary`, or `symbol`.

Gist: the same call may return a one-paragraph gist per dirty heading. Store it on the heading. It is recall for the next compact map. It is not an edge and not a git message.

Tags (component / domain) may ride the same call, on the page row, as in [LifeIndexing — tags](../Agents/LifeIndexing.md#tags). They are filters, not the graph.

### When the model does not run

| Case | Action |
|---|---|
| Heading `body_hash` unchanged | Leave model edges |
| Extract failed for this `docId` | Do not call the model for it |
| Model HTTP / JSON failure | Keep previous model edges. Record `semantic_error` on the page. Git stays valid. Retry the job. |
| Incremental gist count ≥ 8, or the dirty hunk is ≳ half the file | Full re-gist from current `.md`, then replace model edges ([LifeIndexing — drift](../Agents/LifeIndexing.md#periodic-full-reindex-drift)) |
| Edge-type enum or schema version changes | Full wiki semantic pass. Direct graph is a re-parse, not a model pass. |

A nightly timer is a safety net. The snapshotter dirty set is the pulse.

Inbound semantic edges to an unchanged heading appear by themselves when a dirty page emits them. Do not rebuild unchanged pages’ outbound model edges. Fan-out extra pages only when a **glossary term’s definition text** changed: enqueue pages that `mentions` that term (their assumption may have moved). That set comes from the graph, not from “every doc”.

## What a connection is not

| Observation | Stored as | Not stored as |
|---|---|---|
| `[Lease](lease.md)` or `venus:doc:` | `links_to` | `depends_on` |
| Glossary word hit | `mentions` + glossary `defines` | model `defines` |
| Same UUID on two pages | two `mention` rows, one normalized key | `depends_on` |
| tree-sitter function in a fence | `symbol` + `defines_symbol` | a call edge |
| NER “Ivan” | `mention` kind `person` | a page↔page bind |
| Embedding neighbor | nothing, until a later slice uses it as a **candidate** | `related` |

## Pack (for later AB2)

This folder does not ship chat. When [AB2](../Agents/agentic-binding.md#ab2--bound-chat) expands a selection, it reads SurrealDB at `last_indexed`, in this order:

1. Containing heading + page
2. `contains`, `links_to` / `transcludes` 1-hop, inbound
3. `mentions` → glossary definition headings and same-page symbols
4. Model `depends_on` / `constrains`; surface `contradicts`
5. Stop at a token budget

AB4 adds `decided-in` later. Do not invent it from snapshot autocomments.

## Do not

- Ask the model to rediscover `venus:doc:` links.
- Emit `related` because two gists are near each other.
- Put model sentences into the snapshot git message or the `.md` body.
- Block Flush on the semantic call.
- Re-run the model for every page when one page was edited.
