# Git storage

**Status:** design. Map: [README.md](./README.md). CRDT side: [crdt.md](./crdt.md). Convert path: [LiveSnapshot](../LiveSnapshot/README.md). Sidecar / `fromDoc`: [MDGate](../MDGate/README.md). Two commit classes: [venus-design.md](../venus-design.md#apply-and-git).

Git is the **share and history** format: humans and agents clone folders of markdown. It is not the live page and not a second CRDT.

Venus is the only writer of published commits in v1. Clones do not `git push` into the live tree. Later: attach a clone PR as a review commit (same apply path).

## Working tree

```text
wiki/
  spec/
    protocol.md
    crdt/
      lease.md
  design/
    overview.md
  assets/                      ← blobs referenced by pages (dirty only on flush)
  .venus/
    ids/
      <docId>.json             ← block-id sidecar; clock = pin / accept clock
    snapshots/                 ← optional M8: <docId>/<gitSha>.bin (Yjs bytes)
```

| Path | What | Source |
|---|---|---|
| `<gitPath>.md` | Projection of one published page | `fromDoc` on a **pin** (snapshot) or on published after comment-commit **accept** |
| `.venus/ids/<docId>.json` | `{ docId, clock, blocks: [{ id, start, end }] }` | Same export. Ranges rebuilt every write; ids are CRDT ids |
| `assets/` | Image (and other) bytes | keck blob store → files on flush if dirty |
| directories | Folder **nesting** | Catalog `gitPath`; `git mv` on publish when path changed |
| git commit message | Why (or autocomment) | Snapshot vs comment-commit, below |

Clone of `wiki/` is ordinary markdown. Sibling **order** lives on the [catalog CRDT](./crdt.md#catalog), not in git. Empty catalog folders are not in git unless a placeholder is added later.

Do **not** commit keck/Postgres dumps as the share format. Optional `.venus/snapshots/*.bin` is a restore aid, not what agents edit.

## Two commit classes

Do not mix them. Pin **then** convert; do not `fromDoc` the live Store on the hot path ([LiveSnapshot](../LiveSnapshot/README.md)).

| Class | When | Message | Input |
|---|---|---|---|
| **Snapshot** | Idle 30–120s, Flush, flush-before-lease | Autocomment `snapshot: <title>` | Pin of dirty **published** pages + catalog `git mv` |
| **Comment-commit** | Lease **accept** of a review sequence | **Required** why | After ops on published; then same `fromDoc` + sidecar |

Coalesce WYSIWYG: all typing since last SHA is **one** snapshot, not one commit per keystroke.

Review After-space typing is **not** a snapshot. It is the next comment-commit in the [sequence](./crdt.md#commit-before-and-after) (or a draft regenerate before submit).

## Sidecar on disk

Written only on pin convert / accept — not on every CRDT keystroke, not by the live markdown pane ([live-pane.md](../MDGate/live-pane.md)).

```json
{
  "docId": "8f3a…",
  "clock": "<pin or post-accept clock>",
  "blocks": [
    { "id": "b1", "start": 0, "end": 12 }
  ]
}
```

Used when markdown must **come back** ([apply.md](../MDGate/apply.md)): diff vs `markdown_T0`, attribute with this map. Git sidecar at HEAD should match flush-before-lease `T0`.

## What git does not store

| Not in git (as live truth) | Where it lives |
|---|---|
| Live Y.Doc / merge | [CRDT published](./crdt.md#published-page) → Postgres |
| Lease, threads, hunk **records** | [Review session](./crdt.md#review-session) |
| Before/After proposal trees (while in flight) | [Commit spaces](./crdt.md#commit-before-and-after) |
| Catalog sibling order | Catalog CRDT |
| Agent private markdown buffer | Holder RAM until submit |

After accept, git has the new `.md` + sidecar; After/Before spaces remain **archives** in OctoBase.

## Revert

`git log` / `git show` is the user-facing history. Revert is a **lease** + comment-commit: historical `.md` as proposal vs current `T0` ([apply.md](../MDGate/apply.md#revert--clone-later)). Do not replay published Yjs undo as the version log.
