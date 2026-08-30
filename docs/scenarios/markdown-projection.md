# Markdown projection

**Feature:** a read-only markdown **source** pane that photographs the live BlockSuite Store. Not git, not keck `GET …/export`, not `T0`. [MDGate](../design/MDGate/README.md). [M2 plan](../design/M2/plan.md).

**Boxes:** Host pane, Store `fromDoc` ([architecture](../design/architecture.md#markdown-projection-add-here-before-coding-m2)).

## Run

```bash
pnpm test                 # mdgate Vitest (no Docker)
pnpm test:e2e             # e2e/m2-pane.spec.ts + m0-*.spec.ts (memory Vite)
```

Manual: [runbook — Manual testing (M2)](../runbook.md#manual-testing-m2-close-out). Fixture table: [fixtures.md](../design/MDGate/fixtures.md).

Pane hangs off `session.store`, same as the editor. Do not use `/api/block/…/export` for markdown.

## Vitest (`apps/web/src/host/mdgate/`)

| Fixture id | Spec | Proves |
|---|---|---|
| seed `fromDoc` | `from-doc.test.ts` | Seed golden `goldens/seed.fromDoc.md`; `toDoc` does not throw |
| `rt-paragraph` | `roundtrip.test.ts` | `fromDoc → toDoc → fromDoc` equals `goldens/rt-paragraph.md` |
| `rt-headings` | `roundtrip.test.ts` | h1/h2 types survive; `goldens/rt-headings.md` |
| `rt-list` | `roundtrip.test.ts` | Nested bullets GFM `*`; one sidecar row per item |
| `rt-code` | `roundtrip.test.ts` | Fenced `javascript` survives |
| `rt-link` | `roundtrip.test.ts` | `[docs](https://example.com/path)` |
| `rt-linked-doc` | `roundtrip.test.ts` | Export `<!-- venus:doc:… -->`; `toDoc` does not restore the card |
| `rt-marks` | `roundtrip.test.ts` | bold / italic / inline code golden |
| `side-ids` | `from-doc.test.ts` | Three paragraphs; ranges map CRDT ids; no `<!-- id -->` in the body |
| `side-stable` | `sidecar.test.ts` | Two `fromDoc`s, no mutation → same ids and ranges |
| `side-shift` | `sidecar.test.ts` | Insert above keeps `b1`; `start` moved |
| `opaque-untouched` | `sidecar.test.ts` | `affine:image` slice byte-equal; `goldens/opaque-image.md` |
| `loss-color` | `sidecar.test.ts` | Color stays on CRDT; markdown is plain; `goldens/loss-color.md` |
| `incr-inplace` | `splice.test.ts` | Splice equals full `fromDoc`; later ranges shift by UTF-16 delta |
| `incr-marks` | `splice.test.ts` | Bold/italic/code/link: Y.Text length 0, markdown grows |
| `incr-fallback` | `splice.test.ts` | Insert / heading type / linked-doc / empty last-N → full export |
| `incr-perf` | `splice.test.ts` | Splice median faster than `forceFull` |
| `pin-then-fromDoc` | `pin-from-doc.test.ts` | Pin Yjs bytes; live edit is not on that pin convert |
| `one-exporter` | `one-exporter.test.ts` | `mount-md-pane.js` imports `./from-doc.js`; `from-doc.js` has no `highlight.js`; `mount-editor.js` has no adapter |
| coalesce | `md-pane-loop.test.ts` | 20 events during a slow export → at most two runs |

No `ap-*` tests in M2. Apply is [M6](../design/venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks).

## Playwright (`pnpm test:e2e`)

| Fixture id | Spec | Proves |
|---|---|---|
| pane visible | `e2e/m2-pane.spec.ts` | `[data-testid="venus-md-pane"]` has seed H1; not `contenteditable` |
| source highlighted | `e2e/m2-pane.spec.ts` | `hljs-` span; `# Why Venus` stays in `innerText` |
| `e2e-pane` | `e2e/m2-pane.spec.ts` | Type `m2-hello` in the note; pane `innerText` includes it within 5s; highlight remains |
| pane equals helper | `e2e/m2-pane.spec.ts` | Pane `innerText` === `fromDoc(store).markdown` (e2e-only `__VENUS_FROM_DOC__`) |

Existing `e2e/m0-*.spec.ts` still pass with the pane in the layout.

## Out of this group

| Later | Where |
|---|---|
| keck Yjs export | [doc-export](./doc-export.md) |
| Git `wiki/` | [M3](../design/venus-implementation-plan.md#m3--git-snapshotter-week) |
| Apply hunks | [fixtures](../design/MDGate/fixtures.md) `ap-*` (M6) |
