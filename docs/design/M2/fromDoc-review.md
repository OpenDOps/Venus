# M2 — MDGate fromDoc / toDoc review

Markdown copy of the Cursor canvas **MDGate fromDoc / toDoc review** (re-review 2026-08-30), plus pin-then-convert which landed after that canvas pass.

| | |
|---|---|
| **Exporter** | `apps/web/src/host/mdgate/from-doc.js` |
| **RAM splice** | `splice.js` `incrementalFromDoc` |
| **Pin convert** | `pin-from-doc.js` `pinThenFromDoc` — [pin-convert.md](../MDGate/pin-convert.md) |
| **Test-only `toDoc`** | `roundTripFromDoc` on a **new** Store |
| **Security detail** | [security.md](./security.md) (S1–S7). Revisit after [M4](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks). |
| **Adapter** | BlockSuite **0.22.4** |
| **Pane loop** | Mounted: `md-pane-loop.js` + `incrementalFromDoc`; highlight.js re-paints the whole string |

| Snapshot | Meaning |
|---|---|
| P1–P6 | Exporter findings **shipped** |
| `toDoc` | Tests only on the production path |
| S2 | Pane `innerHTML` XSS sink **live** (accepted; highlight.js escapes) |
| Splice | Helper + pane loop mounted |

### Path A — type in WYSIWYG (no `T0`)

This is the flow you described. A keystroke is a CRDT mutation, not a markdown event.

```text
type in BlockSuite
  → store.spaceDoc (Y.Doc)
  → Yjs update v1 on the AFFiNE WebSocket
  → keck apply + broadcast
  → Postgres persist (batched)

same Store, this tab (if the pane is open):
  → subscribe / single-flight on Store updates
  → fromDoc or incrementalFromDoc     photograph the current tree
  → highlight.js pane
```

OctoBase never sees markdown. There is no `T1` / `Tn`. You do not “apply CRDT diffs onto markdown”; you rebuild or splice the string from the live tree. `incrementalFromDoc` is only a cheaper photograph (adapter UTF-16 slice length, not Y.Text). The pane loop is mounted (`md-pane-loop.js`).

### Path B — pin then convert (shipped)

`T0` is **not** for Path A. It is the frozen baseline when someone later edits **markdown** (lease / git) and apply must map those text diffs back onto **existing** CRDT ids. Markdown has no ids; `toDoc` of a whole file mints new ones.

That baseline is [pin-convert.md](../MDGate/pin-convert.md) `pinThenFromDoc`: copy Yjs bytes **first**, hydrate an offline Store, full `fromDoc` on that clone. Git write of `wiki/` is still M3; the convert helper is already the API M5 must call. The pane splice is not this pair and is not written to disk.

Export fixtures do not call `toDoc` on the live page. Goldens are unchanged. Remaining production risk: pane innerHTML (S2), M6 hunk `toDoc`.

---

## Performance — shipped in `from-doc.js`

Full export freezes `job.docToSnapshot`, runs one `fromDocSnapshot`, skips adapter walks for empty text paragraphs (last-N still owns the newline), and `Promise.all`s remaining slices on that snapshot. `markdownAdapterFor` caches transformer + adapter per Store. `roundTripFromDoc` still builds a **fresh** adapter for `toDoc`.

| ID | Was | Now | Status |
|---|---|---|---|
| P1 | 1× `fromDoc` + N× `fromBlockSnapshot` in series | One `fromDocSnapshot`; empty text paragraphs skip the adapter; other slices in parallel | Shipped |
| P2 | `collectRanged` / `ownMarkdown` on the live tree after awaits | Walk and per-block export use the frozen snapshot | Shipped |
| P3 | `getTransformer` + `MarkdownAdapter` every `fromDoc` | `markdownAdapterFor` WeakMap keyed by Store + workspace id / `docMetas` | Shipped |
| P4 | `indexOf(slice)` from a cursor | Match at cursor (skip stringify-gap newlines); `indexOf` only if next bytes are not the slice | Shipped |
| P5 | Copy the whole markdown string per linked-doc card | Collect insert offsets, one `join` | Shipped |
| P6 | Full export and per-block export both `readFromBlob` | Same cached adapter + assets manager after the full walk | Shipped |

Last-N empty mapping is unchanged (O(gap)). `encodeStateVector` for the sidecar clock is still cheap. Goldens unchanged. `blockMarkdownSlice` remains for RAM splice of dirty paragraphs.

---

## Incremental export — `splice.js` (Path A only)

Helper: `incrementalFromDoc(store, workspace, previous, dirtyIds)`. Cheaper Path A photograph of the **live** Store. Tests: `incr-inplace`, `incr-marks` (bold / italic / code / link), heading-type and linked-doc fallbacks, `incr-perf`. `mount-md-pane.js` still does one full `fromDoc` (step 6 loop not wired). Pin convert does not use this helper.

### Safe splice (in-place paragraph)

Dirty ids already in the sidecar, flavour paragraph, parent order unchanged, heading type unchanged vs previous `# ` slice.

