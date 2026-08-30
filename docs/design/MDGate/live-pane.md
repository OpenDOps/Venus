# Live markdown pane — update loop

**Status:** design. Implement in [M2/plan.md](../M2/plan.md). Contract: [subset.md](./subset.md), [fixtures.md](./fixtures.md). Parent: [README.md](./README.md). Hang the pane on the **synced Store**, not keck export ([architecture.md](../architecture.md#markdown-projection-add-here-before-coding-m2)).

This is the **spectator**: read-only **highlighted** markdown aligned with live WYSIWYG. No caret. highlight.js paints the exporter string ([M2/plan.md](../M2/plan.md) step-pane). Not the leased CodeMirror buffer. Not git convert ([LiveSnapshot](../LiveSnapshot/README.md)).

## Invariant

Markdown is a **projection** of the live CRDT, not a replica. The pane **replaces** (or splices) a string in RAM. It does not parse back. It does not write Postgres or `wiki/.venus/ids/`.

```text
WYSIWYG (live Store)  ──loop──►  read-only markdown pane
                         │
                         fromDoc (+ optional splice)
                         RAM sidecar ranges
```

If nobody has the pane open: **do not** run the loop.

## Scheduler (M2 must ship this)

Do **not** start one `fromDoc` job per Yjs event. Use **single-flight + dirty**: if a run is slow, collapse everything that arrived during it into one follow-up.

```text
dirty = false
running = false
dirtyIds = ∅        // block ids touched since last take

on Store / Y.Doc update (local or remote):
  add this transaction’s block ids to dirtyIds
  dirty = true
  if !running: loop()

loop:
  running = true
  ids = dirtyIds; dirtyIds = ∅; dirty = false
  export (full fromDoc, or incremental — see below)
  paint with highlight.js (markdown grammar) → set pane innerHTML
  running = false
  if dirty: loop()    // pick up changes that landed while we computed
```

Optional: yield to `requestAnimationFrame` / idle; if `elapsed > budget`, skip a frame but keep `dirty` so the next loop still runs. Lag the pane; do not queue N exports.

One transaction can touch several blocks (paste, undo). The loop always takes a **set** of ids, not “the last keystroke.”

## M2 body: full `fromDoc`

Each loop iteration:

1. `MarkdownAdapter.fromDoc` on the **live** Store (same transformer/middlewares as git/`T0`).
2. Rebuild RAM `{ id, start, end }` from that string.
3. **Replace** the pane contents: run **highlight.js** on that string (`core` + markdown grammar) and set `innerHTML`. No caret to preserve. `innerText` must still equal the `fromDoc` markdown (source highlight, not a rendered preview).

Discard the previous string and previous RAM sidecar. Do not patch git’s sidecar. Do not map “old disk offsets” onto the new tree — ids are on the CRDT; this is a new photograph.

That is enough for M2 exit: WYSIWYG and markdown stay aligned on one client; pane is replaceable.

## Incremental body (after fixtures)

Optimization of the **same loop**, same exporter semantics. In-memory markdown + RAM ranges only.

In-place edit of an existing block (`b1` `"Hello"` → `"Hello there"`, delta `+6`):

1. Re-export **only that block** (adapter slice or a proven equivalent; see recon).
2. Splice `[b1.start, b1.end)` in the pane string.
3. Set `b1.end += delta` (and `b1.start` unchanged).
4. For every sidecar row with `start >= old b1.end`: `start += delta`, `end += delta`.

Apply dirty ids **top-down** in document order so later shifts see earlier deltas.

### Recon gate (do not splice until true)

Public API today is `fromDoc` on a Store ([api-map.md](../api-map.md) — adapter Actuals still empty). Incremental is allowed only if:

- recon finds a per-block export, **or**
- a scratch Store of one note + that block is **byte-equal** to the corresponding slice of a full `fromDoc`.

If equality fails: keep full `fromDoc` forever for the pane.

### Fall back to full `fromDoc`

Do not splice when:

| Change | Why |
|---|---|
| Insert / delete / move a block | Sidecar **rows** change, not only bounds |
| Split / merge paragraphs | Id count changes |
| List indent / nested structure | Item markdown depends on parent |
| Gaps / blank lines between blocks | Whitespace may sit **between** ranges |
| Linked-doc title middleware | Another page can change this file’s link text |
| Opaque / unknown flavour | Must not invent a slice |
| Reconcile | Pane open, every N splices, or idle — must match full export |

**Correctness:** after any incremental run, markdown + RAM ranges = full `fromDoc` of the same Store (modulo [subset.md](./subset.md) whitespace). If they diverge, the spectator lies relative to git flush (**one exporter**).

## What this loop is not

| Not | That path |
|---|---|
| Policy Live (caret vs remote tree ops) | Lease freeze; holder types in a **private** buffer |
| `fromDoc` on the live Store for **git** | Pin bytes, convert the pin ([LiveSnapshot](../LiveSnapshot/README.md)) |
| Updating disk sidecar every CRDT op | Rewrite `.venus/ids/` only on pin / accept |
| Markdown → CRDT | [apply.md](./apply.md): md vs `T0` → hunks → ops |

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | `fromDoc`, stores, incremental rules in brief |
| [live-pane.md](./live-pane.md) | This loop |
| [apply.md](./apply.md) | Markdown vs `T0` → hunks → ops |
| [subset.md](./subset.md) | Types and whitespace |
| [fixtures.md](./fixtures.md) | How to run / cases |

Accept bar: [implementation plan — adapter gate](../venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it).
