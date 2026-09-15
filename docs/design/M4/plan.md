# M4 — Folder tree + links + product header

| | |
|---|---|
| **planId** | `m4-folder-tree` |
| **Milestone** | [M4 in the implementation plan](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks) |
| **Duration** | About 1–2 weeks |
| **Encoding** | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B) |
| **Board** | [M4.state.yaml](./M4.state.yaml) |

Parent design: [venus-design.md](../venus-design.md) ([folder tree](../venus-design.md#folder-tree-table-of-contents), [cross-document references](../venus-design.md#cross-document-references)). Catalog: [datamodel catalog](../datamodel/crdt.md#catalog). Tree: [CRDT tree](../components/frontend/crdt-tree/). Git: [datamodel git](../datamodel/git.md). Pin + `git mv`: [LiveSnapshot](../LiveSnapshot/README.md). Linked-doc export: [MDGate subset](../MDGate/subset.md#linked-doc-stable-form). Create / `{uuid}.md` / rename / DB map: [page-identity](../datamodel/page-identity.md) ([hub](../components/backend/hub/page-identity.md)). Header: [implementation plan — product header](../venus-implementation-plan.md#product-header--venus-chrome-with-the-folder-tree). Hub: [M3.0](../M3.0/README.md), [M3.0 HA](../M3.0/high-availability.md), [backend hub](../components/backend/hub/). Snapshotter: [M3](../M3/README.md). Dataflow: [architecture.md](../architecture.md). Envelope: [rpc.md](../rpc.md). Words: [glossary.md](../glossary.md). Installed symbols: [api-map.md](../api-map.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

**Gate (do not skip):** do not start step work in the product repo until [M3](../M3/README.md) is **closed** (board steps 1–9 `done`). This folder existing is not permission to mint a second page space, ship a wiki TOC, or `git mv`.

## Story

As an implementer I need **more than one page** in the wiki: a live **catalog CRDT** (folders, order, names, `gitPath`), a **tree** I can drop to reparent, and a **thin product header** (undo / redo / current page) on that same chrome. Publish must **`git mv`** when the catalog path changed. An **`affine:embed-linked-doc`** keys by `docId`, so a move does not break the card; markdown export uses catalog title + relative path + `<!-- venus:doc:… -->`. Venus still has no lease freeze or comment-commit why.

If path is identity, every rename is a broken link. If the tree is a docs-framework TOC, two people cannot move folders. If undo lives only on the keyboard, managers never find it. If git does not `git mv`, clone folders lie.

## Exit

All of these must be true at once:

1. **Two published pages** on the M0 wiki: seed `doc:home` plus one page **created through the API** ([page-identity](../datamodel/page-identity.md) — mint uuid once, first file `{uuid}.md`, tree rename). Refresh / second tab hydrates both. Lease grain stays `workspace_id` (one live hub owner for the wiki). Do not pre-seed `doc:protocol`.
2. **Catalog CRDT** space `venus:catalog` (api-map Actual) on the hub: folders, docs, `parentId`, sibling `order`, derived `gitPath`. Moves = reparent + order. They do **not** rewrite page bodies.
3. **Tree UI** reads only that catalog. Click opens the page in the editor. Drop reparents (live CRDT; other tab sees it without reload). Outline stays the **in-page heading TOC**, not a second wiki tree.
4. **Product header** on the same slice: Undo / Redo on the **open** page’s `Store` (`store.undo()` / `store.redo()`, disabled by subscribing to `store.history.canUndo$` / `canRedo$`). **`venus-page-title` = catalog `name`** of the open node. Not `@affine/core`. Not a history list of Yjs transactions.
5. **Layout:** header top; folder tree left; page editor; outline right. Markdown pane may remain host chrome; it must not occupy the tree slot or become the wiki TOC.
6. **Publish includes `git mv`.** Flush pins **catalog + dirty pages** in the same cut. If `gitPath` changed and the body clock did not, **`git mv` only** (no `fromDoc` of the unchanged page). Clone of `wiki/` shows the new folders. Sibling **order** is not in git. Flush always rewrites `wiki/.venus/pages.yaml` from the **catalog pin** (`pages:` + `folders:`) and replaces `page_identity` from that same decode (YAML is not identity).
7. **`affine:embed-linked-doc`** with `pageId = docId` plus markdown round-trip of **export** form: catalog title, path relative to the current file, `<!-- venus:doc:<id> -->`. After a move, the card still resolves; the next snapshot’s markdown path matches the new `gitPath`. Recreating the card from markdown is still M6 apply, not adapter `toDoc`.
8. Header undo/redo matches keyboard undo (⌘Z / Ctrl+Z) on the open page.
9. [api-map.md](../api-map.md) Actual column is filled for every catalog / tree / header / `git mv` / linked-doc name the code uses.
10. Existing **M1 Playwright**, M2 export / pane, and M3 Flush / live-during-flush stay green. `mount-editor` still imports neither catalog nor git.
11. **Wire A** holds without a SharedWorker. Where `SharedWorker` exists, two pages in one browsing profile share the worker’s A sockets; where it does not, per-tab sockets still pass 1–10.
12. **Create → rename → map** e2e ([page-identity](../datamodel/page-identity.md)) is green (includes **delete**: reject non-empty folder; leaf page gone from tree, DB, git). M4 is not `done` without it.

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
| Convert / catalog ops inside the hub | Hub stays apply + broadcast + persist + export/blob. Delete **authorization** is the host op. Catalog **pin** walk is `venus-sidecar` (Rust + y-octo). Not merge. |
| Auth / ACL | Leftover ([Identity](../venus-implementation-plan.md#identity-v1)) |
| Frame multiplex (C) / path suffix (B) | Wire is **A** (`?doc=`). Do not wrap y-protocols. |
| Service Worker as the collab socket | Different API. Detect **`SharedWorker`**. SW is not the fallback and not the optimization. |

Do not treat outline as the wiki tree. Do not `git mv` on every catalog keystroke (only on publish / Flush). Do not rewrite page Yjs on reparent.

## Constraints

1. **Thin host.** Same Vite + React app. Tree + header are host chrome (like Flush and the markdown pane), not BlockSuite widgets. `mount-editor.js` / `editor-container.js` / `boot.js` do not import catalog, tree, header, or git.
2. **Path is not identity.** Links and catalog docs key by `docId` (hub space / BlockSuite guid). `gitPath` is derived and cached. Renames do not mint a new space.
3. **Mint once.** `collection.createDoc()` / hub space ids are created **once**, then synced. Never invent the same `space_id` independently on two devices ([implementation plan — collection](../venus-implementation-plan.md#1-collection--hub-workspace)).
4. **Wiki sticky unchanged.** Owner / lease grain is `workspace_id`, not `doc_id`, not cookie. Many pages live **on that owner**. [M3.0 HA rooms](../M3.0/high-availability.md#rooms).
5. **SQL `doc_id` is a UUID.** Home = v5 of `doc:home` (`PAGE_DOC_ID`). Catalog = v5 of `venus:catalog` (`CATALOG_DOC_ID`). Created pages **mint uuid v4** (= SQL `doc_id` = BlockSuite `createDoc` id). Hub must persist **any** of those ids, not only `PAGE_DOC_ID`.
6. **Catalog is not an `affine:page`.** It is a small Y.Doc (`Y.Map` of nodes). Do not hang folders on the published block schema.
7. **One apply queue per `docId`**, one persist buffer per `docId`, inside the **one** room for the wiki. Not per socket. **One persist tick per Room** (~1s, today’s hub loop) drains every non-empty buffer; each `INSERT` keeps its `doc_id`. Do not spawn a persist task per doc. Catalog vs page is that `doc_id` (not decoding update bytes). Hub persist stays opaque; **Rust sidecar** walks the catalog pin at Flush.
8. **M1 default collab path stays.** `/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` with no `?doc=` still hydrates `doc:home`. Second docs (catalog, created pages) use `?doc=<sql uuid>` on **that same path**. **Doc export is gRPC** (`Hub.ExportDoc`, omit `doc_id` = home). GET `/api/block/:workspace/export` is advertisement JSON (no Yjs). Envelope: [rpc.md](../rpc.md).
9. **Pin then convert.** Catalog bytes ride in the **same MVCC cut** as dirty pages so `gitPath` matches the files you write ([LiveSnapshot](../LiveSnapshot/README.md)). Convert **page** bodies with the M3 Rust `fromDoc` (JS CLI oracle). Catalog pin: **Rust y-octo hydrate + walk `nodes`** in `venus-sidecar` — YAML, `page_identity`, `gitPath` / `git mv`. **Do not** `fromDoc` the catalog. Catalog-only dirty → walk + `git mv` / YAML / SQL, **no** `fromDoc` of an unchanged body. Cut still released before convert.
10. **Dirty clocks.** Catalog persist upserts `dirty(workspace_id, catalog_sql_uuid, clock)` via the existing trigger. One `jobs` row per wiki still. Sidecar `commit_pins` must stop skipping every `doc_id` that is not home.
11. **Tree data = catalog only.** Drop = catalog reparent. Do not scan `wiki/` for the live tree. Do not use AFFiNE explorer.
12. **Header binds the open `Store`.** Switching pages rebinds undo/redo (new `$` subscribe) and the title. Same stack as ⌘Z / Ctrl+Z. No labeled history.
13. **Pin `yjs` 13.6.32** and BlockSuite **0.22.4**. Headings stay `affine:paragraph` + `type` h1/h2.
14. **Memory default.** Vitest and `pnpm test:e2e` stay green without Docker. Tree / header e2e may use memory (like M0/M2). Hub multi-doc and `git mv` need Compose. Unset sync env: no second space on the hub, no git write.
15. **No `@affine/core`.** No nbstore. No convert in the hub. No lease UI.
16. If this plan disagrees with [api-map.md](../api-map.md) after recon, **the map wins**. If it disagrees with [M3.0 HA](../M3.0/high-availability.md) on owner grain, **HA wins**.
17. **SharedWorker is step 8 only.** Steps 2–7 ship per-tab A sockets. The worker holds those same URLs; it does not become a mux. `Y.Doc` stays in the tab. Memory mode has no worker.

## Target tree

Only create what M4 needs. Do **not** add `packages/review`, lease types, or apply tests. Do **not** put catalog merge into `crates/venus-hub` beyond storing another `doc_id`.

```text
Venus/
  proto/venus/                      # envelope + Hub.ExportDoc / ListDocs (internal gRPC)
  crates/venus-hub/                 # Room: Map<doc_id, Doc>; ExportDoc per doc; GET /export = advertisement
  crates/venus-sidecar/             # y-octo: page fromDoc + catalog nodes walk; git2 mv
  apps/web/
    src/host/
      catalog/                      # Y.Map schema + ops; not mount-editor
        schema.js
        ops.js                      # create / rename / reparent / deleteNode
        git-path.js
        CatalogTree.tsx           # @headless-tree/react; drop → catalog ops
      chrome/                       # header undo/redo + current page
        mount-header.js
      mdgate/from-doc.js            # linked-doc post-process uses catalog title/path
      providers/
        octobase-keck-provider.js   # N sessions; ?doc=; _gen per session
        keck-shared-worker.js       # SharedWorker script (step 8); holds A sockets
    src/App.tsx                     # header / tree / editor / outline
    e2e/
      m4-tree.spec.ts
      m4-header.spec.ts
      m4-move.spec.ts
      m4-link.spec.ts
      m4-create-rename.spec.ts      # required DoD; datamodel/page-identity.md
      m4-shared-worker.spec.ts      # step 8; skip if no SharedWorker
  docs/design/api-map.md            # catalog Actuals in step 1
  docs/design/datamodel/page-identity.md
  docs/design/components/backend/hub/page-identity.md
  docs/design/M4/
    …
```

`packages/catalog` is **not** an M4 workspace package. Schema + ops live in **`apps/web/src/host/catalog/`** (same as mdgate). Do not add an empty package “for later.”

## Binding (what you are proving)

```text
Tab A / Tab B
        │  catalog Y.Doc  venus:catalog     (folders, order, gitPath)
        │  page Y.Doc     doc:home
        │  page Y.Doc     <uuid>            (create API; not pre-seeded)
        ▼
SyncProvider  (kind octobase alias)
        │  AFFiNE WS  /collaboration/<workspace>              → home
        │  AFFiNE WS  /collaboration/<workspace>?doc=<uuid>   → catalog / created page
        │  (step 8: SharedWorker may own those sockets; tabs still hold Y.Doc)
        ▼
Venus hub  one owner per workspace_id
        │  apply / broadcast / persist per docId
        ▼
Postgres  crdt_* / dirty  (home, created pages, catalog)
          page_identity at Flush (sidecar, from catalog pin)

Flush / idle
        │  claim → MVCC cut of S (dirty pages ∪ catalog)
        │  fromDoc dirty page pins; git mv if gitPath changed
        ▼
wiki/  spec/home.md
       spec/<uuid>.md     → after tree rename e.g. spec/protocol.md
       .venus/pages.yaml  ← pages + folders from catalog pin (same decode as page_identity)
       .venus/ids/<docId>.json
```

Ids (Intent; catalog Actual locked; second page is create-API, not seed):

```text
TestWorkspace.id     =  77e4a2b1-8b40-5979-a73c-fd4477216d00
doc:home             =  existing page; SQL PAGE_DOC_ID 395cd07b-bdb1-5f54-ada8-e9a3fabb6a20
venus:catalog        =  catalog Y.Doc guid; SQL CATALOG_DOC_ID 4fe5c16e-4be3-5700-a456-ecc8e86cdf1a
created page         =  uuid v4 = SQL doc_id = BlockSuite createDoc id; first gitPath {folder}/{uuid}.md
gitPath home         =  spec/home.md until renamed (then filename filter; parent stays folder:spec)
```

## Chosen stack

Locked in [step-recon-catalog](#1-step-recon-catalog). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.

| Piece | Intent |
|---|---|
| Live CRDT | Unchanged hub + `OctoBaseKeckProvider` (`kind: 'octobase'` alias). **Many `doc_id`s per room.** **Wire A:** N sockets, one Room, same `/collaboration/:workspace_id`. Socket names `doc_id` with query `?doc=<sql uuid>`. Omit `doc` = home (M1). Not path suffix (B). Not frame multiplex (C). **Persist:** one tick per Room drains every `doc_id` buffer. |
| Catalog | Plain Y.Doc, `Y.Map` nodes. Fields: `id`, `kind` (`folder` \| `doc`), `name`, `parentId`, `order`, `docId?`, `gitPath`. Create: `{uuid}.md`; rename in tree. |
| Order | Fractional index **string** on each catalog node. **Not git.** Helper pin is step 3 (`ops.js`). |
| Tree UI | **`@headless-tree/react@1.7.0`** view over the catalog Y.Doc. Data loader reads `Y.Map`; drop calls catalog `reparent` / `setOrder`. Not AFFiNE explorer. Design: [CRDT tree](../components/frontend/crdt-tree/). |
| Header | Host chrome: `store.undo()` / `store.redo()`; **subscribe** to `store.history.canUndo$` / `canRedo$` (do not poll). **`venus-page-title` = catalog `name`**. |
| Linked-doc | `affine:embed-linked-doc` `pageId = docId`. **Git:** `[catalog name](posix-relative gitPath)` + `<!-- venus:doc:<docId> -->`. **Live card:** `docMetas.title` = catalog `name` on create/rename/seed. Never `./workspace/<ws>/…` in `wiki/`. |
| Git | Same sidecar. `git2` `git mv` when catalog `gitPath` ≠ last committed path. Autocomment: one dirty page → `snapshot: <H1>` (M3); several / catalog-only → `snapshot:`. **Catalog pin:** Rust y-octo walk in `venus-sidecar` → YAML + `page_identity` + `gitPath`. Page pins: M3 `fromDoc`. Do not `fromDoc` the catalog. |
| Tab sockets | Default: one TCP per **connected** Y.Doc in that tab (catalog + current page). `OctoBaseKeckProvider` `_gen` is **per session**, not global. |
| SharedWorker | Last product step ([`step-shared-worker`](#8-step-shared-worker)): if `typeof SharedWorker === 'function'`, one worker per origin+workspace holds those **same A sockets**; tabs `postMessage` updates. `Y.Doc` + BlockSuite stay in the tab. Missing API → per-tab A (required fallback). **Not** a Service Worker. |
| Out of scope | Lease UI, remotes, `@affine/core`, convert in hub, `toDoc` restoring the card, y-protocols envelope (C) |

### Spaces and git

| | |
|---|---|
| **Wire** | **A1+B+C1, locked.** N sockets on one Room. `WS /collaboration/:workspace_id` = home. `WS /collaboration/:workspace_id?doc=<sql uuid>` = that `doc_id`. Guid in `?doc=` is **400**. Vanilla `y-protocols` / `AFFiNE` frames. SharedWorker (step 8) does not change this URL. |
| **Export** | **gRPC `Hub.ExportDoc`**. `doc_id` = SQL uuid; omit = home. GET `/api/block/:workspace/export` = advertisement (**home + catalog** + `?doc=` / gRPC). GET `?doc=` = 400 `export_http_disabled`. **`ListDocs`** = that pair + `page_identity` rows (lags until Flush). No HTTP Yjs. [rpc.md](../rpc.md). |
| **Seed catalog** | If **`folder:spec`** or **`doc:home`** missing after sync: write those two (`spec` + `home` → `doc:home` / `spec/home.md`) **only**. No second seed page. User folders mint `folder:` + uuid v4. Further pages: [page-identity](../datamodel/page-identity.md). |
| **Move** | Catalog reparent live; git catches up on Flush. |
| **Empty folders** | No git directory until a page lives under them. YAML `folders:` still lists them. |

## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).

| # | id | Adds |
|---|---|---|
| 1 | [`step-recon-catalog`](#1-step-recon-catalog) | Gate + map: wire **A** (`?doc=`), catalog schema, tree/header symbols, `git mv`, linked-doc Actuals; spike two docs. |
| 2 | [`step-spaces`](#2-step-spaces) | Hub + host: many pages per wiki; bare path still home; `?doc=` bind; gRPC `ExportDoc` per doc. Per-tab sockets. |
| 3 | [`step-catalog-crdt`](#3-step-catalog-crdt) | Catalog Y.Doc ops: seed, reparent, `gitPath`, `deleteNode` (reject if children). Two tabs see moves. No tree chrome yet. |
| 4 | [`step-chrome`](#4-step-chrome) | Layout slots + product header (undo/redo, current page) on the open Store. |
| 5 | [`step-tree`](#5-step-tree) | Tree UI; click opens; drop reparents. Outline stays headings. |
| 6 | [`step-git-mv`](#6-step-git-mv) | Sidecar: catalog in the cut; two files; `git mv` on Flush when path changed. |
| 7 | [`step-links`](#7-step-links) | `embed-linked-doc` + export title/path/`venus:doc`; still resolves after a move. |
| 8 | [`step-shared-worker`](#8-step-shared-worker) | Optional **SharedWorker** in front of A. Feature-detect; fallback per-tab. Not Service Worker. Not mux. |
| 9 | [`step-verify`](#9-step-verify) | Close-out: person + Playwright; board `done`. |

---

## Steps

Do them in order (1–9). A step is not started until its `dependsOn` steps are done. Steps 2–7 must work **without** a SharedWorker (per-tab A sockets). Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

### 1. step-recon-catalog

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-shared-worker) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 1 |
| **id** | `step-recon-catalog` |
| **title** | Map multi-doc wire, catalog schema, tree, header, git mv |
| **dependsOn** | M3 closed |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** a decision and a map, not product UI. **Wire A is locked:** N sockets, `?doc=<sql uuid>` on the existing workspace path. You still fill catalog guid + SQL uuid, node fields, tree/header imports, and that sidecar `commit_pins` will read `gitPath` from a catalog pin.

M4 dies if each page is a second hub owner, or if the tree scans git, or if undo is copied from `@affine/core`.

#### Work

1. Confirm [M3.state.yaml](../M3/M3.state.yaml) steps 1–9 `done`.
2. Spike (throwaway ok): two Y.Docs on the M0 `workspace_id` **without** keck, **wire A**. Bare `/collaboration/:workspace_id` = home; second socket `?doc=<sql uuid>`. Prove home still hydrates; second doc round-trips; lease row count stays 1. Do not spike B (path suffix) or C (mux).
3. Fill [api-map.md](../api-map.md) **Names — catalog / tree / header**: catalog guid, SQL uuids, **gRPC ExportDoc**, node schema, order helper, tree mount, header undo APIs, `git mv` behavior, linked-doc title/path post-process, **create-page uuid.md + `page_identity` + `pages.yaml`**, SharedWorker feature-detect. Multi-doc wire Actual is A (already chosen). Do **not** fill a second seed `doc:protocol`. Envelope: [rpc.md](../rpc.md). Create/rename DoD: [page-identity](../datamodel/page-identity.md).
4. Record: Room is `Map<doc_id, Doc>`; apply/persist per `docId`; owner still `workspace_id`. Bare collaboration still home. GET `/export` is advertisement, not home Yjs.
5. Note M2 [fromDoc-review](../M2/fromDoc-review.md) S4/S5: two pages make linked-doc resolution and synced-doc inlining live — do not “fix” apply here; export must keep `venus:doc:` first.

#### Do not

- Reopen B/C or a first-frame handshake.
- Ship the tree, header, or SharedWorker (step 8).
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
   - **Then** Actuals are concrete: multi-doc wire **A** (`?doc=`), catalog guid + SQL uuid, **create-page uuid.md** (no `doc:protocol` seed), tree testid + `@headless-tree/react@1.7.0`, header undo methods, **gRPC ExportDoc**, GET export advertisement, `git mv` on Flush, `page_identity` + `pages.yaml`, SharedWorker detect (`SharedWorker`, not `serviceWorker`).
   - **How:** grep / review. Fail if cells are still empty or `recon:`.
3. **Spike two docs**
   - **Given** hub + Postgres (testcontainers or Compose).
   - **When** client A writes a map key on a second `doc_id` over `?doc=<uuid>`; client B is on that same query; home is on the **bare** path.
   - **Then** B sees the value; home still has seed / prior bytes; `workspace_lease` has one owner for the M0 wiki.
   - **How:** `cargo test -p venus-hub --test recon` (or Actual recorded in api-map). Fail if the spike needs a second hub process per page.

---

### 2. step-spaces

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · **2** · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-shared-worker) · [9](#9-step-verify)

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

1. Room: `Map<doc_id, Doc>` (or equivalent). Hydrate/apply/broadcast **per `docId`**. **One persist task per Room** that drains every doc buffer (today’s hub). Each flush `INSERT`s under that buffer’s `doc_id`. Do not spawn a persist task per doc. Do not pause persist.
2. Wire **A:** `connect(docId, ydoc)` opens another WS to the **same** `/collaboration/:workspace_id` with `?doc=<sql uuid>` (omit for home). Hub binds that socket to that `doc_id` at upgrade. `OctoBaseKeckProvider` `_gen` is **per session** so catalog + page stay live together. Do not wrap frames. Do not add `/collaboration/:ws/:doc`. No SharedWorker in this step.
3. Export: **gRPC `ExportDoc`** per `doc_id` (`Hub::live_export` generalized). GET `/api/block/:workspace/export` stays advertisement JSON (no Yjs). Do not add `GET …/:doc/export`. `live_export` must not always encode `PAGE_DOC_ID` only.
4. Host: do **not** mint a second seed doc at boot. Second page = create API ([page-identity](../datamodel/page-identity.md)). Memory provider: home exists; tests call `createDoc` / create API for another page.
5. Dirty trigger already keys `doc_id` — a persist on the second page upserts a second `dirty` row; still one `dirty_wiki` / `jobs` grain.

#### Do not

- One hub lease per page.
- A persist **task** per `doc_id`.
- Change M1 two-tab / hydrate asserts except to stay green.
- Catalog ops or tree UI.
- `git mv`.
- SharedWorker / Service Worker.
- Path suffix or multiplexed frames.

#### Test scenarios

1. **Second page persists**
   - **Given** hub + Postgres; a page created via the create API (uuid from that call, not a recon constant).
   - **When** you type (or set a Yjs key) on that page, wait ≥2s, restart **only** the hub.
   - **Then** a new client hydrates that page’s bytes. Home still hydrates.
   - **How:** hub WS/store test + host Vitest. Fail if the second page lived only in tab RAM.
2. **Home still hydrates**
   - **Given** Compose hub with both docs.
   - **When** you run M1 hydrate / two-tabs.
   - **Then** they still pass (`pnpm test:e2e:m1`). Bare collab still hydrates `doc:home`. GET `/export` is advertisement (no Yjs).
   - **How:** existing M1 suite. Fail if multi-doc broke the default path.
3. **One wiki owner**
   - **Given** hub A holds the M0 workspace; a client opens the second page.
   - **When** hub B tries the same `workspace_id`.
   - **Then** B is still 503 / not a second owner. Opening the second page did not take a second `workspace_lease` row.
   - **How:** extend `compose:ha` or crate lease test. Fail if `doc_id` sticky appeared.
4. **Export per doc**
   - **Given** both pages have distinct bytes.
   - **When** you `ExportDoc` omit `doc_id` and `ExportDoc` with the second SQL uuid.
   - **Then** omit = home; the other call decodes the second guid, not home. GET `/export` is still advertisement (root has no `error`).
   - **How:** hub gRPC test + Vitest advertisement GET. Fail if both RPCs are the same tree, or if GET returns `application/octet-stream`.

---

### 3. step-catalog-crdt

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · **3** · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-shared-worker) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 3 |
| **id** | `step-catalog-crdt` |
| **title** | Catalog Y.Doc: seed, reparent, gitPath, delete |
| **dependsOn** | `step-spaces` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** the collaborative tree as data. No folder UI. Host (or tests) can reparent, delete (reject if children), and see `gitPath` change.

#### Work

1. Schema module in **`apps/web/src/host/catalog/`**: `nodes` map as in [datamodel](../datamodel/crdt.md#catalog). `gitPath` derived from ancestor **filenames** (docs get `.md` in git). `parentId` null = wiki root. No `packages/catalog`.
2. Ops: `createFolder`, `createDoc` (mint **uuid** once + `createDoc(uuid)` on the workspace; **no** `page_identity` HTTP), `rename` (docname + POSIX filename filter, sibling `_1`; **home may rename**), `reparent` (reject `home_protected` for `doc:home`), `setOrder`, `deleteNode` (reject `node_not_empty` / `home_protected`; leaf page is catalog-only until Flush). Reparent does **not** touch page bodies. See [page-identity](../datamodel/page-identity.md). Folder ids: `folder:` + uuid v4. Root: `parentId: null`.
3. Seed-once: if catalog has no nodes after sync, write `spec/` + `home` (`doc:home` → `spec/home.md`) **only**. Do not seed protocol.
4. Connect the catalog Y.Doc through `SyncProvider` like a page. Two tabs share it.
5. Vitest can run these ops on `MemoryNoopProvider` (no Docker).

#### Do not

- Mount a tree.
- `git mv` here (path is catalog-only until step 6).
- Store sibling order in git.
- Put folder nodes on `affine:page`.

#### Test scenarios

1. **Seed catalog**
   - **Given** empty catalog + seeded home (memory ok).
   - **When** seed-once runs.
   - **Then** nodes include folder `id=folder:spec` (`name` spec) and doc `home` with `docId=doc:home` and `gitPath=spec/home.md`. **No** protocol node. A second seed does not duplicate (same ids).
   - **How:** `pnpm --filter @venus/web test` under `src/host/catalog/`. Fail if seed always `add`s.
2. **Create then rename**
   - **Given** that seed.
   - **When** you `createDoc` under `spec`, then `rename` to `protocol`.
   - **Then** first `gitPath` is `spec/<uuid>.md`; after rename `spec/protocol.md`; uuid / `docId` unchanged. `foo/bar` becomes file `foo_bar.md` (name kept); sibling clash gets `_1`.
   - **How:** same Vitest. Fail if create used a hardcoded `doc:protocol`.
3. **Reparent**
   - **Given** a created page renamed `protocol`.
   - **When** you `createFolder` `design` at root and reparent that page under it.
   - **Then** `gitPath` is `design/protocol.md`. Home `gitPath` unchanged. Page Y.Doc body bytes unchanged.
   - **How:** same Vitest. Fail if reparent `Y.applyUpdate`s the page.
4. **gitPath derived**
   - **Given** a nested folder `spec/crdt` and a **created** page (not home).
   - **When** you reparent that page under it.
   - **Then** `gitPath` is `spec/crdt/{filename}.md`. Rename of folder `spec` → `SPEC` updates descendant paths (allowlist still applies). Home `parentId` stays `folder:spec`.
   - **When** you `reparent` home under `crdt`.
   - **Then** `home_protected`; home `gitPath` / parent unchanged.
   - **How:** same Vitest. Fail if home moved.
5. **Delete rejects non-empty; leaf ok**
   - **Given** seed (`spec` + `home`) plus a created page under `spec`.
   - **When** you `deleteNode(spec)`.
   - **Then** error `node_not_empty`; nodes unchanged.
   - **When** you `deleteNode` that created page.
   - **Then** the catalog row is gone; home remains. `deleteNode(home)` errors `home_protected`.
   - **How:** same Vitest (memory ok for catalog; hub Delete API when Compose). Fail if children were cascaded or reparented.
6. **A→B catalog**
   - **Given** Compose hub; two tabs.
   - **When** A creates or reparents a page (ops API or test hook).
   - **Then** B’s catalog shows the new node / `gitPath` without reload.
   - **How:** Playwright `e2e/m4-tree.spec.ts` (ops via `window.__VENUS_*` until the tree exists) or a hub Yjs test. Fail if catalog was tab-local.

---

### 4. step-chrome

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · **4** · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-shared-worker) · [9](#9-step-verify)

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

1. Layout: product **header** top (undo/redo + catalog `name` only). **Tree slot** left; editor; outline right. **Debug bar** (`venus-flush`, `venus-git-log`) is a **separate** host bar — not in the header. Show it only when **`VITE_DEBUG`** is set (web; Playwright m3/m4). Still needs `VITE_SIDECAR_URL` to talk to the sidecar. Git log polls **`spec/home.md`** (omit `?path=` or that path); do not follow the open page. Markdown pane does not take the tree slot.
2. Header `data-testid="venus-header"`: Undo / Redo buttons (`venus-undo` / `venus-redo`) call `store.undo()` / `store.redo()` on the **open** Store; **subscribe** to `store.history.canUndo$` / `canRedo$` to disable (do not poll `canUndo`). Switching pages tears down the old subscribe.
3. Current page: catalog **`name`** (`venus-page-title`). Not `gitPath`, not affine page title. Switching the open doc (test hook until step 5) rebinds header + editor + outline + md pane to that Store.
4. Vitest: `mount-editor.js` has no header/catalog imports. No `@affine/core` in `@venus/web`.

#### Do not

- History list / `undoManager` labels.
- Lease holder chip (M5).
- Import AFFiNE’s workbench header.
- Flush / git-log inside `venus-header`.
- Showing the debug bar without `VITE_DEBUG`.

#### Test scenarios

1. **Header undo**
   - **Given** the app with home open; type a unique word in the note.
   - **When** you click `[data-testid=venus-undo]`.
   - **Then** the word is gone. Redo brings it back. After undo, the Undo button is disabled because `canUndo$` went false (seed constructor history still reset). Keyboard ⌘Z / Ctrl+Z still matches. Fail if the button stays enabled until a later click/poll.
   - **How:** `e2e/m4-header.spec.ts` (memory ok). Fail if undo is a second stack.
2. **Current page**
   - **Given** catalog seed (home) plus a page created in the test hook.
   - **When** that created page is open (test hook or later tree click).
   - **Then** `[data-testid=venus-page-title]` is that catalog **`name`**, not only “Venus” from home, not `gitPath`.
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

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · **5** · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-shared-worker) · [9](#9-step-verify)

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
3. Drop a doc onto a folder → catalog `reparent` (live). **Not home** (`home_protected`; UI does not complete that drop). **Required:** create page / folder buttons (`venus-create-page` / `venus-create-folder`) with **`createAt`** = selected folder, or the folder that was **right-clicked**. **Delete:** `venus-delete-node` — hidden when the node has children or is home; calling the op still errors. Empty folder / leaf page may delete. Home row: `venus-tree-home` (highlighted as home after rename). **`data-testid` only if `VITE_TESTIDS` is set** (web host; Playwright m4 sets it). Not a hub env. [page-identity](../datamodel/page-identity.md).
4. Two tabs: drop in A appears in B without reload.
5. Keyboard: **headless-tree defaults** (`role="tree"`, library hotkeys). No extra a11y library. Revisit later: [CRDT tree — Keyboard / a11y](../components/frontend/crdt-tree/README.md#keyboard--a11y-m4). Do not scrape `wiki/` to build rows.

#### Do not

- Docusaurus / VitePress / AFFiNE explorer.
- Let headless-tree (or any UI) keep the canonical children list.
- `git mv` on drop (wait for Flush).
- Treat outline headings as folders.

#### Test scenarios

1. **Lists nodes**
   - **Given** seeded catalog.
   - **When** you open `/`.
   - **Then** `[data-testid=venus-tree]` shows `spec` and the home row (`[data-testid=venus-tree-home]`). After create (this spec or `m4-create-rename`), the new uuid row is listed. Outline still has H1 `Why Venus` when home is open, not `spec`.
   - **How:** `e2e/m4-tree.spec.ts`. Fail if the tree is the heading list.
2. **Click opens**
   - **Given** that tree.
   - **When** you click the created (or renamed) page.
   - **Then** the editor shows that page (not home seed H1 only); header title matches; undo binds that Store.
   - **How:** same spec. Fail if click only scrolls outline.
3. **Drop reparents**
   - **Given** folder `design` (create-folder) and a created page.
   - **When** you drop that page onto `design`.
   - **Then** catalog `gitPath` is `design/{name}.md` (uuid.md if not yet renamed). Second tab sees the new parent without reload. Page body unchanged.
   - **When** you drop **home** onto `design`.
   - **Then** home stays under `spec`; `[data-testid=venus-tree-home]` still that row. Op is `home_protected` if invoked.
   - **How:** Playwright drag-and-drop or the tree’s published drop API. Fail if drop rewrote markdown / Yjs page bytes, or if home moved.
4. **Outline not a wiki TOC**
   - **Given** home open.
   - **When** you compare tree vs outline.
   - **Then** outline testids stay `outline-block-preview-h1` / `h2`; M0 outline specs still pass.
   - **How:** `pnpm test:e2e` m0-outline + m4-tree. Fail if folders appear as outline items.
5. **Delete hidden / rejected when children**
   - **Given** `spec` still contains `home` (or a created page).
   - **When** you inspect delete on `spec`.
   - **Then** the control is absent / disabled. Invoking `__VENUS_deleteNode` (or the op) returns `node_not_empty`. After deleting a created leaf that was open, the editor/header show **home**. A second tab that still had that page open also auto-switches to home. Opening that uuid again shows home, not a ghost editor. `spec` can be deleted only once empty (home must be moved out first — in M4 home stays in `spec`, so **do not** require deleting `spec` in e2e; assert `spec` stays undeletable while it has children).
   - **How:** `e2e/m4-tree.spec.ts` + Vitest. Fail if cascade.

---

### 6. step-git-mv

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · **6** · [7](#7-step-links) · [8](#8-step-shared-worker) · [9](#9-step-verify)

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
2. Convert: `fromDoc` each **page** pin whose body clock moved (M3 Rust). Hydrate the **catalog pin** with **y-octo** in the same sidecar process; walk `nodes`; read `gitPath` (same filename rules as host ops). Do not `fromDoc` the catalog. Do not walk the tree in hub JS or `from-doc.js`.
3. If a page’s new `gitPath` ≠ path at last commit: `git2` `rename` / `git mv` the `.md` (and keep sidecar keyed by `docId` under `.venus/ids/<docId>.json`).
4. Body unchanged + path changed → **no** `fromDoc`; still `git mv` + one autocomment commit.
5. `commit_pins` writes every dirty page, not only home. `last_flushed` rows for those `doc_id`s (including catalog clock).
6. Rewrite `wiki/.venus/pages.yaml` every Flush from that **Rust catalog walk** (`pages:` + `folders:`). Same walk **replaces** `page_identity`. Never YAML → uuid. Never YAML → catalog. [page-identity](../datamodel/page-identity.md).
7. If the catalog pin has no that doc and HEAD still has that `.md`: **`git rm`** (and drop `.venus/ids/<docId>.json`). Do not `git rm` a folder’s files while children still exist in the catalog (delete already rejected).
8. Autocomment: **one** dirty published page → M3 `snapshot: <first ATX H1>`; **several** pages or **catalog-only** → `snapshot:` (no name).
9. Host Flush unchanged. `mount-editor` still ignorant.

#### Do not

- `git mv` on catalog drop before Flush.
- Markdown in Postgres.
- Convert inside the hub.
- `fromDoc` / `MarkdownAdapter` / `from-doc.js` on the catalog pin.
- Encode sibling order as git file names (`01-home.md`).

#### Test scenarios

1. **Two files**
   - **Given** seed catalog + a page **created** under `spec` (uuid.md); empty or existing `wiki/`.
   - **When** Flush (claim → cut → commit).
   - **Then** `wiki/spec/home.md` and `wiki/spec/<uuid>.md` exist; sidecars `.venus/ids/doc:home.json` and `.venus/ids/<uuid>.json`; YAML `pages:` maps both paths to DB uuids; YAML `folders:` lists `spec` with `id: folder:spec`; autocomment commit.
   - **How:** `cargo test -p venus-sidecar --test flush` (extend). Fail if the created page is skipped because `doc_id != PAGE_DOC_ID`. Fail if YAML uuid ≠ DB.
2. **Rename then Flush**
   - **Given** that page committed at `spec/<uuid>.md`; tree rename to `protocol`.
   - **When** Flush.
   - **Then** git has `spec/protocol.md` and not `spec/<uuid>.md`; YAML key moved; **uuid unchanged**.
   - **How:** sidecar + `e2e/m4-create-rename.spec.ts`. Fail if a new uuid was minted.
3. **Move then Flush**
   - **Given** page committed at `spec/protocol.md`; catalog reparent to `design/protocol.md`.
   - **When** Flush.
   - **Then** git log has a commit that **renames** to `design/protocol.md`; `spec/protocol.md` is gone; YAML / DB `git_path` match.
   - **How:** sidecar test + optional `e2e/m4-move.spec.ts`. Fail if both old and new files exist with copies.
4. **Catalog-only no fromDoc**
   - **Given** that page’s body clock = `last_flushed`; only catalog dirty (path change).
   - **When** Flush.
   - **Then** the markdown blob is byte-identical to HEAD’s old file (rename only). Convert is not invoked for that page (spy / no temp pin convert). Home not rewritten if not dirty.
   - **How:** sidecar test. Fail if a second dialect rewrite jittered the file.
5. **Clone matches**
   - **Given** after the move Flush.
   - **When** you read the clone without the hub.
   - **Then** folders match catalog `gitPath`s. Order of siblings is **not** asserted from git. `pages.yaml` is present (pages + folders) but ignored as live identity.
   - **How:** same flush test / verify script.
6. **Delete then Flush**
   - **Given** a created page committed at `spec/<uuid>.md` (or renamed path).
   - **When** you `deleteNode` that leaf, then Flush.
   - **Then** git no longer has that `.md`; YAML / DB have no that uuid; home file remains.
   - **How:** sidecar + `e2e/m4-create-rename.spec.ts` delete scenario. Fail if the file stayed or children were removed.

---

### 7. step-links

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · **7** · [8](#8-step-shared-worker) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 7 |
| **id** | `step-links` |
| **title** | embed-linked-doc + markdown path; survives git mv |
| **dependsOn** | `step-git-mv` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** a card from home to a **created** (then renamed) page that export and git can name. M2 synthetic `untitled` + `./workspace/…/doc:lease` is not enough once a catalog exists.

#### Work

1. Insert `affine:embed-linked-doc` with `pageId =` the created page’s uuid/`docId`.
2. `fromDoc` post-process **rewrites** the adapter link (do not leave `./workspace/<ws>/<pageId>`). Link text = target catalog `name`; href = POSIX relative from the **current** file’s `gitPath` to the target `gitPath`; keep `<!-- venus:doc:<docId> -->`. Placement must not require `/${pageId}` in the href (M2 `urlMentionsPageId` breaks once the URL is `protocol.md`).
3. After `git mv`, next Flush: markdown URL updates; comment id unchanged. WYSIWYG card still opens/resolves that `pageId` (BlockSuite, not path).
4. Re-read [fromDoc-review S4](../M2/fromDoc-review.md): `pageId` allowlist still holds. Do not implement M6 `toDoc` → card.
5. On create / rename / seed: set `workspace.meta.docMetas[docId].title` = catalog `name`. Live card reads that. Git / `fromDoc` post-process still uses catalog `name` for link text, not affine `affine:page` title. Do not change home `# Venus` (`titleMiddleware` of the page file; M3 autocomment).

#### Do not

- Path-only identity.
- Whole-file `toDoc` onto the live doc.
- Synced-doc transclusion as a product surface (S5 still open; do not seed `embed-synced-doc`).

#### Test scenarios

1. **Card + comment**
   - **Given** home contains `affine:embed-linked-doc` to a created page (renamed `protocol` or still uuid).
   - **When** `fromDoc(home)`.
   - **Then** markdown is `[<catalog name>](<relative>)` plus `<!-- venus:doc:<uuid> -->`. Live card title is that catalog `name` (`docMetas`), not `untitled`. Fail if git still has `./workspace/<ws>/…` or `untitled` only.
   - **How:** Vitest mdgate + catalog fixture; `e2e/m4-link.spec.ts` for the card visible.
2. **Resolves after move**
   - **Given** that card; reparent the created page; Flush.
   - **When** you open home (reload ok).
   - **Then** the linked-doc card still targets the same `pageId` (uuid); git markdown comment still has that id; URL path matches new `gitPath`.
   - **How:** `e2e/m4-link.spec.ts` + sidecar markdown assert. Fail if the card is missing or points at a new space.
3. **Export path from catalog**
   - **Given** that page at `design/protocol.md` (after rename + reparent).
   - **When** Flush home (dirty because title middleware / link path).
   - **Then** `wiki/spec/home.md` contains a relative path to `design/protocol.md` (e.g. `../design/protocol.md`).
   - **How:** sidecar / Vitest. Fail if the file still says `spec/protocol.md` or `spec/<uuid>.md`.

---

### 8. step-shared-worker

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · **8** · [9](#9-step-verify)

| | |
|---|---|
| **n** | 8 |
| **id** | `step-shared-worker` |
| **title** | SharedWorker in front of wire A (feature-detect) |
| **dependsOn** | `step-links` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** fewer hub sockets when two tabs share a browsing profile. Hub protocol **unchanged** (still A: one TCP per `doc_id`, `?doc=`). Tree / header / Flush already work from steps 2–7 on per-tab sockets.

`navigator.serviceWorker` is **not** this feature. Detect `typeof SharedWorker === 'function'`. A Service Worker is a fetch interceptor and can be killed; it does not hold the collab sockets.

#### Work

1. `apps/web/src/host/providers/keck-shared-worker.js` (or Actual path): worker opens the **same** A WebSockets (`/collaboration/:workspace` and `?doc=`). Tabs send/receive `Uint8Array` updates over `MessagePort`. `Y.Doc`, BlockSuite `Store`, and React stay in the tab.
2. Host: if SharedWorker exists, `OctoBaseKeckProvider` (or a thin wrapper) talks to the worker instead of `new WebSocket` in the tab. Same `connect(docId, ydoc)` / `disconnect` / `whenReady` seam. Memory provider unchanged.
3. Subscribe/unsubscribe per `doc_id` in the worker (catalog stays while the tree is mounted; page socket replaced on switch). Last tab close → worker closes those sockets.
4. No SharedWorker (or worker construct throws): **identical** per-tab A path from step 2. Product must not require the worker.
5. Playwright: two **pages in one BrowserContext** share one worker (assert fewer sockets or a host hook). Two **contexts** each get their own worker — that is not a fail; it is how the API isolates. Chromium has SharedWorker; if a browser in CI does not, skip the share assert and still run the fallback.

#### Do not

- Wrap y-protocols (C). The hub still sees vanilla AFFiNE frames, one `doc_id` per socket.
- Put `Y.Doc` or BlockSuite in the worker.
- Use `navigator.serviceWorker.register` for collab.
- Make tree, header, or Flush depend on the worker.
- Change hub routes.

#### Test scenarios

1. **Fallback**
   - **Given** SharedWorker is undefined or the provider is forced to the tab path (test hook).
   - **When** you run `m4-tree` two-tabs drop (or catalog CRDT two-tabs).
   - **Then** B still sees A’s drop. Hub still has one owner.
   - **How:** `e2e/m4-shared-worker.spec.ts` + existing m4 specs. Fail if missing SharedWorker blanks the tree.
2. **Share in one profile**
   - **Given** Chromium (or a browser with SharedWorker); two pages, **same** Playwright context; Compose hub.
   - **When** both show the tree (catalog + a page).
   - **Then** catalog (and the open page, if both opened the same `doc_id`) use the worker-held A sockets; a drop in page 1 appears in page 2 without reload.
   - **How:** same spec; optional hub metric / `performance.getEntriesByType('resource')` / debug hook counting WS upgrades. Fail if each tab still opens a duplicate catalog TCP **and** the worker is running (implementation did not actually share).
3. **Seam holds**
   - **Given** `mount-editor.js`.
   - **When** you search for SharedWorker / `keck-shared-worker`.
   - **Then** none. Memory `pnpm test` still has no worker.
   - **How:** extend `sync-provider.test.ts`. Fail if the editor thread owns the worker.

---

### 9. step-verify

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-catalog) · [2](#2-step-spaces) · [3](#3-step-catalog-crdt) · [4](#4-step-chrome) · [5](#5-step-tree) · [6](#6-step-git-mv) · [7](#7-step-links) · [8](#8-step-shared-worker) · **9**

| | |
|---|---|
| **n** | 9 |
| **id** | `step-verify` |
| **title** | Close-out: create, rename, link, move, header undo |
| **dependsOn** | `step-shared-worker` |
| **kind** | implement |
| **status** | **pending** ([board](./M4.state.yaml); breakpoint `human`) |

**Adds:** board `done`. Person + clone + smoke. SharedWorker is an optimization; exit 1–12 must hold on the fallback path.

#### Work

1. Runbook: M4 close-out — create a page (uuid.md), rename in the tree, link, drop to a folder, delete a leaf (not home), Flush, clone `wiki/`, header undo/redo vs keyboard. Note SharedWorker vs per-tab. Map: [page-identity](../datamodel/page-identity.md).
2. Person in Chrome or Firefox (Chrome has SharedWorker; if Firefox path is fallback-only, say so on the board).
3. `pnpm test:e2e:m1`, M2 pane, M3 flush/live, new `m4-*.spec.ts` including **`m4-create-rename`** and `m4-shared-worker`.
4. Mark [M4.state.yaml](./M4.state.yaml) steps 1–9 `done` with evidence. **Do not** mark `done` if `m4-create-rename` failed or was skipped.
5. Re-read [fromDoc-review](../M2/fromDoc-review.md) S4/S5 note (dogfood starts after this); do not “close” S1–S7 here unless already green.

#### Do not

- Start M5 lease in this step.
- Record remotes as required evidence (optional note only).
- Close M4 if clone folders do not match the tree after Flush.
- Close M4 if the app requires SharedWorker to open two pages.
- Close M4 if the second page was only a recon-seeded `doc:protocol`.

#### Test scenarios

1. **Create → rename → map** (required DoD)
   - **Given / When / Then:** [page-identity](../datamodel/page-identity.md) (create in `spec`, Flush → `uuid.md` + YAML `pages:`/`folders:` + DB from catalog pin; rename `protocol`; Flush → `git mv`; `foo/bar` → `foo_bar.md`; sibling `_1`; corrupt YAML → restore from catalog pin; **delete** non-empty folder rejected; leaf page deleted then Flush → file gone; UI auto-switches to home if that uuid is gone from catalog; home still hydrates).
   - **How:** `e2e/m4-create-rename.spec.ts` under `pnpm test:e2e:m4`. Fail closed if skipped.
2. **Exit held**
   - **Given** Compose `postgres` + `hub` + `web` + sidecar.
   - **When** you create a page, rename it, insert one linked-doc, move it to another folder, Flush, clone `wiki/`.
   - **Then** git tree matches catalog `gitPath`s; YAML uuids match DB; the link still resolves; header undo/redo matches keyboard on the open page.
   - **How:** `pnpm test:e2e:m4` (or Actual) + clone script. Fail if any exit bullet is demo-only.
3. **Smoke**
   - **Then** `m4-create-rename`, `m4-tree`, `m4-header`, `m4-move`, `m4-link`, `m4-shared-worker` pass; `pnpm test:e2e:m1`, M2 pane, `pnpm test:e2e:m3` still pass.
   - **How:** those commands.
4. **Manual path**
   - **Then** runbook M4 close-out holds (create, rename, tree, drop, Flush, clone, undo button, outline still headings). Two tabs in one window still sync if SharedWorker is on.
   - **How:** person + evidence on the board (date + browser).
5. **Prior milestones green**
   - **Then** memory `pnpm test:e2e` (m0 + m2) still passes with the new chrome (no worker).
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
| Day 6 | 7 `step-links` |
| Day 7 | 8 `step-shared-worker` → 9 `step-verify` |

If M3 is still open, **stop**. Do not fake a second page as two git paths on one Y.Doc.

## Risks

| Risk | What to do in M4 |
|---|---|
| Gate skipped | Step 1 **Gate held** fails closed |
| `doc_id` sticky / second hub owner per page | Step 2 **One wiki owner**; HA grain |
| Second `connect()` kills home WS (`_gen`) | Step 2 provider sessions **per doc** |
| GET `/export` still serving Yjs / silent home | Step 2 **Export per doc**; [rpc.md](../rpc.md) |
| Mux frames (C) or path suffix (B) | Wire A only; step 1 spike uses `?doc=` |
| SharedWorker required for the tree | Step 8 last; **Fallback** test; verify does not close if worker is mandatory |
| Service Worker used as WS | Forbidden; detect `SharedWorker` |
| Two Playwright **contexts** expected to share one worker | They must not; share assert is two pages, one context |
| Tree reads `wiki/` | Step 5 data = catalog; step 6 git lags live |
| Delete cascade / Finder-style reparent | Reject `node_not_empty`; UI hides; host op errors |
| `git mv` on every drop | Step 6 only on Flush |
| `commit_pins` still home-only | Step 6 **Two files** |
| Path as link identity | Step 7 `venus:doc:` + `pageId` |
| `@affine/core` header | Step 4 **Seam holds** |
| Outline replaced by folders | Step 5 **Outline not a wiki TOC** |
| Catalog is an `affine:page` | Step 3 schema; hub stores a plain Y.Doc |
| `toDoc` restores the card | Forbidden; M6 |
| Remotes / AB5 as M4 DoD | After this exit |
| Pre-seeded `doc:protocol` as the second page | [page-identity](../datamodel/page-identity.md); verify **Create → rename → map** |
| YAML treated as uuid truth | Flush restores YAML `pages:` + `folders:` from the catalog pin; replaces `page_identity` from that pin |

## Handoff to M5

M5 may assume:

- Two (or more) published spaces + `venus:catalog` on the same wiki owner.
- Tree + header exist; current page is visible; undo/redo is host chrome.
- Per-tab A sockets work; SharedWorker is an optional host fan-in of those sockets.
- Flush `git mv`s; clone folders match catalog after publish.
- Linked-doc export has stable `docId` comments.
- Freeze / `store.readonly` / CodeMirror / lease holder chip are **new**.
- Hub still has no convert and does not write `jobs`.

M5 exit is: second user cannot type in WYSIWYG during the lease; they see who holds it. M4 exit is “create a page, rename it, one link, move, git matches, YAML pages+folders projection, header undo.”

## Invariants (M4 only)

1. Path is not identity. **uuid** / `docId` is. `gitPath` and `pages.yaml` are projections.
2. Catalog CRDT is the live tree; git is the share layout after publish. **`page_identity` is a Flush SQL cache** written by **Rust** in `venus-sidecar` (y-octo walk of the catalog pin, same job as page `fromDoc`). Catalog Yjs id is identity for folders. Flush restores YAML from that walk (never YAML → uuid / catalog). No `/api/pages`. Do not `fromDoc` the catalog.
3. Moves do not rewrite page bodies. `git mv` on Flush, not on drop.
4. One hub owner per `workspace_id`. Many `doc_id`s inside that room.
5. Outline is headings. Tree is folders/pages.
6. Header undo is the open Store’s Yjs undo stack, not a timeline, not `@affine/core`.
7. Linked-doc keys by `docId`; markdown also carries path for humans/agents.
8. `mount-editor` stays unaware of catalog, tree, header, and git.
9. No lease, no comment-commit why, no remotes required.
10. Memory default stays Docker-free for m0/m2 e2e.
11. Hub wire is A. SharedWorker does not change URLs or frames.
12. Second pages are **created**, not recon-seeded. First git file is `{uuid}.md`; tree rename changes path only.
