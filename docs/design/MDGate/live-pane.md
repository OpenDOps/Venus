# Live markdown pane — update loop

**Status:** implemented in [M2 `step-loop`](../M2/plan.md#6-step-loop). Contract: [subset.md](./subset.md), [fixtures.md](./fixtures.md). Parent: [README.md](./README.md). Hang the pane on the **synced Store**, not keck export ([architecture.md](../architecture.md#markdown-projection-add-here-before-coding-m2)).

This is the **spectator**: read-only **highlighted** markdown aligned with live WYSIWYG. No caret. highlight.js paints the exporter string ([M2/plan.md](../M2/plan.md) step-pane). Not the leased CodeMirror buffer. Not git convert ([pin-convert.md](./pin-convert.md), [LiveSnapshot](../LiveSnapshot/README.md)).

## Invariant

Markdown is a **projection** of the live CRDT, not a replica. The pane **replaces** (or splices) a string in RAM. It does not parse back. It does not write Postgres or `wiki/.venus/ids/`.

```text
WYSIWYG (live Store)  ──loop──►  read-only markdown pane
                         │
                         fromDoc or in-place splice
                         RAM sidecar ranges
```

If nobody has the pane open: **do not** run the loop.

## Scheduler (M2 `step-loop` must ship this)

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

## Initial paint / fallback: full `fromDoc`

First pane fill ([M2 step-pane](../M2/plan.md#5-step-pane)) and every **fallback** in the loop:

1. `MarkdownAdapter.fromDoc` on the **live** Store (same transformer/middlewares as git/`T0`) via shared `fromDoc`.
2. Rebuild RAM `{ id, start, end }` from that string.
3. **Replace** the pane contents: run **highlight.js** on that string (`core` + markdown grammar) and set `innerHTML`. No caret to preserve. `innerText` must still equal the exporter markdown (source highlight, not a rendered preview).

Discard the previous string and previous RAM sidecar on a full run. Do not patch git’s sidecar. Do not map “old disk offsets” onto the new tree — ids are on the CRDT; a full run is a new photograph.

## Incremental body (M2 `step-loop`)

Optimization of the **same loop**, same exporter semantics. In-memory markdown + RAM ranges only. **Never** freeze this map as lease `T0` or a git pin. That convert is [pin-convert.md](./pin-convert.md).

In-place edit of an existing block (`b1` `"Hello"` → `"Hello there"`):

1. Re-export **only that block** with the same `ownMarkdown` + indent + `venus:doc` post-process as `from-doc.js`.
2. `delta` = UTF-16 `newSlice.length − (old end − old start)`. **Not** Y.Text length (bold/italic/link change markdown without changing Y.Text length).
3. Splice `[b1.start, b1.end)` in the pane string.
4. Set `b1.end += delta` (`b1.start` unchanged).
5. For every sidecar row with `start >= old b1.end`: `start += delta`, `end += delta`.

Apply dirty ids **top-down** in document order so later shifts see earlier deltas. Then highlight.js the **whole** resulting string (do not patch token spans).

### Recon gate (do not splice until true)

`from-doc.js` already has per-block `ownMarkdown`. Incremental is allowed only if that slice is **byte-equal** to the corresponding range of a full `fromDoc` ([fixtures](./fixtures.md) `incr-inplace` / `incr-marks`).

If equality fails: keep full `fromDoc` forever for the pane.

### Fall back to full `fromDoc`

Do not splice when:

| Change | Why |
|---|---|
| Insert / delete / move a block | Sidecar **rows** change, not only bounds |
| Split / merge paragraphs | Id count changes |
| Heading type `h1`↔`text` (or `h1`↔`h2`) | Y.Text length 0; markdown ± `# `. Not in-place text/marks |
| List indent / nested structure | Item markdown depends on parent |
| Empty-paragraph last-N / gaps | Newlines between ranges are not a per-block slice |
| Linked-doc title middleware | Another page can change this file’s link text without this id dirty |
| Opaque / unknown flavour | Must not invent a slice |
| Reconcile | Pane open, every N splices, or idle — must match full export |

**Correctness:** after any incremental run, markdown + RAM ranges = full `fromDoc` of the same Store (modulo [subset.md](./subset.md) whitespace). If they diverge, take the full result. The spectator must not lie relative to git flush (**one exporter**).

## What this loop is not

| Not | That path |
|---|---|
| Policy Live (caret vs remote tree ops) | Lease freeze; holder types in a **private** buffer |
| `fromDoc` on the live Store for **git** | Pin bytes, convert the pin ([pin-convert.md](./pin-convert.md)) |
| Updating disk sidecar every CRDT op | Rewrite `.venus/ids/` only on pin / accept |
| Markdown → CRDT | [apply.md](./apply.md): md vs `T0` → hunks → ops |

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | `fromDoc`, stores, incremental rules in brief |
| [live-pane.md](./live-pane.md) | This loop |
| [M2/plan.md](../M2/plan.md#6-step-loop) | `step-loop` work + `incr-*` scenarios |
| [apply.md](./apply.md) | Markdown vs `T0` → hunks → ops |
| [pin-convert.md](./pin-convert.md) | Pin then convert (not this loop) |
| [subset.md](./subset.md) | Types and whitespace |
| [fixtures.md](./fixtures.md) | How to run / cases |

Accept bar: [implementation plan — adapter gate](../venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it).
