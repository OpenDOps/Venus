# Apply — markdown → CRDT on commit

**Status:** design. Implement in [M6](../venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks). `fromDoc` / sidecar: [README.md](./README.md). Spectator pane: [live-pane.md](./live-pane.md). Product hunks / After / Before / Diff: [venus-design.md](../venus-design.md#comment-commits-markdown-only). Freeze: [lease-freeze-rationale.md](../lease-freeze-rationale.md).

Do **not** detect changes by converting the whole proposed file to a new block tree. That is adapter parse as the diff, and it produces fake hunks. Detect changes as a **markdown-vs-markdown** diff against `T0`, attribute them with the **frozen sidecar**, overlay those hunks on the **Before/After OctoBase CRDTs** (Cursor-style), then apply **ops** on the published Y.Doc. After/Before persistence: [datamodel — commit Before/After](../datamodel/crdt.md#commit-before-and-after).

## Do not rebuild the CRDT

`MarkdownAdapter.toDoc` / `toDocSnapshot` on the full buffer **mints new ids**. Replacing the published Y.Doc with that snapshot (or `Y.applyUpdate` of it) drops `b1`. Comments, opaque blocks, and later re-export all break.

Apply is always: **same Y.Doc, BlockSuite ops keyed by T0 ids.**

## `T0` (the frozen base)

At lease acquire (flush-before-lease so this matches git HEAD):

```text
pin Yjs bytes
fromDoc on that pin  →  markdown_T0 + sidecar_T0
freeze published Store
holder buffer = copy of markdown_T0
```

`T0` is those three kept together until release. Review Before / Diff is vs this clock. The sidecar offsets are true **only** for `markdown_T0`.

## Change detection: markdown vs markdown

```text
markdown_T0     (frozen export)
markdown_prop   (holder / agent buffer)
        │
        ▼
text diff (line/byte, same idea as git / Cursor)
        │
        ▼
attribute each change onto sidecar_T0 ranges
        │
        ▼
hunks[]   { blockId?, afterBlockId?, kind, old, new, diff }
```

The sidecar is a range map on **`markdown_T0`**, not a parse of the proposal:

| Text diff lands… | Hunk |
|---|---|
| Inside `b1`’s `[start, end)` | `modify` `blockId=b1` |
| In a **gap** between `b1` and `b2` (or before first / after last) | `insert` `blockId=null` `afterBlockId=b1` (or start of note) |
| Covers all of `b2`’s range, nothing left | `delete` `blockId=b2` |
| `b3`’s slice removed here and inserted there (same text) | `move` `blockId=b3` |
| No overlap with any range that changed | **no hunk** — including opaque blocks |

A git-style hunk that **spans two** sidecar ranges is **split** at the range boundary. One Venus hunk ≈ one block (or one insert). That is what you highlight on the page.

If `markdown_prop === markdown_T0` (modulo listed whitespace in [subset.md](./subset.md)): **zero hunks**. Whole-file parse would still “change” blocks. That is why the diff is on markdown.

While the holder types, the RAM map may track ranges through CodeMirror (same shift idea as [live-pane.md](./live-pane.md)). At submit, you can also recompute: diff current buffer vs `markdown_T0` + original `sidecar_T0`. Both must agree. The frozen sidecar is the attribution source of truth, not a re-export of the dirty buffer.

## Parse only the changed slices

After hunks exist, **then** run the adapter on **`hunk.new` / `hunk.old` slices** (scratch note + that markdown), to get BlockSuite props for `updateBlock` / `addBlock`. Never parse the whole proposal as “the new CRDT.”

Untouched ids: apply is a **no-op**. Opaque regions the writer did not touch must not be rewritten.

## Map markdown diff → CRDT diff (review UI)

Hunks are the same object in every view (Cursor: each diff is a card). **Before** and **After** are **OctoBase** BlockSuite docs, not a scratch Store in one tab ([datamodel](../datamodel/crdt.md#commit-before-and-after)).

```text
Before   T0 (or parent After) CRDT, readonly space. Overlay hunk.old (red).
After    clone of Before + hunk ops, **synced space**. Overlay hunk.new (green).
         Same block ids as published. Humans may type here.
Diff     Before + After + hunk cards. Why on the comment rail.
```

Materialize After **once hunks exist**: clone T0 bytes into `afterDocId`, apply hunk ops, persist via keck/Postgres. Do not keep After only in RAM.

Edits on After **after submit** are not `updateBlock` on this commit’s hunks. They are commit `B` with `parentCommitId = A` (sequence). Draft regenerate (same id, `generation++`) is only before first submit.

Humans review **on the WYSIWYG After/Before pages**. v1: accept the visible tip (composed sequence) or reject. Do not put hunk cards into the published schema.

## Apply on accept

```text
for commit in sequence (root → tip):
  for hunk in commit.hunks:
    modify  → updateBlock(blockId, props from parse(hunk.new))
    delete  → deleteBlock(blockId)
    insert  → addBlock(..., after afterBlockId)   // new CRDT id is born here
    move    → moveBlock(blockId, ...)
then fromDoc the published tree → git .md + new sidecar (new ranges, same ids)
archive Before/After spaces readonly
release lease
```

(Equivalent: id-diff published `T0` vs tip **After**, because After was cloned from `T0` with the same ids.)

Published CRDT and git move together. Rollback of a **proposal** drops or rejects that commit (and descendants); the published Y.Doc never moved. Archive Before/After spaces as readonly.

## Revert / clone (later)

Same pipeline: historical `.md` is `markdown_prop`; current pin is `T0`. Diff files, attribute with current `sidecar_T0` (or the sidecar committed next to that SHA when restoring that snapshot). Still ops, not a new Y.Doc.

## Fixture bar (with [README.md](./README.md))

- Markdown-only whitespace that `fromDoc` would also emit is **not** a hunk.
- Insert-before is a hunk with `afterBlockId`, not “paragraph 3 is now paragraph 4.”
- Untouched opaque block: no hunk, apply no-op.
- Re-export after apply keeps ids the writer did not delete.

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | `fromDoc`, sidecar stores |
| [live-pane.md](./live-pane.md) | Spectator loop (CRDT → md) |
| [apply.md](./apply.md) | This: md vs `T0` → hunks → ops; After/Before are OctoBase CRDTs |
| [subset.md](./subset.md) | Types, whitespace, opaque, loss |
| [fixtures.md](./fixtures.md) | M2 export / M6 apply cases |
