# M0 — Empty host

| | |
|---|---|
| **planId** | `m0-empty-host` |
| **Milestone** | [M0 in the implementation plan](../venus-implementation-plan.md#m0--empty-host-days) |
| **Duration** | A few days, not a week |
| **Encoding** | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B) |
| **Board** | [M0.state.yaml](./M0.state.yaml) |

Parent design: [venus-design.md](../venus-design.md). Tool choices and bindings: [venus-implementation-plan.md](../venus-implementation-plan.md). Licensing: [licensing.md](../../legal/licensing.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

## Story

As an implementer I need a **thin host** around BlockSuite: one workspace, one page, the AFFiNE page editor, and the in-page outline. That is the first coding slice. Venus (catalog, git, lease, review) is not this milestone.

If people cannot type in a real BlockSuite page on localhost, later milestones have nothing to attach to.

## Exit

All of these must be true at once:

1. `pnpm --filter @venus/web dev` serves a full-viewport page editor.
2. The seeded page has a title, body text, and at least two heading levels.
3. Typing, slash menu, lists, and undo work in the editor (default AFFiNE widgets; Venus does not reimplement them).
4. The outline lists those headings; clicking one scrolls the editor to it.
5. There is **no** WebSocket, OctoBase, IndexedDB, or git write. Refresh discards the session.
6. No `@affine/core` (or AFFiNE GraphQL / copilot) dependency.
7. [api-map.md](./api-map.md) is filled for every design name the code uses.

## Non-goals (do not start)

| Later | Why not M0 |
|---|---|
| OctoBase, y-websocket, two tabs | M1 |
| IndexedDB “so refresh keeps text” | Optional even in M1; OctoBase is the refresh source there |
| Markdown pane / adapter fixtures | M2 |
| `wiki/` git repo, flush, autocomment | M3 |
| Folder tree UI, catalog CRDT | M4 — outline is **in-page headings only** |
| Lease, freeze, CodeMirror | M5 |
| Review After/Before/Diff | M6 |
| Docker Compose `octobase` + `venus-web` | M0+M1 together; not required to close M0 |
| Edgeless / whiteboard as a product surface | v1 non-goal |
| Identity, auth, display names | v1 can wait |

Do not build a kanban, cycles, or a second TOC that looks like a wiki sidebar. If the home screen is a board, you are building the wrong product ([pitch.md](../../marketing/pitch.md)).

## Constraints

1. **Thin host.** Vite + React in `apps/web`. BlockSuite stays web components. React only mounts them (refs / `appendChild`). Do not wrap every block in React.
2. **Copy playground patterns, not AFFiNE the product.** Allowed sources: `@blocksuite/affine`, `@blocksuite/store`, `@toeverything/theme`, and a **short** editor-container file copied from AFFiNE’s BlockSuite integration-test (see step-editor). Forbidden: `@affine/core`, explorer, GraphQL, copilot.
3. **One collection, one page.** Fixed ids (e.g. workspace `venus-m0`, doc `doc:home`). No page list.
4. **Memory only.** No persistence provider. A `SyncProvider` stub is allowed so M1 does not reshape the host (step-provider-stub).
5. **Page mode.** `mode = 'page'`. Do not ship an edgeless toggle as UI. The container may still accept edgeless specs internally if the copied class requires them; do not expose that.
6. **Pin exact versions.** Do not depend on floating `canary` tags in lockfile-worthy code. Pin `yjs` to the version `@blocksuite/affine` already uses (implementation-plan risk: version skew).
7. **MPL / notices.** Keep BlockSuite license files and npm notices. Venus application files stay separate ([licensing.md](../../legal/licensing.md)). OctoBase is not in M0, so AGPL is not in the web app yet.

## Target tree

Only create what M0 needs. Do **not** add empty `packages/catalog`, `packages/review`, `crates/`, or `wiki/` until their milestones.

```text
Venus/
  pnpm-workspace.yaml
  package.json
  .gitignore
  apps/web/
    package.json
    tsconfig.json
    vite.config.ts
    index.html
    src/
      main.tsx
      App.tsx
      host/
        boot.ts              # effects, theme, fonts — once
        workspace.ts         # one collection, one store
        editor-container.ts  # copied thin Affine editor element
        mount-editor.ts
        mount-outline.ts
        seed.ts
        sync-provider.ts     # MemoryNoopProvider
      styles.css
    e2e/
      m0-smoke.spec.ts       # Playwright, step-verify
  docs/design/M0/
    …
```

Root `pnpm-workspace.yaml`:

```yaml
packages:
  - "apps/*"
```

`apps/web` package name: `@venus/web`.

## Binding (what you are proving)

```text
Vite React shell
    │  mounts (does not reimplement blocks)
    ▼
VenusEditorContainer (web component)
    │  .doc = Store
    │  .pageSpecs = viewManager.get('page') + fonts
    │  .mode = 'page'
    ▼
EditorHost  ──►  OutlinePanel.editor
    │
    ▼
Store (block tree on a Y.Doc)
    │
    ▼
Workspace / collection  (in-memory, no provider)
```

Default AFFiNE page tree (must exist before attach):

```text
affine:page
  affine:surface
  affine:note
    affine:paragraph | affine:heading | …
```

## Current BlockSuite (verify in step-recon)

AFFiNE canary BlockSuite **0.27** (August 2026) no longer matches old docs that say `@blocksuite/presets` + `DocCollection` + `PageEditor`. Treat those as **design aliases**. Expected production path:

1. `import '@blocksuite/affine/effects'` once.
2. `Schema` + `AffineSchemas`.
3. `Workspace` / `TestWorkspace`: `meta.initialize()`, `storeExtensions` from `StoreExtensionManager(getInternalStoreExtensions()).get('store')`.
4. `createDoc(id).getStore()`, then `store.load(() => addBlock(...))`.
5. Editor: copy `TestAffineEditorContainer` from AFFiNE `blocksuite/integration-test/src/editors/editor-container.ts` into `apps/web/src/host/editor-container.ts`. Register `affine-editor-container`. Set `doc`, `pageSpecs`, `edgelessSpecs` (even if unused), `mode = 'page'`.
6. `pageSpecs`: `ViewExtensionManager(getInternalViewExtensions()).get('page')` plus `FontConfigExtension(CommunityCanvasTextFonts)`.
7. Outline: `OutlinePanel` from `@blocksuite/affine/fragments/outline`; `panel.editor = editor.host` (EditorHost). Include `OutlineViewExtension` via the view manager (already in `getInternalViewExtensions()`).
8. CSS: `@toeverything/theme/style.css` and `fonts.css`. Full-height `#root` / editor viewport.

Do **not** add `@blocksuite/integration-test` as a product dependency. Copy the container (~200 lines). Do **not** import AFFiNE’s React `page-detail-editor`.

If installed packages disagree with this list, **api-map.md wins**. Update the map; do not fight the packages.

---

## Steps

Do them in order. A step is not started until its `dependsOn` steps are done.

### step-scaffold

| | |
|---|---|
| **id** | `step-scaffold` |
| **title** | pnpm workspace and Vite React app |
| **dependsOn** | (none) |
| **kind** | implement |

#### Work

1. Root `package.json`: `private`, `packageManager` (pnpm), scripts that delegate (`dev` → `@venus/web`). Node `>=22` in `engines` (or 20 if you must; pick one and stick to it).
2. `pnpm-workspace.yaml` with `apps/*`.
3. `.gitignore`: `node_modules`, `dist`, `.vite`, Playwright output, OS junk. Do not ignore `pnpm-lock.yaml`.
4. `apps/web`: Vite + React + TypeScript (`strict`). `index.html` with `#root`. `src/main.tsx` renders a placeholder (`Venus M0`) so the app boots **before** BlockSuite is installed.
5. Path alias `@/` → `src/` is optional; do not add a UI kit, router, or state library.

#### Do not

- Add BlockSuite yet (that is step-pin).
- Add Tailwind / MUI / a design system. Host CSS is enough.
- Initialize extra packages.

#### DoD

1. **App boots.** Given a clean clone, when `pnpm install` and `pnpm --filter @venus/web dev` run, then the browser shows the placeholder with no console error from the host.
2. **Workspace only.** Given `apps/`, when you list packages, then only `@venus/web` exists.

---

### step-pin

| | |
|---|---|
| **id** | `step-pin` |
| **title** | Pin BlockSuite to one AFFiNE-matching set |
| **dependsOn** | `step-scaffold` |
| **kind** | implement |

#### Work

1. Install `@blocksuite/affine` at an **exact** version (start from the current AFFiNE canary BlockSuite version, e.g. `0.27.x` — confirm on npm / AFFiNE `blocksuite/affine/all/package.json`).
2. If `@blocksuite/store` is not fully re-exported, add it at the **same** version.
3. Add `@toeverything/theme` at the version affine’s integration-test uses (or the peer affine documents).
4. Pin `yjs` to affine’s dependency (do not let npm hoist a second major).
5. Add `lit` and `@preact/signals-core` if the copied editor container needs them as direct deps (do not rely on accidental hoisting).
6. Vite: if the first affine import fails on WASM or CJS, add the smallest plugin that unblocks (`vite-plugin-wasm`, `vite-plugin-top-level-await`). Record it in api-map notes.
7. Write the versions into [api-map.md](./api-map.md) (package table). Commit the lockfile.

#### Do not

- Use `@blocksuite/presets` / `@blocksuite/blocks` unless the pinned affine package **is** those names (old 0.15 line). Prefer `@blocksuite/affine`.
- Mix two BlockSuite versions.
- Add OctoBase or `@blocksuite/sync` providers.

#### DoD

1. **Same version.** Given `apps/web/package.json` and the lockfile, when you inspect `@blocksuite/*` and `yjs`, then they resolve to one coherent set with no duplicate major `yjs`.
2. **Import compiles.** Given `src/host/boot.ts` with only `import '@blocksuite/affine/effects'` and theme CSS, when `pnpm --filter @venus/web build` (or `tsc --noEmit`) runs, then it succeeds.

---

### step-recon

| | |
|---|---|
| **id** | `step-recon` |
| **title** | Map design names to installed exports |
| **dependsOn** | `step-pin` |
| **kind** | implement |

This step is documentation plus a spike, not product UI. It exists because M0 dies if you code against `DocCollection` and the package exports `Workspace`.

#### Work

1. Open `node_modules/@blocksuite/affine/package.json` `exports` (and store). Record collection class, `createDoc` / `getStore`, `Store`, `spaceDoc`, outline export, ext-loader.
2. Fill every **Actual import** cell in [api-map.md](./api-map.md).
3. Optional 30-minute spike in `src/host/spike.ts` (deleted before M0 exit, or never committed): create schema + empty store, `console.log` root flavour. No UI required.
4. If a symbol is missing, try the “likely 0.27” column. If still missing, read AFFiNE git at the SHA that published this npm version (`blocksuite/integration-test/src/__tests__/utils/setup.ts` and `editors/editor-container.ts`). Update the map; do not guess in later steps.

#### Do not

- Leave api-map “likely” cells as the source of truth.
- Start the React editor mount here (that is step-editor).

#### DoD

1. **Map complete.** Given [api-map.md](./api-map.md), when a reviewer greps `apps/web/src` later, then every BlockSuite import appears in the Actual column.
2. **Store exists.** Given the recon spike or `workspace.ts` draft, when you create one doc and `load` the default tree, then `store.root` flavour is `affine:page`.

---

### step-workspace

| | |
|---|---|
| **id** | `step-workspace` |
| **title** | One collection, one page, default block tree |
| **dependsOn** | `step-recon` |
| **kind** | implement |

#### Work

1. `src/host/workspace.ts`: function `createM0Workspace()` that returns `{ workspace, store, docId }`.
2. Register schema (`AffineSchemas`). `meta.initialize()`. Assign `storeExtensions`.
3. `createDoc('doc:home')` (or the Actual API). `getStore()`. `load` and add:
   - `affine:page` with title `Text('Venus')`
   - `affine:surface` under the page
   - `affine:note` under the page
   - one empty `affine:paragraph` under the note (seed headings come in step-seed)
4. `store.resetHistory()` after seed so undo does not walk the constructor.
5. Call this from `App.tsx` via `useMemo` (one workspace per session). No React context framework; a small context is fine if it avoids prop drilling.

#### Do not

- Attach IndexedDB / any `collection.blobSync`.
- Create a second doc “for later.”
- Randomize ids on every HMR if that remounts a second tree; stable ids are easier to debug. HMR may still duplicate custom-element registration — guard `customElements.get`.

#### DoD

1. **Single page.** Given the running app (even with a stub mount), when you inspect the workspace, then it has exactly one doc and that doc’s store has `affine:page` → `affine:note` → `affine:paragraph`.
2. **No network.** Given the browser Network tab (filter WS), when the app loads, then no sync socket is opened.

---

### step-editor

| | |
|---|---|
| **id** | `step-editor` |
| **title** | Mount the page editor |
| **dependsOn** | `step-workspace` |
| **kind** | implement |

#### Work

1. Copy AFFiNE `blocksuite/integration-test/src/editors/editor-container.ts` into `src/host/editor-container.ts`. Rename the class to `VenusEditorContainer` (keep the `affine-editor-container` tag **or** a Venus tag — one tag, registered once). Adjust imports to the Actual paths in api-map. Strip edgeless-only CSS only if it is clearly unused and the file still compiles; otherwise leave it.
2. `src/host/boot.ts`: import affine effects, theme CSS, fonts CSS. `customElements.define` if the class is not already defined.
3. `src/host/mount-editor.ts`: create the container, set `doc` / `mode` / `pageSpecs` / `edgelessSpecs` (copy the integration-test pattern: `viewManager.get('page'|'edgeless')` + `FontConfigExtension`). `autofocus` on.
4. `App.tsx`: a full-viewport host `div`; `useEffect` appends the container and removes it on unmount.
5. Layout CSS: `html, body, #root` height 100%; editor column `overflow: auto`; `box-sizing: border-box`.
6. Confirm slash menu, format toolbar, and drag-handle appear from **page specs**, not from Venus UI.

#### Do not

- Import `@blocksuite/integration-test`.
- Import `@affine/core`.
- Build a Venus toolbar. If a widget is missing, the view manager is incomplete — fix specs, do not fake a toolbar.

#### DoD

1. **Type.** Given the dev server, when you click the paragraph and type `hello`, then the characters appear in the note.
2. **Slash.** Given the caret in a paragraph, when you type `/`, then the slash menu opens.
3. **Undo.** Given typed text, when you undo, then the text reverts.
4. **No AFFiNE app shell.** Given `apps/web/package.json`, when you read `dependencies`, then there is no `@affine/core`.

---

### step-seed

| | |
|---|---|
| **id** | `step-seed` |
| **title** | Seed headings so outline is non-empty |
| **dependsOn** | `step-editor` |
| **kind** | implement |

#### Work

1. `src/host/seed.ts`: after the empty paragraph, add at least:
   - `affine:heading` level 1 — e.g. `Why Venus`
   - `affine:paragraph` — one sentence
   - `affine:heading` level 2 — e.g. `Empty host`
   - `affine:paragraph` — one sentence
2. Keep the page title `Venus` (or `Venus M0`).
3. Enough vertical space that scrolling to H2 is observable (min-height on the note or extra paragraphs).

#### Do not

- Load markdown through `MarkdownAdapter` (M2).
- Fetch remote templates.

#### DoD

1. **Visible structure.** Given a fresh load, when you look at the page, then you see a title, an H1, and an H2 without typing.
2. **Constructor undo.** Given a fresh load, when you undo once, then you do not delete the whole page tree (history was reset after seed).

---

### step-outline

| | |
|---|---|
| **id** | `step-outline` |
| **title** | Mount the in-page outline |
| **dependsOn** | `step-seed` |
| **kind** | implement |

This is BlockSuite’s **heading TOC**, not the wiki folder tree ([venus-design.md](../venus-design.md#folder-tree-table-of-contents)).

#### Work

1. After `editor.updateComplete`, read `editor.host`. If host is null, wait one frame / subscribe to the same hook AFFiNE uses — do not pass the container into OutlinePanel.
2. `src/host/mount-outline.ts`: `new OutlinePanel()`, `panel.editor = host`, `fitPadding` (e.g. `[20, 20, 20, 20]`).
3. Layout: editor left (flex 1), outline right (~280–320px). Outline is always visible in M0 (no AFFiNE sidebar chrome).
4. If headings do not list: confirm `OutlineViewExtension` is in the view manager and fragment effects ran. Fix registration; do not write a custom TOC.

#### Do not

- Build a file-tree sidebar, mock folders, or “pages” list.
- Put outline widgets into the block schema.

#### DoD

1. **Lists headings.** Given a fresh load, when you look at the outline, then `Why Venus` and `Empty host` (or whatever you seeded) appear.
2. **Tracks edits.** Given the editor, when you change an H1’s text, then the outline label updates without reload.
3. **Scroll.** Given a long page, when you click the H2 in the outline, then the editor scrolls so that heading is in view.
4. **Not a wiki TOC.** Given the outline, when you inspect it, then it only reflects headings of the open page (no folders).

---

### step-provider-stub

| | |
|---|---|
| **id** | `step-provider-stub` |
| **title** | Swappable sync interface, memory no-op |
| **dependsOn** | `step-workspace` |
| **kind** | implement |

The implementation plan requires a swappable provider **from day one** (Yjs binaries + spaces, not OctoBase APIs). M0 still has **no** real sync.

#### Work

1. `src/host/sync-provider.ts`:

```ts
export interface SyncProvider {
  readonly kind: 'memory' | 'octobase' | 'y-websocket';
  connect(docId: string, ydoc: unknown): void;
  disconnect(docId: string): void;
}

export class MemoryNoopProvider implements SyncProvider {
  readonly kind = 'memory';
  connect(): void {}
  disconnect(): void {}
}
```

Replace `ydoc: unknown` with the Actual Y.Doc type from api-map once known.

2. `createM0Workspace` takes `SyncProvider` (default `MemoryNoopProvider`) and calls `connect(docId, ydoc)` after load. Disconnect on App unmount.
3. Comment on the interface: M1 implements OctoBase (or y-websocket) behind this; the editor host must not import OctoBase.

#### Do not

- Implement IndexedDB here.
- Leak OctoBase types into `apps/web`.

#### DoD

1. **No-op session.** Given `MemoryNoopProvider`, when you type and refresh, then the text is gone (still memory-only).
2. **Seam exists.** Given `workspace.ts`, when M1 starts, then a second `SyncProvider` can be passed without changing `mount-editor.ts`.

---

### step-verify

| | |
|---|---|
| **id** | `step-verify` |
| **title** | Browser + smoke test |
| **dependsOn** | `step-outline`, `step-provider-stub` |
| **kind** | test |

#### Work

1. **Manual (required):** open the app in a real browser. Type, slash-insert a list, add an H3, confirm outline, click to scroll, refresh and confirm blank seed (not your typing). Check the console for uncaught errors.
2. Playwright in `apps/web/e2e/m0-smoke.spec.ts`:
   - Go to `/`
   - Assert outline contains the seeded H1 text
   - Type into the editor (use the actual editable selector you discover; do not guess from old AFFiNE tests if it fails — record the selector in api-map notes)
   - Assert the typed string is visible
3. `package.json` script `test:e2e`. Run it in CI later; for M0, running locally is enough.
4. Delete any recon spike file.
5. Set [M0.state.yaml](./M0.state.yaml) steps to `done` when each DoD is actually met.

#### Do not

- Call M0 done from a screenshot of first paint.
- Add visual-regression baselines.

#### DoD

1. **Manual path.** Given Chrome or Firefox, when a person follows the manual list above, then all behaviors hold and the console has no host exceptions.
2. **Smoke.** Given `pnpm --filter @venus/web test:e2e`, when it runs against `vite preview` or `dev`, then it exits 0.
3. **Refresh.** Given typed text, when you reload, then that text is absent and the seed headings are back.

---

## Order of work (calendar)

| When | Steps |
|---|---|
| Day 1 | `step-scaffold` → `step-pin` → `step-recon` → start `step-workspace` |
| Day 2 | `step-workspace` → `step-editor` → `step-seed` |
| Day 3 | `step-outline` → `step-provider-stub` → `step-verify` |

If step-pin or recon slips (WASM, missing exports), stop and fix the map. Do not “temporarily” mount `@affine/core`.

## Risks

| Risk | What to do in M0 |
|---|---|
| Package names ≠ this plan | api-map is the contract; this plan’s 0.27 hints are a starting point |
| Editor container needs more than one file | Copy the smallest set from `integration-test/src/editors/`; still no `@affine/core` |
| Custom elements double-register on HMR | Guard `customElements.get` |
| Outline empty | Host not ready; or fragment/view extension missing |
| Fonts / CSS missing | Page looks “broken” but CRDT is fine — add theme + FontConfigExtension |
| Vite cannot bundle affine | WASM / lit / CJS plugins; document in api-map |

## Handoff to M1

M1 may assume:

- `@venus/web` boots one page editor + outline.
- `SyncProvider` is the only place new networking is added.
- Workspace id and `doc:home` can stay for the first OctoBase space, or be replaced **inside** `workspace.ts` without rewriting the outline.
- Still no git, lease, catalog, or markdown pane.

M1 exit is “refresh / second client sees the same page.” M0 exit is the opposite: refresh **must not** keep typed text.

## Invariants (M0 only)

1. One workspace, one page, page mode, in-memory.
2. Outline = headings of that page.
3. No AFFiNE application shell.
4. No sync I/O.
5. Venus product layers are not present yet — and that is correct.