1. Adapter-export that block (`blockMarkdownSlice` — same ownMarkdown + indent + `venus:doc` as full `fromDoc`).
2. `delta` = `newSlice.length − (old end − old start)` in UTF-16 — not bytes, not Y.Text length.
3. Replace `markdown[start, end)`; set `end += delta`; every row with `start ≥ old end` shifts by `delta`.
4. After splice, markdown + ranges must equal a full `fromDoc` of the same Store. Reconcile every N splices or on idle when the pane loop lands.

### Falls back to full `fromDoc`

- Insert, delete, move, split, merge — sidecar row count changes.
- Heading type `h1`↔`text` (or `h1`↔`h2`): inferred from previous slice vs `props.type`. In-place edits of an existing h1 still splice.
- Any `affine:embed-linked-doc` on the page — `titleMiddleware` can change the link label with this id not dirty.
- List indent, empty-paragraph last-N, opaque / unknown flavour, recon mismatch, `forceFull`.

### Y.Text delta is not how ids shift

| Edit | Y.Text length | Markdown length | Helper now |
|---|---|---|---|
| Insert plain characters in a paragraph | Equals typed count | Usually equal — the only case naive shift gets right | Splice; later ids move by adapter UTF-16 delta (`incr-inplace`) |
| Bold / italic / code / link | 0 (marks are attributes) | +4 or more (`**`, `` ` ``, `[ ](url)`) | Splice from adapter slice; Y.Text delta is 0 (`incr-marks`) |
| Heading type `h1`↔`text` | 0 | +2 or −2 (`# `) | Full `fromDoc`. Type mismatch vs previous `# ` slice. |
| Linked-doc title middleware | 0 on this block | Link label length changes; this id was not dirty | Full `fromDoc` if any `embed-linked-doc` is on the page |

### Why splice is not the Path B baseline (already decided)

Path A can tolerate a missed dirty id: the pane looks wrong until the next full `fromDoc`. Path B cannot: apply attributes markdown hunks to sidecar `{start,end}`. That is why convert is `pinThenFromDoc` (shipped), not “freeze whatever the pane last spliced.” The splice helper does not write disk sidecar. M5 must call the pin helper, not the pane map.

---

## Security (summary)

Detail: [security.md](./security.md). `toDoc` still runs only in Vitest (`roundTripFromDoc` on a new Store). M6 apply is hunk-slice parse, never whole-file replace. S4 export inject was tightened with the P5 rewrite.

**Revisit after M4.** One-page M2 keeps S4 apply-resolution and S5 synced-doc inlining mostly latent. [M4](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks) adds catalog, two pages, and `embed-linked-doc` round-trip; [dogfood](../venus-implementation-plan.md#v1-dogfood--document-the-product) starts then. Re-read this note and [security.md](./security.md) before treating the wiki as the product store.

| ID | Sev | M2 status | Finding |
|---|---|---|---|
| S1 | High | Latent | `toDoc` fetches `http(s)` and `data:` image URLs via `FetchUtils` with no allowlist / no `imageProxy`. SSRF and `data:` DoS when M6 hunk parse runs. |
| S2 | High | Live (accepted sink) | `mount-md-pane` sets `innerHTML` from highlight.js **11.11.1**. Highlighter escapes; still the XSS boundary. Do not patch hljs spans on splice — replace one `highlight()` result. |
| S3 | High | Mitigated for convert | Wrong-id apply if a spliced RAM sidecar is used as `T0`. Splice uses adapter UTF-16 delta + heading/linked-doc fallback. Convert is `pinThenFromDoc`. `roundTripFromDoc` must stay off the published Y.Doc. |
| S4 | Med | Partial | `pageId` must match `[A-Za-z0-9_.:-]{1,128}`. Comment injects at the adapter URL (path ends with `/${pageId}`), not `indexOf(pageId)` in prose. Apply still resolves `venus:doc:` later. |
| S5 | Med | Open | `embedSyncedDocMiddleware('content')` can inline another doc’s body into this export. One-page M2; risk if a synced-doc block is pasted. |
| S6 | Med | Latent | Hunk `toDoc` can materialize `javascript:` / `blob:` links and HTML. Apply must allowlist hrefs after parse. |
| S7 | Low | Open | Placement errors `JSON.stringify` user slices. Fine in Vitest; truncate if the pane loop surfaces them. |

---

## Still open

| Do | Layer | Why it is still open |
|---|---|---|
| Re-read S1–S7 after M4 | [implementation plan M4](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks) | Catalog + two pages make S4 comment resolution and S5 `'content'` inlining live. Do this before [dogfood](../venus-implementation-plan.md#minimum-useful-after-m4). |
| M5 lease convert must call `pinThenFromDoc`, not the pane map | M5 + [pin-convert.md](../MDGate/pin-convert.md) | Helper shipped. Wiring is later. Path A typing does not use this. |
| Allowlist image URLs on any `toDoc`; no default `fetch(http)` | M6 apply + adapter configs | S1. SSRF / `data:` DoS in `FetchUtils.fetchable`. |
| Stop inlining embed-synced-doc bodies (or strip from subset) | `createMarkdownAdapter` | S5. `'content'` middleware still set. |
| Truncate exporter throw slices; allowlist hrefs after hunk parse | Pane loop / M6 | S7 noise; S6 `javascript:` links. |
