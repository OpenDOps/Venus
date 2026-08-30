# LifeIndexing

**Status:** design. **Main product feature** (spatial + temporal spec graph): [Agents README](./README.md). **AB1** / **AB4** in [agentic-binding.md](./agentic-binding.md). Not an M3 exit. Consumes a **git SHA** after [LiveSnapshot](../LiveSnapshot/README.md) convert. Bound chat (**AB2**) expands a selection from this index. Chat-edit (**AB3**) uses the same pack on a lease `T0`. **AB4** adds the temporal why chain (comment-commits + comments). Lease packing later uses the index **at `T0`** (or last indexed SHA ≤ that clock). Exporter: [MDGate](../MDGate/README.md). Sidecar / `gitPath`: [datamodel git](../datamodel/git.md).

A **life index** is a hidden photograph of the published wiki in **two dimensions**:

| Axis | Nodes | Edges |
|---|---|---|
| **Spatial** | Headings / pages at a SHA | Direct links; logical `defines` / `depends-on` / `constrains` / `contradicts` |
| **Temporal** | Git commits + review comments on those parts | `changed-at` / `decided-in` / `commented-on` / `supersedes` / `reverts` |

It is not the spec. It is not shown on the page. It is not a fourth source of truth beside CRDT and git. Snapshot autocomments and hidden change-gists are **not** human why ([two git classes](../venus-design.md#apply-and-git)).

## Invariant

Live collaboration never waits on gists, tags, or either graph. Same rule as pin convert ([LiveSnapshot — invariant](../LiveSnapshot/README.md#invariant), [HA locked #1](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3)).

```text
pin cut → fromDoc + sidecar → wiki/*.md → git commit → last_flushed
                                                         │
                                                         ▼  enqueue (SHA + dirty docIds)
                                              LifeIndexing job (async)
```

The indexer reads **committed** markdown + `.venus/ids/` at that SHA. It does **not** read the live Store, the spectator pane splice ([live-pane.md](../MDGate/live-pane.md)), or the pin buffer after it is dropped.

| Must | Must not |
|---|---|
| Same dirty grain as the snapshotter: **page** (`docId`), not keystroke | `fromDoc` or LLM on every CRDT op |
| Clock = git SHA / pin clock of that flush | Mix pane `[start,end)` with index edges |
| Failure of the LLM job leaves git HEAD valid | Retry the pin, hold the cut, or delay `last_flushed` |
| Gists / logical edges / change-gists stay hidden | Print them as wiki body or as a snapshot **git message** |

Snapshot commits stay autocomment `snapshot: <title>` ([two git classes](../venus-design.md#apply-and-git)). A hidden **change gist** may describe the diff for the indexer. It is not a comment-commit why.

## Hang on LiveSnapshot

Flush is still pin-then-convert ([README — pin then convert](../LiveSnapshot/README.md#pin-then-convert)). LifeIndexing is **after** step 4 (commit) and does not extend the pin cut ([HA — convert and git](../LiveSnapshot/high-availability.md#convert-and-git)).

```text
1. Dirty set     docIds with a new Yjs clock
                 ∪ catalog gitPath changes
                 ∪ blobs
2. Pin           copy Yjs bytes (+ catalog) — live never waits
3. Convert       fromDoc on the pin; sidecar ranges
4. Commit        wiki/<gitPath>.md + .venus/ids
                 autocomment or required why (lease accept)
4b. Direct graph parse of dirty .md (no LLM; optional same commit)
5. Release       last_flushed; drop pins (keep if T0)
6. Enqueue       LifeIndexing { wikiSha, dirtyDocIds, reason }
                 gist + tags + logical graph (LLM, async)
```

`reason` is the snapshotter’s `idle` | `flush` | `lease` ([HA — queue](../LiveSnapshot/high-availability.md#queue)). A lease flush may kick the indexer so `T0` packing is not missing the pages just committed; it still **must not** sit on the cut or on `fromDoc`.

Catalog-only `git mv` (body clock unchanged): update `path` on existing index rows; do not re-gist the body; rewrite direct edges whose URLs changed.

Poison page: fail that `docId`’s index row; do not fail the flush or the fleet ([HA — high availability](../LiveSnapshot/high-availability.md#high-availability)).

## Node identity

Index nodes are **headings** (and the page title), not 512-token chunks.

| Id | Source |
|---|---|
| `docId` | Catalog / space id (stable). Path is not identity. |
| `blockId` | Sidecar row for that ATX heading (`affine:paragraph` `h1`…`h6`) or page title range ([subset](../MDGate/subset.md)) |
| `gitPath` | Catalog at this SHA |
| Heading text | Outline; used in the compact map, not as the key |

File-level rows exist for tags and the gist rollup. Logical **binds** are heading → heading (other file or same file). Selection expansion walks heading containment (outline) then edges.

Skip LLM work when the heading’s markdown hash (sidecar slice) is unchanged. Dirty **page** is not dirty **meaning** (stringify gaps are not a gist update).

## Compact map (always available)

Every index job, incremental or full, may send the model a **map of the whole wiki** as targets. Unchanged files are not re-extracted; they still appear here.

```text
for each page at this SHA:
  gitPath
  title
  headings[]     { blockId, level, text }
  gist           (last stored; maybe stale until this job writes dirty ones)
  tags           component[] + domain[]
```

This pack is how “reindex changed files against others” stays O(dirty) LLM calls, not O(pages²).

## Incremental (steady state)

Trigger = snapshotter dirty set for this SHA, not a timer.

### New page

Summarize from **full** committed markdown (heading-grained gists + file rollup). Tag. Direct-parse links. Logical-graph call for its headings against the compact map.

### Existing dirty page

```text
old gist (per heading) + attributed hunks (md vs previous SHA, sidecar ids)
        → new gist
        → tags upsert
        → drop outbound logical edges whose `from` is a dirty heading
        → LLM: dirty heading gists + compact map → new outbound binds
```

Prefer hunks attributed like apply ([MDGate apply](../MDGate/apply.md)), not raw `git diff` of adapter whitespace.

Inbound edges to an unchanged heading are the outbound edges from dirty headings. Do not rebuild the whole logical graph. Fan-out to other pages only when a **defined term** on D changed (invert `defines`); not “every page.”

### Unchanged page

Leave gist, tags, and outbound logical edges. Stay in the compact map as a **target**. Recompute direct inbound automatically when a dirty page’s links change (reverse index).

## Periodic full reindex (drift)

Rolling “old gist + diff” drifts: constraints flatten, hallucinated bullets stick, adapter jitter looks like meaning.

Per `docId` (and on a wiki-wide schema bump):

| Signal | Action |
|---|---|
| `incrementalCount` since last full gist ≥ N (start: 8) | Full re-gist from current `.md`, not from the last gist |
| Cumulative hunk size ≳ half the file | Same |
| Cheap “is this gist still true of HEAD?” fails | Same |
| Heading `blockId` vanished from sidecar | Drop that node and its edges |
| Index schema / edge-type enum changed | Full wiki pass (gists + logical); direct graph is a re-parse |

Full reindex still uses the **committed SHA**, not the live CRDT. It may batch dirty-looking pages; it must not run inside the pin cut.

A nightly timer is a **safety net**, not the steady state. The snapshotter dirty set remains the pulse.

## Two graphs

### Direct (links) — no LLM

Deterministic. Precision over recall. Rebuild outbound for dirty `.md` only; maintain a reverse index for inbound.

| Edge | From |
|---|---|
| `contains` | Catalog folder → page; heading → child blocks (outline) |
| `links-to` | Markdown links, `affine:embed-linked-doc`, `<!-- venus:doc:<id> -->` ([subset](../MDGate/subset.md#linked-doc-stable-form)) |
| `transcludes` | `affine:embed-synced-doc` when present |
| `same-page` | Prev/next heading (sequence) |

May run in the convert worker **after** `.md` is written and the cut is released. Cheap enough for the **same** snapshot commit if stored under `.venus/` (optional cache). Always re-derivable from clone of `wiki/` + sidecar. Do not ask the model to invent these.

### Logical (LLM) — background

Typed binds between headings, using the compact map plus dirty gists. Cheap model, structured JSON, **required evidence** (`docId`, `blockId`, short quote).

| Type | Meaning |
|---|---|
| `defines` | This heading (or block) is the definition of a term |
| `depends-on` | This heading assumes that definition or section |
| `constrains` | This must-clause binds that API / page / heading |
| `contradicts` | These two cannot both be current |
| `supersedes` | This is meant to replace that (low confidence unless explicit) |

Gists are **recall**, not the only graph source. Soft `depends-on` / see-also may use path + headings + gists. `constrains` / `contradicts` need the dirty heading **body** (or extracted claims) vs a **candidate set** (direct 1-hop + gist hits + term invert), not gist-vs-gist of the whole wiki.

`supersedes` without “this replaces X” (or a comment-commit why) is easy to fake. Prefer `constrains` + `contradicts`; keep `supersedes` droppable on low confidence.

Do not emit a catch-all `related` as “in context.” Cosine on gists is search, not a bind.

## Temporal graph (commits + comments)

Spatial binds answer “what else is in force with this heading.” Temporal binds answer **why it is this way**, how the design evolved, and **which accept to revert**.

Index after **every** git commit of dirty pages (same job as spatial). Node grain is still sidecar `blockId` where the hunk landed.

| Commit class | What the timeline may store | Pack as |
|---|---|---|
| **Snapshot** | SHA, dirty heading ids, optional hidden change-gist | `changed-at` — *what* moved, **not** why |
| **Comment-commit** | SHA, hunk ids, **required why**, pinned review comments (Before rail) | `decided-in`, `commented-on` — this is the reasoning |
| **Revert** (M8) | SHA that rolled back another | `reverts` → prior `decided-in` |

```text
heading H @ HEAD
    —decided-in→  comment-commit C  (why + comments)
    —changed-at→  snapshot S        (autocomment only; do not cite as rationale)
    —supersedes→  H @ parent SHA
    —constrains→  heading L         (spatial, same SHA)
```

**Ask:** “Why is freeze whole-page?” → pack H + spatial neighborhood + `decided-in` chain (whys + comments), not Notion page history as a blob and not Cursor chat.

**Rollback:** the pack names the comment-commit (or the heading clock) where the bad decision landed. Revert is still the M8 lease path (proposal vs current `T0`), not a live undo of the CRDT.

Do not put LLM change-gists in the why slot. Do not invent `decided-in` from `snapshot: <title>`.

This dimension is **AB4** in [agentic-binding.md](./agentic-binding.md#ab4--history--why-pack). It needs comment-commits (M6) before “why” is real; snapshots alone only give `changed-at`.

## Tags

LLM (or rules + LLM) on dirty files. Stored on the file row; headings may inherit. Two axes:

| Axis | Examples | For |
|---|---|---|
| **Component** | `MDGate`, `LiveSnapshot`, `lease`, `catalog`, `keck`, `host` | Which subsystem this page is about |
| **Domain** | `API`, `UI`, `architectural`, `spec`, `ops`, `security` | What kind of concern |

Tags are filters and compact-map hints. They are **not** the dependency graph. Rebuild with the gist; same skip-if-hash-unchanged rule.

## Change gist vs git message

| Artifact | Where | Snapshot (WYSIWYG) | Comment-commit |
|---|---|---|---|
| Git message | `git log` | Autocomment `snapshot: <title>` only | **Required human** why |
| Change gist | Index only | Optional hidden summary of dirty hunks | Not a substitute for accept |

Do not write an LLM sentence onto snapshot commits. That would look like a review why and collapse the two git classes.

## Store and clocks

Not Yjs. Not markdown in Postgres. Not keck. Postgres may hold **index rows** the same way HA holds `dirty` / `jobs` / `last_flushed` (Venus tables, not `jwst` blobs).

```text
last_flushed[docId]   = { clock, gitSha }     LiveSnapshot
last_indexed[docId]   = { gitSha, gistClock, incrementalCount, lastFullAt }
index job             = { wikiSha, dirtyDocIds }   after commit; upsert per wiki
```

Key gists and logical edges by `docId` + `blockId` + `gitSha`. If the sidecar id is gone on the next export, drop those edges.

If machine files land in git, they belong under `.venus/` (same family as `ids/`), in a **follow-up** that does not change `.md` bodies and does not delay `last_flushed`. Prefer serving the logical index via Venus/MCP so clone of `wiki/` stays ordinary markdown + ids. Direct graph need not be stored; parse HEAD.

Agents at lease time pack from the index **at `T0`’s SHA** (or last `last_indexed` ≤ that SHA). Do not mix live pane ranges with stored edges.

## Selection and agent pack

On a selected span (sidecar ids → containing heading):

1. Direct: containment, `links-to` / `transcludes` 1-hop, inbound reverse index
2. Logical: `depends-on` / `constrains` targets; surface `contradicts` as warnings
3. **Temporal (AB4):** `decided-in` comment-commits + rail comments for those headings; `changed-at` snapshots only as “what moved”
4. Tags / compact map for the rest if the wiki still fits

While the dogfood wiki fits in context, dump-with-a-map still wins for many agent tasks. The graphs **order and bound** that pack; they do not replace git as the share format.

Host chat that **pins a selection then expands this pack** is **AB2** ([agentic-binding.md](./agentic-binding.md#ab2--bound-chat)). The indexer does not own the thread UI.

## Do not

- Hold the pin cut, `fromDoc`, or `last_flushed` on an LLM ([HA — do not](../LiveSnapshot/high-availability.md#do-not)).
- Index the spectator splice or live Store ([pin-convert](../MDGate/pin-convert.md)).
- Treat file-level gists as enough for heading-to-heading `contradicts`.
- Rebuild every unchanged page’s logical outbound on each flush.
- Show gists, tags, or edges as published spec.
- Use Cursor subscription as a generic completions API for this job (agent harness, not chat/completions). Cheap JSON completions are a **separate** model key.
- Embed the indexer in keck.

## Files

| File | Role |
|---|---|
| [LifeIndexing.md](./LifeIndexing.md) | This design (AB1) |
| [agentic-binding.md](./agentic-binding.md) | AB1 → AB4 plan |
| [Agents README](./README.md) | Folder map |
| [LiveSnapshot README](../LiveSnapshot/README.md) | Pin then convert; where step 6 hangs |
| [high-availability.md](../LiveSnapshot/high-availability.md) | Dirty set, queue, cut; indexer is not convert |
| [datamodel git](../datamodel/git.md) | `wiki/` + `.venus/ids`; index is not live truth |
| [MDGate subset](../MDGate/subset.md) | Headings, linked-doc comments |
| [glossary](../glossary.md) | Pin, dirty set, LifeIndexing |
