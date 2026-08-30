# M2 — Markdown projection

| | |
|---|---|
| **planId** | `m2-markdown-projection` |
| **Milestone** | [M2 in the implementation plan](../venus-implementation-plan.md#m2--markdown-projection-week) |
| **Duration** | About a week |
| **Encoding** | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B) |
| **Board** | [M2.state.yaml](./M2.state.yaml) — steps 1–8 `done` |

Parent design: [venus-design.md](../venus-design.md). Adapter: [MDGate](../MDGate/README.md) ([subset](../MDGate/subset.md), [fixtures](../MDGate/fixtures.md), [live-pane](../MDGate/live-pane.md)). Stores: [datamodel](../datamodel/README.md) — markdown is a **projection**, not Postgres and not git. Dataflow: [architecture.md](../architecture.md#markdown-projection-add-here-before-coding-m2). Words: [glossary.md](../glossary.md). M1 host: [M1/plan.md](../M1/plan.md). Installed symbols: [api-map.md](../api-map.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

## Story

As an implementer I need a **read-only markdown pane** that stays aligned with the live BlockSuite page: `MarkdownAdapter.fromDoc` on the **synced Store**, plus a RAM block-id sidecar. The [export fixture suite](../MDGate/fixtures.md#m2--export-must-be-green-to-close-m2) is green. Venus still has no git write, lease, apply, or editable markdown.

If export jitters, later comment-commits will be fake hunks. If the pane is a second replica, we have already lost.

## Exit

All of these must be true at once:

1. One shared **exporter** (`fromDoc` + sidecar builder). Pane, tests, and later pin/git (M3) import it. [one exporter](../MDGate/README.md#one-exporter).
2. [subset](../MDGate/subset.md) types: `fromDoc → toDoc → fromDoc` byte-stable modulo listed whitespace. Goldens checked in.
3. Sidecar: every exported subset (and opaque) block has a stable id; **no** ids in the markdown body; insert-above does not rename surviving ids.
4. Opaque / loss: untouched opaque slice is byte-stable; color (if present) is documented loss, second `fromDoc` stable.
5. Read-only markdown pane hangs off the **live Store** ([architecture](../architecture.md#markdown-projection-add-here-before-coding-m2)), not keck `GET …/export`. [Single-flight loop](../MDGate/live-pane.md) ([step-loop](#6-step-loop)): in-place splice or full `fromDoc`. **No caret.** Paint the resulting string with **highlight.js** (markdown grammar) so headings/fences/comments are visible as **source**, not as rendered HTML.
6. If the pane is not shown: do not run the loop.
7. [fixtures.md](../MDGate/fixtures.md) **M2 export** rows all pass in CI (`pnpm test` + `pnpm test:e2e` pane spec). **Apply rows (`ap-*`) are out of M2.**
8. No `wiki/` git commit, no lease, no CodeMirror, no `Y.applyUpdate` of `toDoc`.
9. [api-map.md](../api-map.md) Actual column is filled for every adapter name the code uses.
10. `mount-editor` still does not import the adapter. Layout chrome (App) mounts the pane, like outline.

M1 + M2 together: same Y.Doc in two tabs **and** a markdown photograph of that tree on one client. Postgres is still only Yjs (+ blobs). Sidecar on **disk** is M3 ([datamodel git](../datamodel/git.md)).

## Non-goals (do not start)

| Later | Why not M2 |
|---|---|
| `wiki/` git, autocomment | M3 — [LiveSnapshot](../LiveSnapshot/README.md). Convert helper `pinThenFromDoc` is in mdgate; this milestone does not write git. |
| Apply / hunks / `ap-*` fixtures | M6 — [apply.md](../MDGate/apply.md) |
| CodeMirror, lease, freeze | M5. M2 pane uses **highlight.js**, not CM. |
| After/Before OctoBase spaces | [datamodel CRDT](../datamodel/crdt.md#commit-before-and-after); M5–M6 |
| Catalog, folder tree, product header | M4 |
| Second page / linked-doc **resolution** in the catalog | M4. M2 may **export** the linked-doc markdown form with a synthetic `pageId`. |
| Telling agents “edit `.md` in git and it will apply” | After M6 apply fixtures |
| Markdown as Y.Text | Forbidden ([datamodel](../datamodel/README.md)) |

Do not write sidecar JSON to Postgres or `wiki/.venus/ids/`. RAM only.

## Constraints

1. **Thin host.** Same Vite + React app. Pane is host chrome: a `<pre><code>` painted by **highlight.js**, not a BlockSuite widget, not a `<textarea>` (a textarea cannot hold token spans).
2. **One Store.** `fromDoc` the same `session.store` as the editor. Do not `GET /api/block/…/export` for the pane.
3. **Single-flight + dirty** ([live-pane](../MDGate/live-pane.md#scheduler-m2-must-ship-this)). Not one job per Yjs event. In-place edits **splice** RAM markdown + shift later sidecar ranges; structural edits **fall back** to full `fromDoc`. Git / lease `T0` stay full `fromDoc` on a **pin** (M3).
4. **Seam.** Adapter lives in `src/host/mdgate/` (or equivalent). `mount-editor.js` / `editor-container.js` / `boot.js` do not import it.
5. **Memory default.** Vitest and `pnpm test:e2e` stay green without Docker. Pane e2e may use memory (like M0). Optional: same pane on Compose — not required to close M2.
6. **Goldens win.** If adapter bytes disagree with [subset](../MDGate/subset.md) intent, **update subset Actual**, do not weaken tests.
7. **One page.** Workspace `venus-m0`, doc `doc:home`. Do not add catalog spaces.
8. **Pin `yjs` 13.6.32** and BlockSuite **0.22.4**. Headings stay `affine:paragraph` + `type` h1/h2.
9. **Source highlight, not a preview.** `highlight.js` colors the `fromDoc` bytes (`#`, fences, `<!-- … -->` stay in `innerText`). Do not run markdown-it / marked / a second adapter to turn the pane into HTML headings. Do not import `highlight.js` from `from-doc.js` (exporter stays bytes-only). CodeMirror stays M5.

## Target tree

Only create what M2 needs. Do **not** add `wiki/`, `packages/review`, or apply tests.

```text
Venus/
  apps/web/
    src/host/
      mdgate/
        from-doc.js            # MarkdownAdapter.fromDoc + sidecar ranges (step 2)
        from-doc.d.ts
        markdown-adapter.js    # recon: adapter + middlewares (no sidecar)
        markdown-adapter.d.ts
        from-doc.test.ts       # seed fromDoc; side-ids; gaps
        roundtrip.test.ts      # rt-* goldens (step 3)
        sidecar.test.ts        # side-stable, side-shift, opaque, loss-color (step 4)
        goldens/               # seed, rt-*, opaque-image.md, loss-color.md
        highlight-md.js        # highlight.js core + markdown grammar → HTML
        pin-from-doc.js        # pin Yjs bytes, hydrate clone, fromDoc (git/T0 convert)
        pin-from-doc.test.ts
        splice.js              # incrementalFromDoc (in-place or full)
        splice.test.ts         # incr-*
        md-pane-loop.js        # single-flight dirty / running
        md-pane-loop.test.ts   # coalesce with fake timers
        one-exporter.test.ts   # pane imports from-doc.js; editor mount ignorant
        mount-md-pane.js       # read-only host; paint; subscribe + loop
    src/App.tsx                # .md-pane-host beside editor + outline
    e2e/
      m2-pane.spec.ts          # e2e-pane
      m0-*.spec.ts             # still pass
      m1-*.spec.ts             # still pass when PLAYWRIGHT_M1=1
  docs/design/api-map.md       # adapter Actuals in step 1
  docs/design/MDGate/subset.md # whitespace Actual after recon
  docs/design/M2/
    …
```

## Binding (what you are proving)

```text
WYSIWYG  (session.store)  ──single-flight──►  full fromDoc | in-place splice
                                              │
                                              ▼
                                    read-only markdown pane
                                    (RAM string + ranges, highlight.js paint, no caret)

Postgres / keck / git     unchanged from M1
```

Ids stay on the CRDT. Sidecar offsets are a photograph of **this** export ([MDGate README](../MDGate/README.md)).

## Chosen stack

Locked in [step-recon-adapter](#1-step-recon-adapter). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.

| Piece | Actual |
|---|---|
| Adapter | `MarkdownAdapter` from `@blocksuite/affine/shared/adapters`. `new MarkdownAdapter(store.getTransformer(middlewares), store.provider)`. `fromDoc` / `toDoc` / `toDocSnapshot`. |
| Transformers | `titleMiddleware(workspace.meta.docMetas)`, `docLinkBaseURLMiddleware(workspace.id)`, `embedSyncedDocMiddleware('content')` — all three. |
| Sidecar | Venus JSON `{ docId, clock, blocks: [{ id, start, end }] }` — UTF-16 `[start,end)`. `from-doc.js` (step 2 done). |
| Pane | Host `<pre><code data-testid="venus-md-pane">` (or testid on the host). Not CodeMirror. Step 5. |
| Highlighter | **highlight.js** — `highlight.js/lib/core` + `highlight.js/lib/languages/markdown` + `highlight.js/styles/github.css`. Pin **11.11.1** in [api-map](../api-map.md). |

## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).

| # | id | Adds |
|---|---|---|
| 1 | [`step-recon-adapter`](#1-step-recon-adapter) | **Done.** api-map adapter Actuals; seed `fromDoc` golden; subset whitespace Actuals. |
| 2 | [`step-exporter`](#2-step-exporter) | **Done.** Shared `fromDoc` + sidecar module. |
| 3 | [`step-roundtrip`](#3-step-roundtrip) | **Done.** `rt-*` goldens on the documented subset. |
| 4 | [`step-sidecar`](#4-step-sidecar) | **Done.** Stable ids, shift on insert, opaque image, loss-color. |
| 5 | [`step-pane`](#5-step-pane) | **Done.** Read-only pane; initial `fromDoc`; **highlight.js** source paint. |
| 6 | [`step-loop`](#6-step-loop) | **Done.** Single-flight; in-place splice or full `fromDoc`; re-paint highlight.js. |
| 7 | [`step-one-exporter`](#7-step-one-exporter) | **Done.** Pane uses the same helper as Vitest; highlighter is not a second dialect. |
| 8 | [`step-verify`](#8-step-verify) | **Done.** Close-out: person in browser (including highlighted source) + full export suite. |

---

## Steps

Do them in order (1–8). A step is not started until its `dependsOn` steps are done. Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

Fixture ids (`rt-paragraph`, `e2e-pane`, …) are defined in [fixtures.md](../MDGate/fixtures.md). Do not rename them.

### 1. step-recon-adapter

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-exporter) · [3](#3-step-roundtrip) · [4](#4-step-sidecar) · [5](#5-step-pane) · [6](#6-step-loop) · [7](#7-step-one-exporter) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 1 |
| **id** | `step-recon-adapter` |
| **title** | Map MarkdownAdapter, first fromDoc, subset Actuals |
| **dependsOn** | (none; M1 closed) |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** a decision and a map, not the pane. You know the Actual import of `MarkdownAdapter`, `fromDoc` / `toDoc` (or `toDocSnapshot`), transformers, and what `fromDoc` of the M0 seed looks like. [subset.md](../MDGate/subset.md) recon checklist is started.

M2 dies if you code against guessed package paths or hide adapter jitter in tests.

#### Work

1. Find `MarkdownAdapter` on `@blocksuite/affine` **0.22.4**. Record import path, constructor, `fromDoc` / `toDoc` / `toDocSnapshot` names in [api-map.md](../api-map.md) (new **Names — markdown adapter** rows).
2. In a Vitest (Node) test, `createM0Workspace(MemoryNoopProvider)`, seed as today, call `fromDoc`. Log / write a **draft** golden of the seed note (h1, body, spacers, h2).
3. Append [subset.md](../MDGate/subset.md) **Recon** Actuals: EOF newlines, empty-paragraph bytes, whether list **items** vs list container get sidecar rows (sidecar builder may wait for step 2).
4. Note opaque form for `affine:image` if you `fromDoc` a store that has one (optional here; required in step 4).
5. Confirm `toDoc` of that markdown does not throw. Do **not** require byte-stable round-trip until goldens in step 3.

#### Do not

- Mount a pane or add `highlight.js`.
- Write `wiki/` or Postgres markdown.
- Import `@affine/core`.
- Start apply / hunk code.

#### Test scenarios

1. **Map complete**
   - **Given** the repo after this step.
   - **When** a reviewer opens [api-map.md](../api-map.md) **Names — markdown adapter** Actual column.
   - **Then** these are concrete strings: adapter import, class name, `fromDoc` method, `toDoc` or `toDocSnapshot` method, transformer setup (or “none”).
   - **How:** read the file. Fail if cells are still empty or `recon:`.
   - **Autotest:** none (docs). **Manual:** reviewer reads api-map.

2. **Seed fromDoc**
   - **Given** Node Vitest and `MemoryNoopProvider` (no Docker).
   - **When** the test builds the same seed tree as `seedHomeNote` and calls Actual `fromDoc`.
   - **Then** it returns a string containing `Why Venus` and `Empty host` (seed H1/H2); it does not throw.
   - **How:** `pnpm test` file under `apps/web/src/host/mdgate/` (or recon test path recorded in api-map). **Autotest:** required. **Manual:** none.

#### Done

api-map adapter Actuals filled. A Vitest `fromDoc` of seed succeeds. Subset recon notes started (EOF / empty para). Draft golden may be uncommitted until step 3.

---

### 2. step-exporter

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · **2** · [3](#3-step-roundtrip) · [4](#4-step-sidecar) · [5](#5-step-pane) · [6](#6-step-loop) · [7](#7-step-one-exporter) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 2 |
| **id** | `step-exporter` |
| **title** | Shared fromDoc + sidecar builder |
| **dependsOn** | `step-recon-adapter` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** one module that returns `{ markdown, sidecar }` from a `Store`. Sidecar ids are CRDT block ids; ranges are UTF-16 `[start, end)` per [subset](../MDGate/subset.md#whitespace-rules). Markdown body has **no** `<!-- id:b1 -->`.

This is the exporter M3 will call on a pin. Build it once.

#### Work

1. `apps/web/src/host/mdgate/from-doc.js` (plus `.d.ts`): Actual adapter + transformers; walk document order; attach ranges (gaps between blocks are not part of a range).
2. `clock` on sidecar: Yjs state vector encoding or a documented Store clock; `docId` = `doc:home` (or `store.id` Actual).
3. Vitest: three paragraphs → `side-ids` ([fixtures](../MDGate/fixtures.md)).
4. Do not mount UI.

#### Do not

- Persist sidecar to disk or Postgres.
- Incremental splice.
- `fromDoc` the live Store from a tab for git.

#### Test scenarios

1. **Helper exists**
   - **Given** the module path in api-map **Exporter**.
   - **When** Vitest imports it and calls it on a Store with three paragraphs.
   - **Then** `markdown` is a string; `sidecar.blocks` has three `{ id, start, end }`; ids match `store` block ids; `markdown.slice(start, end)` equals that block’s markdown slice; the markdown file contains **no** `b1` as HTML comments.
   - **How:** `pnpm test` — fixture id `side-ids`. **Autotest:** required. **Manual:** none.

2. **Gaps**
   - **Given** two paragraphs with a blank line between in `fromDoc` output.
   - **When** you inspect ranges.
   - **Then** the extra blank line is in **neither** range (or subset Actual documents otherwise — then update subset, do not skip).
   - **How:** same Vitest file. **Autotest:** required. **Manual:** none.

#### Done

`from-doc.js` is the only exporter. `side-ids` green.

---

### 3. step-roundtrip

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · [2](#2-step-exporter) · **3** · [4](#4-step-sidecar) · [5](#5-step-pane) · [6](#6-step-loop) · [7](#7-step-one-exporter) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 3 |
| **id** | `step-roundtrip` |
| **title** | Subset round-trip goldens |
| **dependsOn** | `step-exporter` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** checked-in goldens and `fromDoc → toDoc → fromDoc` tests for every **rt-*** row except those that need sidecar-only (those stay step 4). Linked-doc **export form** may use a synthetic `pageId` (no catalog).

If a type is unstable, **cut it from [subset.md](../MDGate/subset.md)** and drop the `rt-*` row — do not `replace(/\s+/g, '')` to pass.

#### Work

1. Goldens under `apps/web/src/host/mdgate/goldens/`.
2. Vitest: `rt-paragraph`, `rt-headings`, `rt-list`, `rt-code`, `rt-link`, `rt-linked-doc`.
3. Update [subset.md](../MDGate/subset.md) whitespace Actual from goldens.
4. Bold/italic/inline code: if they survive, add a `rt-marks` golden or document under loss.

#### Do not

- Pane.
- Apply ops.
- Custom fences.

#### Test scenarios

1. **rt-paragraph** — **Given** one `affine:paragraph`. **When** `fromDoc → toDoc → fromDoc`. **Then** second markdown equals golden modulo subset whitespace. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

2. **rt-headings** — **Given** `type: h1` and `type: h2` paragraphs (seed-like). **When** round-trip. **Then** Store still has h1/h2 types; markdown matches golden. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

3. **rt-list** — **Given** bulleted list + one nested item. **When** round-trip. **Then** golden match; sidecar ids follow subset (item vs container). **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

4. **rt-code** — **Given** `affine:code` with a language. **When** round-trip. **Then** fences survive. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

5. **rt-link** — **Given** a paragraph with an inline URL. **When** round-trip. **Then** link in markdown; golden match. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

6. **rt-linked-doc** — **Given** `affine:embed-linked-doc` with a `pageId`. **When** `fromDoc` (+ post-process if needed). **Then** markdown has a path or title **and** `<!-- venus:doc:… -->` with that id. Round-trip as far as the adapter allows; document remainder in subset. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

#### Done

All `rt-*` M2 rows green. Goldens in git. Subset whitespace Actual matches goldens.

---

### 4. step-sidecar

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · [2](#2-step-exporter) · [3](#3-step-roundtrip) · **4** · [5](#5-step-pane) · [6](#6-step-loop) · [7](#7-step-one-exporter) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 4 |
| **id** | `step-sidecar` |
| **title** | Sidecar stability, opaque, loss |
| **dependsOn** | `step-roundtrip` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** proof that ids survive re-export and insert-above, and that opaque/loss do not jitter into fake changes.

**Actual:** Vitest `sidecar.test.ts`. `side-stable` ranges are identical. `side-shift` uses `addBlock(..., parentIndex=0)`. Opaque is `affine:image` → `![dot.png](assets/dot.png)` (`opaque-image.md`). `loss-color`: schema accepts `color`; markdown is plain (`loss-color.md`). Per-block `<!-- id:… -->` is absent.

#### Work

1. Vitest `side-stable`, `side-shift`.
2. Opaque: include `affine:image` (bytes from `e2e/fixtures/dot.png` via store blob API in Node if possible) **or** a flavour recon marked opaque. `fromDoc` twice → opaque slice equal (`opaque-untouched`).
3. `loss-color` if the schema can set color; else document “no color API on 0.22.4” in subset and skip with a named skip reason in the test file (fail if silently omitted).
4. Confirm markdown has no per-block id comments.

#### Do not

- Git `assets/`.
- Apply no-op tests (`ap-opaque-noop` is M6).

#### Test scenarios

1. **side-stable** — **Given** three paragraphs, `fromDoc` twice with no Store mutation. **When** compare sidecars. **Then** same `id`s; ranges differ only if subset whitespace exemption applies (prefer identical). **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

2. **side-shift** — **Given** export of `b1,b2`. **When** `addBlock` **before** `b1`, export again. **Then** `b1` id unchanged; `b1.start` increased. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

3. **opaque-untouched** — **Given** subset paragraph + opaque block. **When** `fromDoc` twice. **Then** opaque slice byte-equal; subset `rt-*` still holds. **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

4. **loss-color** — **Given** a colored paragraph **or** documented skip. **When** `fromDoc` twice. **Then** golden is the flattened form; second export equals first. **How:** `pnpm test`. **Autotest:** required (or skip with subset citation). **Manual:** none.

#### Done

`side-stable`, `side-shift`, `opaque-untouched`, `loss-color` (or skip) green.

---

### 5. step-pane

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · [2](#2-step-exporter) · [3](#3-step-roundtrip) · [4](#4-step-sidecar) · **5** · [6](#6-step-loop) · [7](#7-step-one-exporter) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 5 |
| **id** | `step-pane` |
| **title** | Read-only highlighted markdown pane in the layout |
| **dependsOn** | `step-sidecar` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** host chrome: a markdown pane next to the editor (outline stays). Initial fill from `fromDoc(session.store)`. **highlight.js** paints that string as markdown **source**. Not `contenteditable`. `data-testid="venus-md-pane"`. `mount-editor` still has no adapter import. `from-doc.js` still has no `highlight.js` import.

#### Work

1. Add `highlight.js` to `@venus/web`. Import **core + markdown language only**, not the full grammar bundle. Import **one** theme CSS (any shipped theme). Record package version in [api-map.md](../api-map.md) **Highlight.js**.
2. `highlight-md.js`: `highlight(md)` → HTML string (`hljs.highlight(md, { language: 'markdown' }).value`). No Store, no adapter.
3. `mount-md-pane.js`: attach a `<pre><code>` (or equivalent) to the host; set `innerHTML` from `highlight(fromDoc(store).markdown)`; not `contenteditable`. `innerText` of the pane must still be the exporter markdown (highlighter wraps spans; it must not rewrite the source).
4. `App.tsx`: `.md-pane-host` (left or under editor; outline remains right). Do not remove outline.
5. Unmount stops any loop (loop may be step 6; at least no leak).
6. Playwright: pane visible on `pnpm test:e2e`; `innerText` contains seed H1 text; at least one highlight `<span>` is present.

#### Do not

- CodeMirror, Prism, Shiki, monaco.
- `markdown-it` / `marked` / adapter `toDoc` as a **preview** (that hides `#` / fences).
- Import `highlight.js` from `from-doc.js` or `mount-editor.js`.
- Fetch keck export for the pane.
- Hide outline.
- Incremental splice (that is [step-loop](#6-step-loop)). Initial paint is one full `fromDoc`.

#### Test scenarios

1. **Pane visible**
   - **Given** `pnpm test:e2e` (memory, Vite `:5173`).
   - **When** the app loads.
   - **Then** `[data-testid="venus-md-pane"]` is visible; its **`innerText`** includes `Why Venus`; the element is not `contenteditable=true`.
   - **How:** Playwright `e2e/m2-pane.spec.ts` (or first test in that file). **Autotest:** required. **Manual:** open `pnpm --filter @venus/web dev`, see markdown beside the page.

2. **Source highlighted**
   - **Given** the same load (seed page).
   - **When** you inspect the pane DOM.
   - **Then** the pane contains at least one `span` with an `hljs-` class (heading/section token is enough); `innerText` still includes `#` or the ATX form of the H1 if `fromDoc` emits it (do not require a specific class name beyond `hljs-` prefix — class names may move with highlight.js).
   - **How:** Playwright in `e2e/m2-pane.spec.ts`: `locator('[data-testid="venus-md-pane"] span[class*="hljs-"]')` count ≥ 1. Optional Vitest on `highlight-md.js`: `highlight(seedMd)` HTML contains `<span` and stripping tags yields the input (modulo highlight.js whitespace). **Autotest:** required. **Manual:** seed H1 / H2 / fences (if present) are colored vs body text.

3. **Outline still there**
   - **Given** the same load.
   - **When** you look at the outline host.
   - **Then** outline H1 `Why Venus` still exists (M0 smoke still passes).
   - **How:** existing `m0-smoke.spec.ts` / `m0-outline.spec.ts` still green. **Autotest:** required. **Manual:** optional.

#### Done

Pane shows seed markdown **source**, token-colored. `from-doc.js` unchanged (no highlighter). M0 e2e still pass.

---

### 6. step-loop

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · [2](#2-step-exporter) · [3](#3-step-roundtrip) · [4](#4-step-sidecar) · [5](#5-step-pane) · **6** · [7](#7-step-one-exporter) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 6 |
| **id** | `step-loop` |
| **title** | Single-flight loop + in-place splice |
| **dependsOn** | `step-pane` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** while the pane is mounted, Store updates retrigger export via [single-flight](../MDGate/live-pane.md). **In-place** edits splice the RAM markdown and shift later sidecar ranges. Structural edits fall back to full `fromDoc`. Each run re-highlights the **whole** resulting string (do not patch highlight.js spans). Git / `T0` are **not** this loop.

#### Work

1. Subscribe to Store / Y.Doc updates; implement `dirty` / `running` / `dirtyIds` loop ([live-pane scheduler](../MDGate/live-pane.md#scheduler-m2-must-ship-this)).
2. `splice.js`: given last `{ markdown, sidecar }` + dirty ids, either splice or return “fallback”. Splice only when every dirty id already has a sidecar row, flavour/parent/index are unchanged, and the change is in-place text/marks on that block. Delta is **UTF-16** `newSlice.length − (old end − old start)` from adapter `ownMarkdown` (same post-process as `fromDoc`), **not** Y.Text length. Shift every row with `start >= old end`. Apply dirty ids **document order**.
3. Fallback to full `fromDoc` on insert / delete / move / split / merge / list indent / empty-paragraph last-N / gap-only / opaque / linked-doc title middleware / unknown flavour / recon mismatch.
4. Reconcile: every N splices or on idle, full `fromDoc`; if markdown or ranges diverge (modulo [subset](../MDGate/subset.md) whitespace), keep the full result.
5. Each loop: splice or full `fromDoc` → `highlight-md.js` on the **entire** markdown string → set pane `innerHTML`. Do not incrementally patch token spans.
6. Unmount: unsubscribe; `running` must not paint after unmount.
7. Optional Vitest fake-timers: many events during a slow export → one follow-up, not N jobs.
8. Playwright `e2e-pane`: type in the note; pane **innerText** equals a fresh `fromDoc` (or contains the typed string) after wait; highlight `span`s still present.

#### Do not

- Patch highlight.js token spans.
- Shift ranges from Y.Text / keystroke count (bold does not change Y.Text length; markdown grows).
- Freeze a spliced RAM sidecar as lease `T0` or write `wiki/.venus/ids/`.
- `fromDoc` the live Store for git (M3 pin).
- Require Docker.
- Skip re-highlight (raw `textContent = md` after the first paint).

#### Test scenarios

1. **e2e-pane (autotest)**
   - **Given** memory Vite app, pane open.
   - **When** you click the note and type a unique string (e.g. `m2-hello`).
   - **Then** within **5 seconds** the pane **innerText** includes that string; pane still not `contenteditable`; at least one `span[class*="hljs-"]` remains (re-paint happened, not a dead first highlight).
   - **How:** `pnpm test:e2e` `e2e/m2-pane.spec.ts`. **Autotest:** required.

2. **e2e-pane (manual)**
   - **Given** `pnpm --filter @venus/web dev`.
   - **When** you type in WYSIWYG.
   - **Then** the markdown pane updates without a page reload; tokens stay colored; you cannot type in the pane.
   - **How:** person in Chrome or Firefox. Record in yaml at verify if you skip here.

3. **Coalesce (autotest, optional but recommended)**
   - **Given** a fake exporter that takes 50ms.
   - **When** 20 Store events fire during that run.
   - **Then** at most **two** exporter calls (initial in-flight + one follow-up), not 20.
   - **How:** Vitest fake timers on `mount-md-pane`. **Autotest:** recommended. **Manual:** none.

4. **incr-inplace**
   - **Given** three paragraphs, a full `fromDoc` snapshot.
   - **When** only `b1` text changes (plain insert); run splice on that dirty id.
   - **Then** spliced markdown and sidecar ranges **byte-equal** a fresh full `fromDoc` of the same Store; `b1` id unchanged; later `start`/`end` shifted by the UTF-16 delta.
   - **How:** `pnpm test` `splice.test.ts`. **Autotest:** required. **Manual:** none.

5. **incr-marks**
   - **Given** a paragraph, full `fromDoc`.
   - **When** a bold (or italic / code) mark is applied so Y.Text length is unchanged and markdown grows.
   - **Then** splice (adapter slice, not Y.Text delta) equals full `fromDoc`. A Y.Text-length shift must **not** be how the helper computes delta.
   - **How:** same Vitest file. **Autotest:** required. **Manual:** none.

6. **incr-fallback**
   - **Given** export of `b1,b2`.
   - **When** `addBlock` **before** `b1` (or delete / list indent — one structural case is enough if the helper’s fallback table is unit-tested).
   - **Then** splice helper reports fallback (does not invent rows); a following full `fromDoc` matches `side-shift` (surviving id, `start` moved).
   - **How:** same Vitest file. **Autotest:** required. **Manual:** none.

7. **incr-perf**
   - **Given** a long note (tens of paragraphs), a full `fromDoc` snapshot, one in-place text edit.
   - **When** time `incrementalFromDoc` splice (**on**) vs `forceFull` (**off**), median of a few runs after warmup.
   - **Then** log both medians; splice markdown equals full; splice median wall time is **less than** full. Do not assert a fixed speedup ratio (machines differ).
   - **How:** same Vitest file. **Autotest:** required. **Manual:** none.

#### Done

Typing in the editor updates the pane. Loop is single-flight. In-place splice equals full `fromDoc`; structural edits full-export. highlight.js re-paints the whole string each run. `incr-*` and `e2e-pane` green.

---

### 7. step-one-exporter

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · [2](#2-step-exporter) · [3](#3-step-roundtrip) · [4](#4-step-sidecar) · [5](#5-step-pane) · [6](#6-step-loop) · **7** · [8](#8-step-verify)

| | |
|---|---|
| **n** | 7 |
| **id** | `step-one-exporter` |
| **title** | Pane and tests share from-doc.js |
| **dependsOn** | `step-loop` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** proof there is not a second markdown dialect in App. Fixture `one-exporter`. highlight.js is paint only: pane `innerText` is still `fromDoc`.

#### Work

1. `mount-md-pane.js` imports `from-doc.js` (same specifier as tests) **and** `highlight-md.js` (or inlines the same highlight.js calls).
2. Vitest: `readFileSync` of `mount-md-pane.js` contains the exporter specifier; `from-doc.js` does **not** contain `highlight.js`; `mount-editor.js` does **not** contain `MarkdownAdapter` / `from-doc` / `highlight.js`.
3. Optional: after load, pane `innerText` === `fromDoc(store).markdown` (expose `__VENUS_FROM_DOC__` in test only, or compare via Playwright evaluate if you attach the helper on `window` in e2e only).

#### Do not

- Duplicate adapter setup in App.tsx.
- Pretty-print or re-indent markdown before highlight.js (that would be a second dialect).

#### Test scenarios

1. **one-exporter (autotest)**
   - **Given** the host sources.
   - **When** Vitest reads `mount-md-pane.js`, `from-doc.js`, and `mount-editor.js`.
   - **Then** pane file imports the exporter module; `from-doc.js` has no `highlight.js`; `mount-editor.js` has no `MarkdownAdapter`, no `mdgate/from-doc`, and no `highlight.js`.
   - **How:** `pnpm test`. **Autotest:** required. **Manual:** none.

2. **Pane equals helper (autotest)**
   - **Given** e2e after seed.
   - **When** you read pane `innerText` and run the same exporter in the page (or compare to a known seed golden substring).
   - **Then** pane `innerText` includes the same H1/H2 as `fromDoc` of that store (highlight spans do not add or drop source characters).
   - **How:** extend `m2-pane.spec.ts`. **Autotest:** required. **Manual:** none.

#### Done

`one-exporter` green. Editor mount still ignorant of markdown.

---

### 8. step-verify

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-adapter) · [2](#2-step-exporter) · [3](#3-step-roundtrip) · [4](#4-step-sidecar) · [5](#5-step-pane) · [6](#6-step-loop) · [7](#7-step-one-exporter) · **8**

| | |
|---|---|
| **n** | 8 |
| **id** | `step-verify` |
| **title** | M2 close-out |
| **dependsOn** | `step-one-exporter` |
| **kind** | implement |
| **status** | **done** ([board](./M2.state.yaml)) |

**Adds:** nothing in the product. Marks the board `done`. Person in browser + full export suite.

Add [runbook](../../runbook.md) **Manual testing (M2 close-out)** and [scenarios/markdown-projection.md](../../scenarios/markdown-projection.md) listing the specs.

#### Work

1. Runbook M2 section: Vite memory path; optional Compose (pane still from Store, not export GET).
2. Scenarios page for markdown projection.
3. Walk manual checklist. Fill [M2.state.yaml](./M2.state.yaml) evidence.
4. Confirm **no** `ap-*` tests are required to close M2.

#### Do not

- Start M3 git in this step.
- Close M2 if any export fixture row is skipped without subset citation.

#### Test scenarios

1. **Manual path**
   - **Given** `pnpm --filter @venus/web dev` (memory).
   - **When** you load the app, read the markdown pane (seed headings **as markdown source**, token-colored — not a second WYSIWYG), type a unique word in WYSIWYG, confirm the pane updates and stays highlighted, confirm you cannot type in the pane, refresh (memory: typed word **gone**, seed back — same as M0).
   - **Then** all of that holds. Optional: with Compose + `VITE_SYNC_URL`, refresh **keeps** the word **and** the pane still matches (M1 persist); not required to close M2.
   - **How:** person in Chrome or Firefox. Yaml: browser + date. **Autotest:** no. **Manual:** required.

2. **Smoke (autotest)**
   - **Given** the repo.
   - **When** `pnpm test` and `pnpm test:e2e`.
   - **Then** all `mdgate/*.test.ts` M2 rows pass; `e2e/m2-pane.spec.ts` passes; existing `m0-*.spec.ts` still pass.
   - **How:** CI/local commands. **Autotest:** required. **Manual:** none.

3. **Export suite complete**
   - **Given** [fixtures.md](../MDGate/fixtures.md) M2 table.
   - **When** you check each id: `rt-paragraph`, `rt-headings`, `rt-list`, `rt-code`, `rt-link`, `rt-linked-doc`, `side-ids`, `side-stable`, `side-shift`, `opaque-untouched`, `loss-color`, `incr-inplace`, `incr-marks`, `incr-fallback`, `incr-perf`, `one-exporter`, `e2e-pane`.
   - **Then** each has a passing test or a subset-cited skip (`loss-color` only).
   - **How:** map ids → files in yaml evidence. **Autotest:** the tests themselves. **Manual:** reviewer ticks the table.

4. **M1 still green (if Docker)**
   - **Given** Compose postgres + octobase up.
   - **When** `pnpm test:e2e:m1`.
   - **Then** still passes (pane must not break hydrate/two-tabs).
   - **How:** `pnpm test:e2e:m1`. **Autotest:** required **when** closing M2 on a machine with Compose; if CI has no Docker, local evidence in yaml. **Manual:** none.

#### Done

Board step 8 `done`. M2 README **Exit** holds. Handoff to M3: same `from-doc.js` on a pin; do not `fromDoc` the live Store for git.

---

## Order of work (calendar)

| When | Steps |
|---|---|
| Day 1 | 1 `step-recon-adapter` → 2 `step-exporter` |
| Day 2 | 3 `step-roundtrip` |
| Day 3 | 4 `step-sidecar` |
| Day 4 | 5 `step-pane` → 6 `step-loop` |
| Day 5 | 7 `step-one-exporter` → 8 `step-verify` |

If `fromDoc` is unstable on a subset type, **stop and cut the type** in subset.md. Do not ship a jittering pane.

## Risks

| Risk | What to do in M2 |
|---|---|
| Adapter import / Vite TS | Same M0 `.js` host pattern; do not let `tsc` follow affine `.ts` |
| `fromDoc` jitter | Golden + subset Actual; cut types |
| Pane uses GET export | Fail: Store only |
| Double dialect | step 7 static import check |
| M0 e2e layout break | Keep outline; pane is extra host |
| Linked-doc without catalog | Synthetic pageId; resolution is M4 |
| Highlighter rewrite | `innerText` must equal `fromDoc` markdown; fail if highlight.js (or a preview) drops `#` / fences |
| Splice ≠ full export | `incr-*` must equal full `fromDoc`; never shift by Y.Text length; fall back on structure |
| Scope creep into apply | No `ap-*` in this plan |

## Handoff to M3

M3 may assume:

- `from-doc.js` + sidecar builder; export fixtures green.
- Pane is replaceable; loop is single-flight (in-place splice or full `fromDoc`); **highlight.js** is pane chrome only (M3 convert does not import it).
- **Disk** sidecar and `wiki/` do **not** exist yet. Pin then convert ([LiveSnapshot](../LiveSnapshot/README.md)); do not `fromDoc` the live Store for git. Do not freeze a spliced RAM sidecar as `T0`.
- Apply / lease / After spaces are **not** done.

M3 exit is clone `wiki/` and read markdown. M2 exit is “WYSIWYG and a read-only pane stay aligned on a documented subset.”

M3 **implementation** is gated on [high-availability.md](../LiveSnapshot/high-availability.md) **Acceptance**. M3 is the thin column of that shape (RAM dirty, in-process idle, replica or idle GET). Do not invert it: no `fromDoc` of the live Store for git, no snapshotter in keck, no markdown in Postgres.

## Invariants (M2 only)

1. One workspace, one page, page mode.
2. Markdown is a projection of the live Store, not a replica ([datamodel](../datamodel/README.md)).
3. Sidecar is RAM (and test goldens), not Postgres, not git.
4. No lease, no CodeMirror, no comment-commits.
5. Outline remains in-page headings.
6. `mount-editor` stays unaware of the adapter.
7. **highlight.js** paints the exporter string in the pane only. It is not a markdown store and not part of goldens.
