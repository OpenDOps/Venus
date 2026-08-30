# M2 — Markdown projection security

Static review of the MDGate exporter (`fromDoc` / test-only `toDoc`), the read-only pane, and the designed apply path. Source: BlockSuite **0.22.4** adapter + Venus host on 2026-08-30. Findings IDs match the MDGate review canvas. Folder copy of that canvas (perf, splice, security summary, still-open): [fromDoc-review.md](./fromDoc-review.md).

**This is not an apply implementation.** M6 hunk parse is specified as slice `toDoc`, never whole-file replace of the published Y.Doc. `roundTripFromDoc` is the only `toDoc` caller and must stay on a **new** Store.

Related exporter performance work (P1–P6) landed in `apps/web/src/host/mdgate/from-doc.js` in the same change set; it is summarized at the end so this note stays the security record.

## Status by finding

| ID | Sev | Status in M2 | When it becomes live |
|---|---|---|---|
| [S1](#s1--todoc-fetches-image-urls) | High | **Latent.** No allowlist. `toDoc` is tests-only. | M6 hunk `toDoc` / clone import |
| [S2](#s2--pane-innerhtml-is-the-xss-boundary) | High | **Live** on the pane (`mount-md-pane.js`). highlight.js escapes; still the sink. | Already (step-pane) |
| [S3](#s3--wrong-id-apply) | High | **Mitigated for convert.** `pinThenFromDoc` pins Yjs bytes then full `fromDoc` on an offline clone. Splice stays pane-only. | M3 git write / M5 lease still must call this helper, not the pane map |
| [S4](#s4--venusdoc-comment-injection) | Med | **Partial.** pageId allowlist + inject at adapter URL, not `indexOf(pageId)`. | Export path now; apply still resolves comments |
| [S5](#s5--embedsynceddocmiddleware-inlines-other-docs) | Med | **Open.** Middleware still `'content'`. | Any export that transcludes |
| [S6](#s6--hunk-todoc-materializes-javascript--html-links) | Med | **Latent.** | M6 parse of `hunk.new` |
| [S7](#s7--placement-errors-stringify-user-slices) | Low | **Open.** Fine in Vitest. | Pane loop if it surfaces exporter throws |

## Invariants (do not weaken)

1. **Never freeze a spliced RAM sidecar as lease `T0` or a git pin.** Convert is `pinThenFromDoc` (Yjs bytes → offline Store → full `fromDoc`). Git file write is M3 ([live-pane](../MDGate/live-pane.md), [LiveSnapshot](../LiveSnapshot/README.md)).
2. **Never `Y.applyUpdate` of a parsed doc.** Apply is markdown vs `markdown_T0` + sidecar attribution ([apply.md](../MDGate/apply.md)).
3. **`roundTripFromDoc` stays off the published Y.Doc.** It `toDoc`s into a new Store for goldens only.
4. **Pane paint is one `highlight()` result.** Do not patch highlight.js spans on splice; re-highlight the whole string ([live-pane](../MDGate/live-pane.md)).
5. **No ids in the markdown body.** Sidecar is RAM in M2. Linked-doc `<!-- venus:doc:… -->` is **page** identity, not a per-block map.

---

### S1 — `toDoc` fetches image URLs

**Path:** Affine `processImageNodeToBlock` → `FetchUtils.fetchImage` / `fetchable` for `http(s)` and `data:`. Venus does not set `imageProxy` middleware.

**Impact:** Agent markdown `![](http://169.254.169.254/…)` or a huge `data:` URL runs in the browser (or Node) that parses the hunk. SSRF and memory DoS.

**M2:** Export fixtures and the pane do not call `toDoc` on the live page. Goldens do not exercise this fetch.

**Mitigation (M6):** Allowlist image URLs on any `toDoc` (hunk or import). Do not default to `fetch(http)`. Prefer same-origin blob ids already in the Store; reject or proxy the rest. Cap `data:` size.

### S2 — Pane `innerHTML` is the XSS boundary

**Path:** `mount-md-pane.js` sets `code.innerHTML = highlight(markdown)`. `highlight-md.js` uses highlight.js **11.11.1** `lib/core` + markdown grammar only. `hljs.highlight` HTML-escapes source and wraps tokens in spans.

**Impact:** If highlight.js ever fails to escape, or if a later change concatenates unsanitized markdown into HTML, the spectator pane becomes a script sink. Incremental splice that patches HTML spans is worse than replacing one escaped `highlight()` string.

**M2:** Live. `contenteditable` is `"false"`. Do not introduce a markdown-it / marked preview.

**Mitigation:** Keep replacing the **whole** highlighted HTML after each export or splice. Treat highlight.js as the sanitizer; do not add a second HTML pipeline. If the pane loop lands, paint `highlight(splicedMarkdown)`, never surgically edit `span` nodes.

### S3 — Wrong-id apply

**Path:** Sidecar ranges bind UTF-16 slices to CRDT ids. `indexOf` mis-placement, or using a **spliced** RAM map as `markdown_T0`, attributes a hunk to the wrong node. `updateBlock` / `deleteBlock` then mutates the wrong block. Whole-file `toDoc` still **mints new ids** (`ap-no-replace`).

**Impact:** Published CRDT diverges from git. Same class of bug whole-file `toDoc` was forbidden for.

**M2:** Placement now prefers cursor-aligned slices from a **frozen** snapshot (see P2/P4). Splice is RAM-only (`incrementalFromDoc`). Path B convert: [pin-convert.md](../MDGate/pin-convert.md). `roundTripFromDoc` remains the only `toDoc` caller.

**Mitigation:** Git flush and lease `T0` call `pinThenFromDoc` / `fromPinnedBytes`, never `fromDoc` of the live Store and never the pane sidecar. After splice, pane markdown + ranges must equal a full `fromDoc` of the live Store (reconcile) — that is Path A only.

### S4 — `venus:doc` comment injection

**Path:** `injectVenusLinkedDocComments` / `withVenusLinkedDocComment`. A `pageId` containing `-->` breaks the HTML comment. Searching `indexOf(pageId)` can insert the comment at the first substring match (short or colliding ids, or prose that mentions the id).

**Impact:** Comment breakout in the export; false injection so the sidecar range and later apply resolve the wrong span.

**M2 (partial):** `pageId` must match `^[A-Za-z0-9_.:-]{1,128}$` or the comment is not written. Injection finds a markdown link whose URL is `pageId` or ends with `/${pageId}`, not a raw substring. Covered by `from-doc.test.ts` (prose `doc:lease` then a card).

**Mitigation (apply / M4 catalog):** Resolve `venus:doc:` first, path second. Do not parse comments with an unsanitized id. Recreating the card from the comment is M6, not adapter `toDoc`.

### S5 — `embedSyncedDocMiddleware('content')`

**Path:** `createMarkdownAdapter` in `markdown-adapter.js`. `'content'` inlines transcluded doc bodies into this page’s markdown.

**Impact:** A snapshot or pane refresh can copy another page’s content into git and the spectator. Confidentiality / unexpected export size.

**M2:** One page (`doc:home`). Linked-doc **cards** are `affine:embed-linked-doc` (title + URL + Venus comment), not synced-doc transclusion. Risk is if a synced-doc block is pasted into the note.

**Mitigation:** Switch to a middleware mode that does not inline foreign bodies for git/`T0`, or strip `affine:embed-synced-doc` from the export subset. Document remainder in [subset.md](../MDGate/subset.md) if the adapter still emits a stub.

### S6 — Hunk `toDoc` materializes `javascript:` / HTML links

**Path:** remark + Affine inline matchers. `toDoc` of `hunk.new` can put `javascript:`, `blob:`, or HTML into block props. WYSIWYG then renders them.

**Impact:** XSS in the editor after apply, not only in the markdown pane.

**M2:** Not hit. Goldens use `https://example.com/path`.

**Mitigation (M6):** Allowlist `href` after parse (`https:` / relative wiki paths). Drop or neutralize other schemes. Do not trust adapter props as already safe.

### S7 — Placement errors stringify user slices

**Path:** `placePageTitle` / `placeFromCursor` / `assignEmptyParagraphs` throw `JSON.stringify` of markdown slices.

**Impact:** Fine in Vitest. If the pane loop logs or displays those messages, user content leaks into consoles / error reporters.

**Mitigation:** Truncate slices in thrown messages; do not send exporter errors to a multi-tenant log without redaction.

---

## What M2 will not fix in code

S1, S5, S6, S7 stay documented until apply (M6) or a dedicated adapter-config change. S2 is accepted as the pane sink with the highlight.js contract. S3 is a pin/`T0` policy. S4 is tightened on export only.

## Exporter performance (P1–P6) — same change set

These were review findings on `fromDoc`, not CVEs. Shipped so the pane loop is not N+1 on every coalesced refresh:

| ID | Was | Now |
|---|---|---|
| P1 | 1× `fromDoc` + N× `fromBlockSnapshot` in series | One `fromDocSnapshot`; empty text paragraphs skip the adapter; remaining slices `Promise.all` |
| P2 | `collectRanged` / `ownMarkdown` on the live tree after awaits | `job.docToSnapshot` first; walk and per-block export use that snapshot |
| P3 | `getTransformer` + `MarkdownAdapter` every call | `markdownAdapterFor` WeakMap per Store (fresh adapter for `toDoc` in `roundTripFromDoc`) |
| P4 | `indexOf(slice)` from a cursor | Match at cursor (skip stringify-gap `\n`); `indexOf` only if the next bytes are not the slice |
| P5 | Copy the whole markdown string per linked-doc card | Collect insert offsets, one `join` |
| P6 | Full export and per-block export both `readFromBlob` | Same cached adapter + assets manager after the full walk |

`blockMarkdownSlice` remains for RAM splice of dirty paragraphs. Git / `T0` convert is [pin-convert.md](../MDGate/pin-convert.md), not live `fromDoc` and not splice.
