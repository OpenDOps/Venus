# Fixture suite

**Status:** design. Types and loss: [subset.md](./subset.md). Exporter: [README.md](./README.md). Pane: [live-pane.md](./live-pane.md). Apply: [apply.md](./apply.md). Accept bar: [implementation plan](../venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it). Coding steps: [M2/plan.md](../M2/plan.md).

Not a demo. M2 **exit** is the **export** rows green. M6 **exit** (and “edit the `.md` and it will apply”) needs the **apply** rows green.

## How to run

From repo root. Adapter tests are **Node Vitest** (build a `Store`, no keck). Pane alignment is **Playwright** (one client, memory or octobase).

```bash
pnpm test                 # includes mdgate Vitest (path below)
pnpm test:e2e             # add e2e/m2-*.spec.ts when the pane exists
```

| Layer | Files (create in M2/M6) | Needs |
|---|---|---|
| Export / sidecar / opaque | `apps/web/src/host/mdgate/*.test.ts` | nothing |
| Pane aligned, replaceable | `apps/web/e2e/m2-pane.spec.ts` | Vite; sync optional |
| Apply hunks | `apps/web/src/host/mdgate/apply.test.ts` | nothing (in-memory T0 Store) |

Goldens: `apps/web/src/host/mdgate/goldens/` — checked-in `fromDoc` bytes for named cases. First recon run writes them; after that, diffs are failures.

Do **not** require Compose for the subset round-trip. One exporter: the same `fromDoc` helper the pane and (later) pin convert will call.

## M2 — export (must be green to close M2)

| Id | Proves | Setup | Assert |
|---|---|---|---|
| `rt-paragraph` | Stable subset | One `affine:paragraph` | `fromDoc → toDoc → fromDoc` equals golden modulo [whitespace](./subset.md#whitespace-rules) |
| `rt-headings` | Headings as `paragraph` + `type` h1/h2 | Seed-like h1, body, h2 | Same; outline types still h1/h2 in the Store |
| `rt-list` | Lists | Bulleted + one nested item | Byte-stable; sidecar has an id per listed rule. **Actual** bullets are GFM `*` (not `-`) |
| `rt-code` | Fenced code | `affine:code` with language | Fences survive |
| `rt-link` | Inline link | Paragraph with a URL mark | Link in markdown; round-trip |
| `rt-linked-doc` | Linked-doc form | `affine:embed-linked-doc` | Export: adapter URL + `<!-- venus:doc:… -->`. `toDoc` does **not** restore the card ([subset](./subset.md#linked-doc-export-vs-todoc)) |
| `rt-marks` | Inline marks | Bold, italic, inline code | Round-trip golden `**bold** *italic* \`code\`` |
| `side-ids` | Sidecar ids | Three paragraphs `b1,b2,b3` | Every range maps `id` → slice; slices concatenate with gaps to the file; **no** ids in the markdown body |
| `side-stable` | Re-export keeps ids | `fromDoc`, mutate nothing, `fromDoc` again | Same ids, ranges may follow whitespace rule only |
| `side-shift` | Insert above does not rename | Export; `addBlock` **before** `b1`; export | `b1` still `b1`; its `start` moved |
| `opaque-untouched` | Opaque not rewritten | Subset block + opaque (image or unknown); `fromDoc` twice | Opaque slice **byte-equal**; subset still round-trips |
| `loss-color` | Loss documented | Paragraph with color mark if the schema allows | `fromDoc` golden is plain (or listed exception); second `fromDoc` stable |
| `one-exporter` | Same helper | Call the shared `fromDoc` used by tests | Pane code imports that helper (static check or same module) |
| `e2e-pane` | Live alignment | Type in WYSIWYG; markdown pane open | Pane text equals `fromDoc(store)` after debounce; **no caret**; replace whole string |

Whitespace-only `fromDoc` jitter that is **not** on the exemption list fails `rt-*`. That is the fake-hunk bug.

## M6 — apply (must be green before agents apply `.md`)

Build `markdown_T0` + `sidecar_T0` from a Store. Edit a **copy** of the markdown. Diff + attribute ([apply.md](./apply.md)). Apply ops to a **clone** of the T0 Store. Then `fromDoc`.

| Id | Proves | Edit | Assert |
|---|---|---|---|
| `ap-noop-ws` | Whitespace is not a hunk | Change only exempt whitespace, including extra/fewer `\n` in a **gap** next to an empty paragraph | **Zero** hunks; Store ids and text unchanged (seed spacers still there) |
| `ap-modify` | Diff inside a range | Change text inside `b1` | One `modify` `blockId=b1`; `b2` untouched |
| `ap-insert-before` | Gap insert | Insert a **non-empty** paragraph **between** `b1` and `b2` (not blank lines only) | `insert` `afterBlockId=b1`; `b2` still `b2` (not “paragraph 3”) |
| `ap-delete` | Range removed | Delete `b2`’s slice | `delete` `blockId=b2`; `b1`/`b3` remain |
| `ap-opaque-noop` | Untouched opaque | Edit only a subset paragraph; opaque present | No hunk on opaque id; opaque bytes on re-export match T0 opaque slice |
| `ap-reexport-ids` | Ids survive apply | `ap-modify` then `fromDoc` | `b1` still `b1` |
| `ap-no-replace` | No whole-doc apply | — | Tests call `updateBlock` / `addBlock` / `deleteBlock`; never `Y.applyUpdate` of `toDoc` |

## Out of this suite

| Later | Why |
|---|---|
| Incremental pane splice = full `fromDoc` | After M2 goldens exist ([live-pane.md](./live-pane.md)) |
| Git pin convert | [LiveSnapshot](../LiveSnapshot/README.md) M3; same `fromDoc` helper |
| Two-tab pane | Nice-to-have; M2 exit is one client |
| Clone-and-PR | After apply fixtures |

## How to add a type

1. Put it in [subset.md](./subset.md) (or opaque/loss).
2. Add `rt-*` golden.
3. If apply should see it: add `ap-*`.
4. Do not expand the subset in a review-UI milestone.

Until **M6** rows are green: do not tell agents that editing git `.md` will apply.
