# M4 — Folder tree + links + product header

| | |
|---|---|
| **planId** | `m4-folder-tree` |
| **Milestone** | [M4 in the implementation plan](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks) |
| **Duration** | About 1–2 weeks |
| **Encoding** | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B) |
| **Board** | [M4.state.yaml](./M4.state.yaml) |

Parent design: [venus-design.md](../venus-design.md) ([folder tree](../venus-design.md#folder-tree-table-of-contents), [cross-document references](../venus-design.md#cross-document-references)). Catalog: [datamodel catalog](../datamodel/crdt.md#catalog). Tree: [CRDT tree](../components/frontend/crdt-tree/). Git: [datamodel git](../datamodel/git.md). Pin + `git mv`: [LiveSnapshot](../LiveSnapshot/README.md). Linked-doc export: [MDGate subset](../MDGate/subset.md#linked-doc-stable-form). Header: [implementation plan — product header](../venus-implementation-plan.md#product-header--venus-chrome-with-the-folder-tree). Hub: [M3.0](../M3.0/README.md), [M3.0 HA](../M3.0/high-availability.md), [backend hub](../components/backend/hub/). Snapshotter: [M3](../M3/README.md). Dataflow: [architecture.md](../architecture.md). Words: [glossary.md](../glossary.md). Installed symbols: [api-map.md](../api-map.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

**Gate (do not skip):** do not start step work in the product repo until [M3](../M3/README.md) is **closed** (board steps 1–9 `done`). This folder existing is not permission to mint a second page space, ship a wiki TOC, or `git mv`.

## Story

As an implementer I need **more than one page** in the wiki: a live **catalog CRDT** (folders, order, names, `gitPath`), a **tree** I can drop to reparent, and a **thin product header** (undo / redo / current page) on that same chrome. Publish must **`git mv`** when the catalog path changed. An **`affine:embed-linked-doc`** keys by `docId`, so a move does not break the card; markdown export uses catalog title + relative path + `<!-- venus:doc:… -->`. Venus still has no lease freeze or comment-commit why.

If path is identity, every rename is a broken link. If the tree is a docs-framework TOC, two people cannot move folders. If undo lives only on the keyboard, managers never find it. If git does not `git mv`, clone folders lie.

## Exit

All of these must be true at once:

1. **Two published pages** on the M0 wiki (seed `doc:home` plus one more, mint `space_id` once). Refresh / second tab hydrates both. Lease grain stays `workspace_id` (one live hub owner for the wiki).
2. **Catalog CRDT** space `venus:catalog` (api-map Actual) on the hub: folders, docs, `parentId`, sibling `order`, derived `gitPath`. Moves = reparent + order. They do **not** rewrite page bodies.
3. **Tree UI** reads only that catalog. Click opens the page in the editor. Drop reparents (live CRDT; other tab sees it without reload). Outline stays the **in-page heading TOC**, not a second wiki tree.
4. **Product header** on the same slice: Undo / Redo on the **open** page’s `Store` (`store.undo()` / `store.redo()`, disabled from `canUndo` / `canRedo` or the matching observables). Shows the current catalog name / `gitPath`. Not `@affine/core`. Not a history list of Yjs transactions.
5. **Layout:** header top; folder tree left; page editor; outline right. Markdown pane may remain host chrome; it must not occupy the tree slot or become the wiki TOC.
6. **Publish includes `git mv`.** Flush pins **catalog + dirty pages** in the same cut. If `gitPath` changed and the body clock did not, **`git mv` only** (no `fromDoc` of the unchanged page). Clone of `wiki/` shows the new folders. Sibling **order** is not in git.
7. **`affine:embed-linked-doc`** with `pageId = docId` plus markdown round-trip of **export** form: catalog title, path relative to the current file, `<!-- venus:doc:<id> -->`. After a move, the card still resolves; the next snapshot’s markdown path matches the new `gitPath`. Recreating the card from markdown is still M6 apply, not adapter `toDoc`.
8. Header undo/redo matches keyboard undo (⌘Z / Ctrl+Z) on the open page.
9. [api-map.md](../api-map.md) Actual column is filled for every catalog / tree / header / `git mv` / linked-doc name the code uses.
10. Existing **M1 Playwright**, M2 export / pane, and M3 Flush / live-during-flush stay green. `mount-editor` still imports neither catalog nor git.

## Non-goals (do not start)

| Later | Why not M4 |
|---|---|
| Lease freeze, CodeMirror, who-holds-the-lease in the header | M5 (header **current page** is this exit; holder chrome waits) |
| Comment-commit, After/Before/Diff, required why | M6 |
| Apply / hunks / `ap-*` / `toDoc` restoring the card | M6 — [apply.md](../MDGate/apply.md). M2 `rt-linked-doc` remainder stands. |
| Threads, alternatives, stacks | M7 |
| Revert | M8 |
| Record wiki remote + product remote@branch | After this exit; [code-bind](../Agents/code-bind.md). Do not delay M5. |
| AB5 analyzer / Aider / CodeGraph | Not this board |
| `@affine/core` explorer, Docusaurus, VitePress as the live tree | Forbidden |
| History timeline of `store.history` | Not a product surface ([implementation plan](../venus-implementation-plan.md#product-header--venus-chrome-with-the-folder-tree)) |
| Empty catalog folders as git placeholders | Git has no empty dirs; catalog may lead |
| Convert / catalog ops inside the hub | Hub stays apply + broadcast + persist + export/blob |
| Auth / ACL | Leftover ([Identity](../venus-implementation-plan.md#identity-v1)) |

Do not treat outline as the wiki tree. Do not `git mv` on every catalog keystroke (only on publish / Flush). Do not rewrite page Yjs on reparent.

## Constraints

1. **Thin host.** Same Vite + React app. Tree + header are host chrome (like Flush and the markdown pane), not BlockSuite widgets. `mount-editor.js` / `editor-container.js` / `boot.js` do not import catalog, tree, header, or git.
2. **Path is not identity.** Links and catalog docs key by `docId` (hub space / BlockSuite guid). `gitPath` is derived and cached. Renames do not mint a new space.
3. **Mint once.** `collection.createDoc()` / hub space ids are created **once**, then synced. Never invent the same `space_id` independently on two devices ([implementation plan — collection](../venus-implementation-plan.md#1-collection--hub-workspace)).
4. **Wiki sticky unchanged.** Owner / lease grain is `workspace_id`, not `doc_id`, not cookie. Many pages live **on that owner**. [M3.0 HA rooms](../M3.0/high-availability.md#rooms).
5. **SQL `doc_id` is UUID v5** of the BlockSuite guid (same DNS namespace as `PAGE_DOC_ID` / `doc:home`). Catalog space guid `venus:catalog` gets its own v5. Recon records both. Hub must persist **any** of those ids, not only `PAGE_DOC_ID`.
6. **Catalog is not an `affine:page`.** It is a small Y.Doc (`Y.Map` of nodes). Do not hang folders on the published block schema.
7. **One apply queue per `docId`**, one persist buffer per `docId`, inside the **one** room for the wiki. Not per socket.
8. **M1 default path stays.** `/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` with no extra doc still hydrates `doc:home`. Bare `GET …/export` still returns that page (M1 tests).
9. **Pin then convert.** Catalog bytes ride in the **same MVCC cut** as dirty pages so `gitPath` matches the files you write ([LiveSnapshot](../LiveSnapshot/README.md)). Convert page bodies with the M3 Rust `fromDoc` (JS CLI oracle). Catalog-only dirty → `git mv` / write paths, **no** `fromDoc` of an unchanged body. Cut still released before `fromDoc`.
10. **Dirty clocks.** Catalog persist upserts `dirty(workspace_id, catalog_sql_uuid, clock)` via the existing trigger. One `jobs` row per wiki still. Sidecar `commit_pins` must stop skipping every `doc_id` that is not home.
11. **Tree data = catalog only.** Drop = catalog reparent. Do not scan `wiki/` for the live tree. Do not use AFFiNE explorer.
12. **Header binds the open `Store`.** Switching pages rebinds undo/redo and the title. Same stack as ⌘Z / Ctrl+Z. No labeled history.
13. **Pin `yjs` 13.6.32** and BlockSuite **0.22.4**. Headings stay `affine:paragraph` + `type` h1/h2.
14. **Memory default.** Vitest and `pnpm test:e2e` stay green without Docker. Tree / header e2e may use memory (like M0/M2). Hub multi-doc and `git mv` need Compose. Unset sync env: no second space on the hub, no git write.
15. **No `@affine/core`.** No nbstore. No convert in the hub. No lease UI.
16. If this plan disagrees with [api-map.md](../api-map.md) after recon, **the map wins**. If it disagrees with [M3.0 HA](../M3.0/high-availability.md) on owner grain, **HA wins**.

## Target tree

Only create what M4 needs. Do **not** add `packages/review`, lease types, or apply tests. Do **not** put catalog merge into `crates/venus-hub` beyond storing another `doc_id`.

```text
Venus/
  crates/venus-hub/                 # Room: Map<doc_id, Doc>; export per doc; default still home
  crates/venus-sidecar/             # cut pins catalog; gitPath from catalog; git2 mv
  apps/web/
    src/host/
      catalog/                      # Y.Map schema + ops; not mount-editor
        schema.js
        ops.js
        git-path.js
        CatalogTree.tsx           # @headless-tree/react; drop → catalog ops
      chrome/                       # header undo/redo + current page
        mount-header.js
      mdgate/from-doc.js            # linked-doc post-process uses catalog title/path
    src/App.tsx                     # header / tree / editor / outline
    e2e/
      m4-tree.spec.ts
      m4-header.spec.ts
      m4-move.spec.ts
      m4-link.spec.ts
  docs/design/api-map.md            # catalog Actuals in step 1
  docs/design/M4/
    …
```

`packages/catalog` from the implementation-plan layout is allowed if recon wants a workspace package; default is `apps/web/src/host/catalog/` (same as mdgate). Do not add an empty package “for later.”

## Binding (what you are proving)

```text
Tab A / Tab B
        │  catalog Y.Doc  venus:catalog     (folders, order, gitPath)
        │  page Y.Doc     doc:home
        │  page Y.Doc     doc:protocol      (mint once)
        ▼
SyncProvider  (kind octobase alias)
        │  AFFiNE WS  /collaboration/<workspace>   (+ doc Actual from recon)
        ▼
Venus hub  one owner per workspace_id
        │  apply / broadcast / persist per docId
        ▼
Postgres  crdt_* / dirty  (home, protocol, catalog each a doc_id)

Flush / idle
        │  claim → MVCC cut of S (dirty pages ∪ catalog)
        │  fromDoc dirty page pins; git mv if gitPath changed
        ▼
wiki/  spec/home.md
       spec/protocol.md   → after move e.g. design/protocol.md
       .venus/ids/<docId>.json
```

Ids (Intent; Actual in recon):

```text
TestWorkspace.id     =  77e4a2b1-8b40-5979-a73c-fd4477216d00
doc:home             =  existing page; SQL PAGE_DOC_ID
venus:catalog        =  catalog Y.Doc guid; SQL v5 of that string
doc:protocol         =  second seed page (or Actual mint); SQL v5
gitPath home         =  spec/home.md
gitPath protocol     =  spec/protocol.md   (until a move)
```

## Chosen stack

Locked in [step-recon-catalog](#1-step-recon-catalog). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.

| Piece | Intent |
|---|---|
| Live CRDT | Unchanged hub + `OctoBaseKeckProvider` (`kind: 'octobase'` alias). **Many `doc_id`s per room.** |
| Catalog | Plain Y.Doc, `Y.Map` nodes. Fields: `id`, `kind` (`folder` \| `doc`), `name`, `parentId`, `order`, `docId?`, `gitPath`. |
| Order | Fractional index among siblings (recon locks the helper). |
| Tree UI | **`@headless-tree/react`** view over the catalog Y.Doc. Data loader reads `Y.Map`; drop calls catalog `reparent` / `setOrder`. Not AFFiNE explorer. Design: [CRDT tree](../components/frontend/crdt-tree/). |
| Header | Host chrome: `store.undo()` / `store.redo()`; `canUndo` / `canRedo` (or `store.history.canUndo$` / `canRedo$`). Title = catalog name / `gitPath`. |
| Linked-doc | `affine:embed-linked-doc` `pageId = docId`. Export: `[title](relative.md)` + `<!-- venus:doc:<id> -->`. |
| Git | Same sidecar. `git2` `git mv` when catalog `gitPath` ≠ last committed path. Autocomment still `snapshot: <title>`. |
| Out of scope | Lease UI, remotes, `@affine/core`, convert in hub, `toDoc` restoring the card |

### Spaces and git

| | |
|---|---|
| **Wire** | Recon Actual: how a socket names `doc_id` (query, path suffix, first frame, or N sockets on the same Room). Bare `/collaboration/:workspace_id` = `doc:home`. Do not invent a second handshake that breaks M1. |
| **Export** | Bare `GET /api/block/:workspace/export` = home (M1). Per-doc export Actual in api-map (path or query). |
| **Seed catalog** | If empty after sync: folder `spec`, doc `home` → `doc:home`, doc `protocol` → second page. Same seed-once rule as pages. |
| **Move** | Catalog reparent live; git catches up on Flush. |
| **Empty folders** | Catalog only until a page lives under them. |

## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).

| # | id | Adds |
|---|---|---|
| 1 | [`step-recon-catalog`](#1-step-recon-catalog) | Gate + map: multi-doc wire, catalog schema, tree/header symbols, `git mv`, linked-doc Actuals; spike two docs. |
| 2 | [`step-spaces`](#2-step-spaces) | Hub + host: many pages per wiki; default path still home; export per doc. |
| 3 | [`step-catalog-crdt`](#3-step-catalog-crdt) | Catalog Y.Doc ops: seed, reparent, `gitPath`. Two tabs see moves. No tree chrome yet. |
| 4 | [`step-chrome`](#4-step-chrome) | Layout slots + product header (undo/redo, current page) on the open Store. |
| 5 | [`step-tree`](#5-step-tree) | Tree UI; click opens; drop reparents. Outline stays headings. |
| 6 | [`step-git-mv`](#6-step-git-mv) | Sidecar: catalog in the cut; two files; `git mv` on Flush when path changed. |
| 7 | [`step-links`](#7-step-links) | `embed-linked-doc` + export title/path/`venus:doc`; still resolves after a move. |
| 8 | [`step-verify`](#8-step-verify) | Close-out: person + Playwright; board `done`. |

---

## Steps

Do them in order (1–8). A step is not started until its `dependsOn` steps are done. Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

### 1. step-recon-catalog

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 1 |
| **id** | `step-recon-catalog` |
| **title** | Map multi-doc wire, catalog schema, tree, header, git mv |
| **dependsOn** | M3 closed |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** a decision and a map, not product UI. You know how a second `doc_id` rides on the wiki owner, the catalog guid + SQL uuid, node fields, tree/header imports, and that sidecar `commit_pins` will read `gitPath` from a catalog pin.

M4 dies if each page is a second hub owner, or if the tree scans git, or if undo is copied from `@affine/core`.

#### Work

1. Confirm [M3.state.yaml](../M3/M3.state.yaml) steps 1–9 `done`.
2. Spike (throwaway ok): two Y.Docs on the M0 `workspace_id` **without** keck. Prove home still hydrates; second doc round-trips; lease row count stays 1. Log the Actual framing (path / query / N sockets on one Room).
3. Fill [api-map.md](../api-map.md) **Names — catalog / tree / header**: catalog guid, SQL uuids, export URL per doc, node schema, order helper, tree mount, header undo APIs, `git mv` behavior, linked-doc title/path post-process.
4. Record: Room is `Map<doc_id, Doc>`; apply/persist per `docId`; owner still `workspace_id`. Bare collaboration + export URLs remain home.
5. Note M2 [fromDoc-review](../M2/fromDoc-review.md) S4/S5: two pages make linked-doc resolution and synced-doc inlining live — do not “fix” apply here; export must keep `venus:doc:` first.

#### Do not

- Ship the tree or header.
- Import `@affine/core`.
- Change M3 convert dialect.
- Write `wiki/` paths by hand as the live tree.
- Start lease / remotes.

#### Test scenarios

1. **Gate held**
   - **Given** this repo.
   - **When** you read [M3.state.yaml](../M3/M3.state.yaml).
   - **Then** steps 1–9 are `done` (including `step-verify`).
   - **How:** review the board. Fail closed if M3 is still in progress.
2. **Map complete**
   - **Given** this repo after this step.
   - **When** you read [api-map.md](../api-map.md) **Names — catalog / tree / header**.
   - **Then** Actuals are concrete: multi-doc wire, catalog guid + SQL uuid, second seed `docId` + `gitPath`, tree testid, header undo methods, per-doc export curl, `git mv` on Flush.
   - **How:** grep / review. Fail if cells are still empty or `recon:`.
3. **Spike two docs**
   - **Given** hub + Postgres (testcontainers or Compose).
   - **When** client A writes a map key on a second `doc_id`; client B is on that same doc; home is also connected.
   - **Then** B sees the value; home still has seed / prior bytes; `workspace_lease` has one owner for the M0 wiki.
   - **How:** `cargo test -p venus-hub --test recon` (or Actual recorded in api-map). Fail if the spike needs a second hub process per page.

---

### 2. step-spaces

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · **2** · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 2 |
| **id** | `step-spaces` |
| **title** | Many pages per wiki on the hub owner |
| **dependsOn** | `step-recon-catalog` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** product persist and hydrate for a second published page. Catalog space may exist empty. No tree.

Hub `Room` today is one `Doc` (`PAGE_DOC_ID`). SQL already keys `(workspace_id, doc_id)`. This step makes RAM match SQL.

#### Work

1. Room: `Map<doc_id, Doc>` (or equivalent). Hydrate/apply/persist/broadcast **per `docId`**. One persist task per doc or one task that drains every buffer — Actual. Do not pause persist.
2. Wire: implement api-map Actual so `SyncProvider.connect(docId, ydoc)` does not smash into home (`OctoBaseKeckProvider` `_gen` must not invalidate other sessions).
3. Export: keep `GET /api/block/:workspace/export` as home. Add per-doc export Actual. `live_export` must not always encode `PAGE_DOC_ID` only.
4. Host: mint the second seed doc **once** after sync (same seed-if-empty rule as home). Memory provider: both docs exist in one `TestWorkspace` without WS.
5. Dirty trigger already keys `doc_id` — a persist on the second page upserts a second `dirty` row; still one `dirty_wiki` / `jobs` grain.

#### Do not

- One hub lease per page.
- Change M1 two-tab / hydrate asserts except to stay green.
- Catalog ops or tree UI.
- `git mv`.

#### Test scenarios

1. **Second page persists**
   - **Given** hub + Postgres; second seed `docId` from api-map.
   - **When** you type (or set a Yjs key) on that page, wait ≥2s, restart **only** the hub.
   - **Then** a new client hydrates that page’s bytes. Home still hydrates.
   - **How:** hub WS/store test + host Vitest. Fail if the second page lived only in tab RAM.
2. **Home still hydrates**
   - **Given** Compose hub with both docs.
   - **When** you run M1 hydrate / two-tabs.
   - **Then** they still pass (`pnpm test:e2e:m1`). Bare export still decodes `doc:home`.
   - **How:** existing M1 suite. Fail if multi-doc broke the default path.
3. **One wiki owner**
   - **Given** hub A holds the M0 workspace; a client opens the second page.
   - **When** hub B tries the same `workspace_id`.
   - **Then** B is still 503 / not a second owner. Opening the second page did not take a second `workspace_lease` row.
   - **How:** extend `compose:ha` or crate lease test. Fail if `doc_id` sticky appeared.
4. **Export per doc**
   - **Given** both pages have distinct bytes.
   - **When** you curl bare export and the per-doc Actual URL.
   - **Then** bare = home; the other URL decodes the second guid, not home.
   - **How:** Vitest `snapshot.test.ts` (or sibling) + curl. Fail if both URLs are the same tree.

---

### 3. step-catalog-crdt

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · **3** · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 3 |
| **id** | `step-catalog-crdt` |
| **title** | Catalog Y.Doc: seed, reparent, gitPath |
| **dependsOn** | `step-spaces` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** the collaborative tree as data. No folder UI. Host (or tests) can reparent and see `gitPath` change.

#### Work

1. Schema module: `nodes` map as in [datamodel](../datamodel/crdt.md#catalog). `gitPath` derived from ancestor `name`s (docs get `.md` in git; catalog `name` Actual). `parentId` null = wiki root.
2. Ops: `createFolder`, `createDoc` (mint `docId` once + `createDoc` on the workspace), `rename`, `reparent`, `setOrder`. Reparent does **not** touch page bodies.
3. Seed-once: if catalog has no nodes after sync, write `spec/` + `home` (`doc:home` → `spec/home.md`) + `protocol` (Actual → `spec/protocol.md`).
4. Connect the catalog Y.Doc through `SyncProvider` like a page. Two tabs share it.
5. Vitest can run these ops on `MemoryNoopProvider` (no Docker).

#### Do not

- Mount a tree.
- `git mv` here (path is catalog-only until step 6).
- Store sibling order in git.
- Put folder nodes on `affine:page`.

#### Test scenarios

1. **Seed catalog**
   - **Given** empty catalog + seeded home/protocol pages (memory ok).
   - **When** seed-once runs.
   - **Then** nodes include folder `spec`, doc `home` with `docId=doc:home` and `gitPath=spec/home.md`, doc `protocol` with Actual `docId` and `gitPath=spec/protocol.md`. A second seed does not duplicate.
   - **How:** `pnpm --filter @venus/web test` under `src/host/catalog/`. Fail if seed always `add`s.
2. **Reparent**
   - **Given** that seed.
   - **When** you `createFolder` `design` at root and reparent `protocol` under it.
   - **Then** `protocol.gitPath` is `design/protocol.md` (or Actual). Home `gitPath` unchanged. Protocol page Y.Doc body bytes unchanged.
   - **How:** same Vitest. Fail if reparent `Y.applyUpdate`s the page.
3. **gitPath derived**
   - **Given** a nested folder `spec/crdt`.
   - **When** you reparent `home` under it.
   - **Then** `gitPath` is `spec/crdt/home.md` (or Actual). Rename of folder `spec` → `SPEC` updates descendant paths.
   - **How:** same Vitest.
4. **A→B catalog**
   - **Given** Compose hub; two tabs.
   - **When** A reparents `protocol` (ops API or test hook).
   - **Then** B’s catalog shows the new parent / `gitPath` without reload.
   - **How:** Playwright `e2e/m4-tree.spec.ts` (ops via `window.__VENUS_*` until the tree exists) or a hub Yjs test. Fail if catalog was tab-local.

---

### 4. step-chrome

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · **4** · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 4 |
| **id** | `step-chrome` |
| **title** | Layout + product header: undo/redo, current page |
| **dependsOn** | `step-catalog-crdt` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** the slice the tree will sit in. Header works on the open page even if the tree is still a stub slot.

#### Work

1. Layout: header top; **tree slot** left; editor; outline right. Flush / git-log chrome stay host controls (header or existing bar — Actual). Markdown pane does not take the tree slot.
2. Header `data-testid="venus-header"`: Undo / Redo buttons (`venus-undo` / `venus-redo`) call `store.undo()` / `store.redo()` on the **open** Store; disabled when `canUndo` / `canRedo` (or Actual observables) are false.
3. Current page: catalog `name` and/or `gitPath` (`venus-page-title`). Switching the open doc (test hook until step 5) rebinds header + editor + outline + md pane to that Store.
4. Vitest: `mount-editor.js` has no header/catalog imports. No `@affine/core` in `@venus/web`.

#### Do not

- History list / `undoManager` labels.
- Lease holder chip (M5).
- Import AFFiNE’s workbench header.

#### Test scenarios

1. **Header undo**
   - **Given** the app with home open; type a unique word in the note.
   - **When** you click `[data-testid=venus-undo]`.
   - **Then** the word is gone. Redo brings it back. After undo, the Undo button is disabled if `canUndo` is false (seed constructor history still reset). Keyboard ⌘Z / Ctrl+Z still matches.
   - **How:** `e2e/m4-header.spec.ts` (memory ok). Fail if undo is a second stack.
2. **Current page**
   - **Given** catalog seed (home + protocol).
   - **When** the open doc is protocol (test hook or later tree click).
   - **Then** `[data-testid=venus-page-title]` contains that catalog name or `gitPath`, not only “Venus” from home.
   - **How:** same spec or Vitest. Fail if the title is hard-coded `doc:home`.
3. **Layout**
   - **Given** `/`.
   - **When** you inspect the shell.
   - **Then** header is present; a tree host `[data-testid=venus-tree]` exists (may be empty until step 5); outline still lists **headings** of the open page (`outline-block-preview-h1`), not folder names.
   - **How:** Playwright. Fail if outline was replaced by a wiki TOC.
4. **Seam holds**
   - **Given** `mount-editor.js` / `editor-container.js` / `boot.js`.
   - **When** you search for catalog / tree / header / git imports.
   - **Then** none.
   - **How:** extend `sync-provider.test.ts` (or sibling). Fail if the editor container owns chrome.

---

### 5. step-tree

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · **5** · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 5 |
| **id** | `step-tree` |
| **title** | Folder tree: click opens, drop reparents |
| **dependsOn** | `step-chrome` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** the wiki TOC people use. Data = catalog. Drop = `reparent`.

#### Work

1. Mount [CatalogTree](../components/frontend/crdt-tree/) (`@headless-tree/react`). `data-testid="venus-tree"`. `dataLoader` from catalog nodes; drop → `reparent` + `setOrder`. No AFFiNE explorer. Do not let the library own the tree.
2. Click a doc → that Store is open (editor, outline, header, md pane).
3. Drop a doc onto a folder → catalog `reparent` (live). Optional: create folder / create page controls (needed for dogfood; at least one way to add a page besides seed).
4. Two tabs: drop in A appears in B without reload.
5. Accessible: keyboard focus / aria tree Actual. Do not scrape `wiki/` to build rows.

#### Do not

- Docusaurus / VitePress / AFFiNE explorer.
- Let headless-tree (or any UI) keep the canonical children list.
- `git mv` on drop (wait for Flush).
- Treat outline headings as folders.

#### Test scenarios

1. **Lists nodes**
   - **Given** seeded catalog.
   - **When** you open `/`.
   - **Then** `[data-testid=venus-tree]` shows `spec`, `home`, and the protocol page name (Actual). Outline still has H1 `Why Venus` when home is open, not `spec`.
   - **How:** `e2e/m4-tree.spec.ts`. Fail if the tree is the heading list.
2. **Click opens**
   - **Given** that tree.
   - **When** you click protocol.
   - **Then** the editor shows that page (not home seed H1 only); header title matches; undo binds that Store.
   - **How:** same spec. Fail if click only scrolls outline.
3. **Drop reparents**
   - **Given** folder `design` (seed extra or create-folder control).
   - **When** you drop protocol onto `design`.
   - **Then** catalog `gitPath` is `design/protocol.md` (or Actual). Second tab sees the new parent without reload. Page body unchanged.
   - **How:** Playwright drag-and-drop or the tree’s published drop API. Fail if drop rewrote markdown / Yjs page bytes.
4. **Outline not a wiki TOC**
   - **Given** home open.
   - **When** you compare tree vs outline.
   - **Then** outline testids stay `outline-block-preview-h1` / `h2`; M0 outline specs still pass.
   - **How:** `pnpm test:e2e` m0-outline + m4-tree. Fail if folders appear as outline items.

---

### 6. step-git-mv

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · **6** · [7](#7-step-links) · [8](#8-step-verify)

| | |
|---|---|
| **n** | 6 |
| **id** | `step-git-mv` |
| **title** | Flush pins catalog; git mv when gitPath changed |
| **dependsOn** | `step-tree` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** cloneable folders that match the catalog after publish. Sidecar today writes only `spec/home.md` and skips other `doc_id`s.

#### Work

1. Cut: include catalog `doc_id` in S when its `dirty.clock` > `last_flushed` **or** any page is dirty (so `gitPath` is consistent). Pin catalog Yjs bytes like a page. COMMIT before `fromDoc`.
2. Convert: `fromDoc` each **page** pin whose body clock moved. Read `gitPath` from the catalog pin. Do not `fromDoc` the catalog.
3. If a page’s new `gitPath` ≠ path at last commit: `git2` `rename` / `git mv` the `.md` (and keep sidecar keyed by `docId` under `.venus/ids/<docId>.json`).
4. Body unchanged + path changed → **no** `fromDoc`; still `git mv` + one autocomment commit.
5. `commit_pins` writes every dirty page, not only home. `last_flushed` rows for those `doc_id`s (including catalog clock).
6. Host Flush unchanged. `mount-editor` still ignorant.

#### Do not

- `git mv` on catalog drop before Flush.
- Markdown in Postgres.
- Convert inside the hub.
- Encode sibling order as git file names (`01-home.md`).

#### Test scenarios

1. **Two files**
   - **Given** seed catalog + both pages; empty or existing `wiki/`.
   - **When** Flush (claim → cut → commit).
   - **Then** `wiki/spec/home.md` and `wiki/spec/protocol.md` (Actual) exist; sidecars `.venus/ids/doc:home.json` and `.venus/ids/<protocol>.json`; autocomment commit.
   - **How:** `cargo test -p venus-sidecar --test flush` (extend). Fail if protocol is skipped because `doc_id != PAGE_DOC_ID`.
2. **Move then Flush**
   - **Given** protocol committed at `spec/protocol.md`; catalog reparent to `design/protocol.md`.
   - **When** Flush.
   - **Then** git log has a commit that **renames** (or deletes + adds with identical blob) to `design/protocol.md`; `spec/protocol.md` is gone; clone elsewhere shows `design/protocol.md`.
   - **How:** sidecar test + optional `e2e/m4-move.spec.ts`. Fail if both old and new files exist with copies.
3. **Catalog-only no fromDoc**
   - **Given** protocol body clock = `last_flushed`; only catalog dirty (path change).
   - **When** Flush.
   - **Then** the markdown blob is byte-identical to HEAD’s old file (rename only). Convert is not invoked for that page (spy / no temp pin convert). Home not rewritten if not dirty.
   - **How:** sidecar test. Fail if a second dialect rewrite jittered the file.
4. **Clone matches**
   - **Given** after the move Flush.
   - **When** you read the clone without the hub.
   - **Then** folders match catalog `gitPath`s. Order of siblings is **not** asserted from git.
   - **How:** same flush test / verify script.

---

### 7. step-links

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · **7** · [8](#8-step-verify)

| | |
|---|---|
| **n** | 7 |
| **id** | `step-links` |
| **title** | embed-linked-doc + markdown path; survives git mv |
| **dependsOn** | `step-git-mv` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** a card from home to protocol that export and git can name. M2 synthetic `untitled` + `./workspace/…/doc:lease` is not enough once a catalog exists.

#### Work

1. Insert `affine:embed-linked-doc` with `pageId =` protocol `docId` (seed or e2e).
2. `fromDoc` post-process: link text = catalog title/name; URL = path relative to the **current** file’s `gitPath`; keep `<!-- venus:doc:<id> -->`. Do not drop the id comment.
3. After `git mv`, next Flush: markdown URL updates; comment id unchanged. WYSIWYG card still opens/resolves that `pageId` (BlockSuite, not path).
4. Re-read [fromDoc-review S4](../M2/fromDoc-review.md): `pageId` allowlist still holds. Do not implement M6 `toDoc` → card.
5. `titleMiddleware` should see `workspace.meta.docMetas` for real titles when available (Actual).

#### Do not

- Path-only identity.
- Whole-file `toDoc` onto the live doc.
- Synced-doc transclusion as a product surface (S5 still open; do not seed `embed-synced-doc`).

#### Test scenarios

1. **Card + comment**
   - **Given** home contains `affine:embed-linked-doc` to protocol.
   - **When** `fromDoc(home)`.
   - **Then** markdown has the catalog title (or protocol page title), a relative path to `spec/protocol.md`, and `<!-- venus:doc:<protocolId> -->`.
   - **How:** Vitest mdgate + catalog fixture; `e2e/m4-link.spec.ts` for the card visible. Fail if export is still `./workspace/<uuid>/doc:protocol` with `untitled` only.
2. **Resolves after move**
   - **Given** that card; reparent protocol; Flush.
   - **When** you open home (reload ok).
   - **Then** the linked-doc card still targets the same `pageId`; git markdown comment still has that id; URL path matches new `gitPath`.
   - **How:** `e2e/m4-link.spec.ts` + sidecar markdown assert. Fail if the card is missing or points at a new space.
3. **Export path from catalog**
   - **Given** protocol at `design/protocol.md`.
   - **When** Flush home (dirty because title middleware / link path).
   - **Then** `wiki/spec/home.md` (or Actual) contains `design/protocol.md` (relative form Actual, e.g. `../design/protocol.md` if needed).
   - **How:** sidecar / Vitest. Fail if the file still says `spec/protocol.md`.

---

### 8. step-verify

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · **8**

| | |
|---|---|
| **n** | 8 |
| **id** | `step-verify` |
| **title** | Close-out: two pages, link, move, header undo |
| **dependsOn** | `step-links` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** board `done`. Person + clone + smoke.

#### Work

1. Runbook: M4 close-out — two pages in the tree, link, drop to a folder, Flush, clone `wiki/`, header undo/redo vs keyboard.
2. Person in Chrome or Firefox.
3. `pnpm test:e2e:m1`, M2 pane, M3 flush/live, new `m4-*.spec.ts`.
4. Mark [M4.state.yaml](./M4.state.yaml) steps 1–8 `done` with evidence.
5. Re-read [fromDoc-review](../M2/fromDoc-review.md) S4/S5 note (dogfood starts after this); do not “close” S1–S7 here unless already green.

#### Do not

- Start M5 lease in this step.
- Record remotes as required evidence (optional note only).
- Close M4 if clone folders do not match the tree after Flush.

#### Test scenarios

1. **Exit held**
   - **Given** Compose `postgres` + `hub` + `web` + sidecar.
   - **When** you create/open two pages, insert one linked-doc, move a page to another folder, Flush, clone `wiki/`.
   - **Then** git tree matches catalog `gitPath`s; the link still resolves; header undo/redo matches keyboard on the open page.
   - **How:** `pnpm test:e2e:m4` (or Actual) + clone script. Fail if any exit bullet is demo-only.
2. **Smoke**
   - **Then** `m4-tree`, `m4-header`, `m4-move`, `m4-link` pass; `pnpm test:e2e:m1`, M2 pane, `pnpm test:e2e:m3` still pass.
   - **How:** those commands.
3. **Manual path**
   - **Then** runbook M4 close-out holds (tree, drop, Flush, clone, undo button, outline still headings).
   - **How:** person + evidence on the board (date + browser).
4. **Prior milestones green**
   - **Then** memory `pnpm test:e2e` (m0 + m2) still passes with the new chrome.
   - **How:** `pnpm test` + `pnpm test:e2e`. Fail if the tree requires Docker in the default suite.

---

## After M4

[M5 — Lease + freeze](../venus-implementation-plan.md#m5--lease--freeze-week): acquire / heartbeat / readonly banner; header may then show **who holds** the lease. Same pin helper as M3; keep the pin as `T0`.

**Not this exit:** record wiki remote + product remote@branch ([code-bind](../Agents/code-bind.md)). Do not delay M5 for the analyzer.

**v1 dogfood** ([implementation plan](../venus-implementation-plan.md#v1-dogfood--document-the-product)): after this board is `done`, managers can navigate folders and author in the wiki; clones read markdown. Intention-changing edits still wait for M6 comment-commit.

## Order of work (calendar)

| When | Steps |
|---|---|
| Day 1 | 1 `step-recon-catalog` |
| Day 2 | 2 `step-spaces` |
| Day 3 | 3 `step-catalog-crdt` |
| Day 4 | 4 `step-chrome` → 5 `step-tree` |
| Day 5 | 6 `step-git-mv` |
| Day 6 | 7 `step-links` → 8 `step-verify` |

If M3 is still open, **stop**. Do not fake a second page as two git paths on one Y.Doc.

## Risks

| Risk | What to do in M4 |
|---|---|
| Gate skipped | Step 1 **Gate held** fails closed |
| `doc_id` sticky / second hub owner per page | Step 2 **One wiki owner**; HA grain |
| Second `connect()` kills home WS (`_gen`) | Step 2 provider sessions |
| Bare export no longer home | Step 2 **Home still hydrates** / **Export per doc** |
| Tree reads `wiki/` | Step 5 data = catalog; step 6 git lags live |
| `git mv` on every drop | Step 6 only on Flush |
| `commit_pins` still home-only | Step 6 **Two files** |
| Path as link identity | Step 7 `venus:doc:` + `pageId` |
| `@affine/core` header | Step 4 **Seam holds** |
| Outline replaced by folders | Step 5 **Outline not a wiki TOC** |
| Catalog is an `affine:page` | Step 3 schema; hub stores a plain Y.Doc |
| `toDoc` restores the card | Forbidden; M6 |
| Remotes / AB5 as M4 DoD | After this exit |
| S5 synced-doc inlining | Do not seed it; note in verify |

## Handoff to M5

M5 may assume:

- Two (or more) published spaces + `venus:catalog` on the same wiki owner.
- Tree + header exist; current page is visible; undo/redo is host chrome.
- Flush `git mv`s; clone folders match catalog after publish.
- Linked-doc export has stable `docId` comments.
- Freeze / `store.readonly` / CodeMirror / lease holder chip are **new**.
- Hub still has no convert and does not write `jobs`.

M5 exit is: second user cannot type in WYSIWYG during the lease; they see who holds it. M4 exit is “two pages, one link, move, git matches, header undo.”

## Invariants (M4 only)

1. Path is not identity. `docId` is.
2. Catalog CRDT is the live tree; git is the share layout after publish.
3. Moves do not rewrite page bodies. `git mv` on Flush, not on drop.
4. One hub owner per `workspace_id`. Many `doc_id`s inside that room.
5. Outline is headings. Tree is folders/pages.
6. Header undo is the open Store’s Yjs undo stack, not a timeline, not `@affine/core`.
7. Linked-doc keys by `docId`; markdown also carries path for humans/agents.
8. `mount-editor` stays unaware of catalog, tree, header, and git.
9. No lease, no comment-commit why, no remotes required.
10. Memory default stays Docker-free for m0/m2 e2e.
