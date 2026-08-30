# Markdown adapter gate (MDGate)

**Status:** `fromDoc`, [live-pane](./live-pane.md), [apply](./apply.md), [subset](./subset.md), and [fixtures](./fixtures.md) are written. **Shared exporter** is `apps/web/src/host/mdgate/from-doc.js`. **Step 3** `rt-*` goldens are in `apps/web/src/host/mdgate/goldens/`.

**M2 coding** starts at [M2/plan.md](../M2/plan.md). **M2 exit:** export fixture rows green. **M6:** apply rows green. Architecture hang-point: [architecture.md](../architecture.md#markdown-projection-add-here-before-coding-m2).

## Why a separate folder

Yjs merge of the block tree is BlockSuite’s job. Venus’s share promise is **git markdown an agent can edit**. That lives or dies in `MarkdownAdapter` plus a block-id sidecar, not in the CRDT wire ([CRDT](../CRDT/README.md)).

The [implementation plan — adapter gate](../venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it) is the **accept bar** (fixture suite). This folder is the **design**.

If we skip it: fake hunks, diffs by “paragraph 3,” opaque blocks rewritten, clone-and-apply broken.

## Two directions

| Direction | Job | Where |
|---|---|---|
| **`fromDoc`** | CRDT → `.md` + sidecar. Live pane, git flush, lease `T0` md, review `old`. | This README + [live-pane.md](./live-pane.md) |
| **Apply** | Diff **markdown vs `markdown_T0`**, attribute with **frozen sidecar**, hunks on Before/After **OctoBase** CRDTs, ops on the **published** Y.Doc. After edits after submit = next commit. | [apply.md](./apply.md) · [datamodel](../datamodel/crdt.md#commit-before-and-after) |

Still required for M2 **exit**: [fixture](./fixtures.md) export rows green on the [subset](./subset.md).

## Sidecar — what it is for

Markdown has no block ids. `fromDoc` emits ordinary text. `toDoc` / `toDocSnapshot` **mints new ids**. The sidecar is a **name map** for one export: “this span of `.md` is CRDT block `b1`.”

It does **not** convert markdown to a CRDT. Change detection is **markdown vs `markdown_T0`**; the sidecar attributes those edits to ids. The adapter parses **hunk slices** only, for op payloads ([apply.md](./apply.md)).

```text
CRDT block b1  ──fromDoc──►  "Hello"     sidecar: b1 → [0, 6) at T0

apply          ──diff md──►  what actually changed vs markdown_T0
               ──sidecar──►  that edit is still b1 (or insert in a gap)
               ──ops──►      updateBlock(b1) / addBlock / … on the existing Y.Doc
```

Linked-doc `<!-- venus:doc:… -->` is **page** identity, not this per-block map.

## Steps: CRDT → markdown

### 1. The live tree

A page is a BlockSuite tree on a Y.Doc. Ids already live here:

```text
affine:note
  b1  affine:paragraph   "Hello"
  b2  affine:paragraph   "World"
  b3  affine:paragraph   "Title"   (type: h2 on this pin)
```

Postgres (via keck) stores **Yjs update bytes**, not markdown and not sidecar rows.

### 2. One `fromDoc`

`MarkdownAdapter.fromDoc` walks the tree **in document order** and concatenates markdown. Venus records where each block landed **in that string**:

```text
markdown:

Hello

World

## Title

sidecar:

{
  "docId": "…",
  "clock": "<Yjs clock of this export>",
  "blocks": [
    { "id": "b1", "start": 0,  "end": 6  },
    { "id": "b2", "start": 7,  "end": 13 },
    { "id": "b3", "start": 14, "end": 23 }
  ]
}
```

`id` is the CRDT block id (stable while the block lives). `start` / `end` are true **only for this clock**. Exact newline / gap rules: [subset.md](./subset.md). The shape is id → range in this file, not a pointer into Yjs.

### 3. Where each copy lives

| Copy | Store | Durable? |
|---|---|---|
| Live Y.Doc | Browser `Store` + keck memory | After persist |
| Same Y.Doc persist | **Postgres** (docs + blobs) | Yes. **Not** markdown. **Not** sidecar. |
| M2 pane string + ranges | **Tab RAM**, while the pane is open | No. Next export replaces both. |
| Pin (flush / `T0`) | Venus process memory ([LiveSnapshot](../LiveSnapshot/README.md)) | Until convert finishes, or until the lease ends for `T0` |
| Published `.md` + sidecar | **Git:** `wiki/<gitPath>.md`, `wiki/.venus/ids/<docId>.json` | Yes, at a SHA. Clock = pin clock. **M3+**, not M2. |

Do not put markdown or sidecar ids in Postgres. Do not invent a second document store.

### 4. How CRDT maps to paragraphs

Each export is a **fresh walk**. The sidecar does not subscribe to the CRDT.

1. Take a Store (live tab, or a Store loaded from a **pin**).
2. Walk blocks in order.
3. Emit markdown; measure each block’s slice (plus listed whitespace).
4. Write `{ id, start, end }`.

“Paragraph 1 in the file” is not identity. Identity is `b1`. Insert a block above → new export still names the old paragraph `b1`; only offsets change.

The **disk** sidecar is for **coming back** (accept, revert). The spectator pane does not read git’s JSON.

### 5. How often `fromDoc` runs

| Path | When |
|---|---|
| **Live pane** | While the markdown view is **open**: once on open, then the [update loop](./live-pane.md) on Store changes. If the pane is closed: do not export for display. |
| **Git flush** | Idle / Flush / flush-before-lease: **pin** Yjs bytes, then `fromDoc` **on the pin**. Not per keystroke. Not the live `Store` on the hot path. |
| **Lease `T0`** | Once at acquire (keep that pin). |
| **After accept** | Full `fromDoc` on the updated tree; rewrite git sidecar. |

### 6. Live WYSIWYG (spectator)

A types in WYSIWYG. B watches markdown.

```text
A edits block b1
  → Y.Doc: b1 text changes; id still b1
  → keck broadcast; Postgres gets Yjs bytes (~1s)
  → git sidecar on disk: unchanged (last pin)

B's pane is open
  → [live-pane.md](./live-pane.md) loop
  → new markdown from the live Store
  → RAM ranges rebuilt or spliced; previous pane string discarded
```

**Old git sidecar does not map new CRDT → new markdown.** Ids never left the tree. A new export walks `b1, b2, b3` again.

The leased **editor** is the other direction (CRDT frozen, map T0 ranges through CodeMirror). That is apply, not this file.

## Incremental work (pane only)

Allowed as an optimization of the **RAM** projection, after full `fromDoc` equals the fixture subset.

In-place edit of `b1` (`"Hello"` → `"Hello there"`, `+6`):

1. Re-export **that block** (same adapter semantics).
2. Splice its slice in the markdown string.
3. Grow `b1`’s range; **shift** `start`/`end` of every later block by the delta.

Do **not** incrementally update `wiki/.venus/ids/` on every CRDT op. Git dirty unit is the **page** ([LiveSnapshot](../LiveSnapshot/README.md)).

M2 ships **full `fromDoc` + the coalescing loop**. Incremental splice is later, and only when a run is byte-equal to full `fromDoc`. Structural edits (insert/delete/move/split/merge/list indent/opaque) **fall back to full export**. Detail: [live-pane.md](./live-pane.md).

## One exporter

Pane, git flush, lease `T0` md, and review `old` slices share the same `fromDoc` + sidecar builder. Incremental pane output must match that exporter, not a second markdown dialect.

Until the fixture suite is green: no alternatives/stacks; do not tell agents “edit the `.md` in git and it will apply.” v1 agent path is lease → private buffer → hunks → accept ([apply.md](./apply.md)).

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | `fromDoc`, sidecar stores, incremental rules |
| [live-pane.md](./live-pane.md) | Spectator update loop (single-flight, full vs splice) |
| [apply.md](./apply.md) | Markdown vs `T0` → hunks → ops; WYSIWYG overlay |
| [subset.md](./subset.md) | Round-trippable types, whitespace, opaque, loss |
| [fixtures.md](./fixtures.md) | Suite layout, How to run, M2 vs M6 rows |
| [M2/plan.md](../M2/plan.md) | Implementation steps for the exporter + pane |

Product rules: [venus-design.md](../venus-design.md). Pin then convert: [LiveSnapshot](../LiveSnapshot/README.md). Words: [glossary.md](../glossary.md#four-snapshot-senses).
