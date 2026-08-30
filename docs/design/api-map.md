# API map

One contract for **installed** symbols. Milestone plans ([M0](./M0/plan.md), [M1](./M1/plan.md), …) use **design names** from [venus-implementation-plan.md](./venus-implementation-plan.md). This file is the Actual column. Do not keep a second map per milestone.

Fill new rows when recon for that slice runs. Do not keep coding against names that are not in this table. If packages disagree with a plan’s hints, **this file wins**.

## Pins

| Piece | Version | Filled |
|---|---|---|
| `@blocksuite/affine` | `0.22.4` (latest on npm; 0.27 is not published). npm `gitHead` `a5091e72365a47351f370ca23f212ef43a9d42f0` | M0 |
| `@blocksuite/store` (if separate from affine re-export) | not a direct dep — affine `0.22.4` depends on `@blocksuite/store@0.22.4` and re-exports `./store` and `./store/test` | M0 |
| `yjs` | `13.6.32` (root `pnpm.overrides`; one major, no duplicate) | M0 |
| `@toeverything/theme` | `1.1.23` (`style.css` + `fonts.css`) | M0 |
| AFFiNE git SHA used as the copy source (editor container) | AFFiNE tag `v0.22.4`, file `blocksuite/integration-test/src/editors/editor-container.ts`. Class renamed `TestAffineEditorContainer` → `VenusEditorContainer`; tag stays `affine-editor-container`. Copy lives at `apps/web/src/host/editor-container.js` (`.js` so `tsc` does not follow affine’s published `.ts`). | M0 |
| Sync server | OctoBase **keck** @ git `276e0e94719a652483119c5fea16be13293ee21c` (2025-03-05, `master`, “chore: bump deps”). Clone `https://github.com/toeverything/OctoBase`. Listen `0.0.0.0:3000` (`KECK_PORT`). Product store: **Postgres** (`DATABASE_URL`). Compose service **`octobase`**. | M1 |
| JS sync client | No npm OctoBase client. Venus class `OctoBaseKeckProvider` (`apps/web/src/host/providers/octobase-keck-provider.js`, step 3). Wire: `yjs@13.6.32` + `y-protocols@1.0.7` + `lib0@0.2.117`, `new WebSocket(url, ['AFFiNE'])`. **Not** `y-websocket` `WebsocketProvider`. | M1 |
| Blob HTTP | Default keck features include `api`: `POST /api/blobs/:workspace` (`application/octet-stream`) → `{ id, exists }`; `GET`/`HEAD`/`DELETE /api/blobs/:workspace/:hash`. Fallback if `api` is off: `PUT /api/workspace/:id/blob`. | M1 |
| `y-octo` crate | crates.io **`0.1.0`**: `Doc::try_from_binary_v1`, `apply_update_from_binary_v1`, `encode_update_v1`. Same codec family as keck `jwst-codec`. | M1 |
| Docker image / compose file | Venus `deploy/octobase/Dockerfile` builds keck from the SHA above. `docker-compose.yml`: **`postgres`** (`postgres:16`, volume `pg-data`) + **`octobase`**. **Step 8** adds **`web`**. Do not put keck and Postgres in one container. | M1 |

## Pin notes (BlockSuite / Vite)

- `@blocksuite/affine/effects` **src** is type-only (`import { type … }`). `viewManager.get('page')` already runs each view extension’s `effect()` (foundation includes `stdEffects` + `richTextEffects`). Do **not** also call `stdEffects()` — a second `customElements.define` throws.
- `src/host/boot.js` and `src/host/workspace.js` (not `.ts`): `tsc` must not follow affine’s published `.ts` sources (they fail under TS 5.8). Host tests live in `*.test.ts` and import those `.js` files (plus `.d.ts`). `@types/node` is a direct dep so `node:fs` in tests typechecks.
- `@blocksuite/icons` peers React 18; host is React 19. Warning only.
- Production minify is off (`vite.config.ts`): esbuild would emit an invalid template from lit-html’s backtick-in-character-class regex (`SyntaxError` at `index-*.js` line 12).
- Vite 8/oxc must lower BlockSuite’s published TS with **`decorator.legacy: true`** and **`assumptions.setPublicClassFields: true`**. Do **not** use `removeClassFieldsWithoutInitializer` — it drops `accessor std!` (no initializer) and page blocks never receive `std`. The same assumption emits broken class fields `[null];` for `this[symbol] = …`; `blocksuiteDecorators` strips those so `vite build` (rolldown) can parse.
- Lit `@provide` (legacy) `Object.defineProperty`s the wrapped setter and returns `undefined`. Stock `__decorate` then restores the pre-decorator accessor. `vite.config.ts` rewrites oxc’s decorate helper (`virtual:venus-oxc-decorate`) so a decorator that returns undefined is left alone.
- `@Peekable()` in `@blocksuite/affine-components` is a **TC39** class decorator (`context.kind`). Legacy lowering calls it with no context. Plugin `venus-peekable-legacy-compat` changes the guard to `if (context && context.kind !== 'class')`. Must run `enforce: 'pre'` before oxc.
- Do not put `decorator.legacy` on **global** `oxc`: vanilla-extract’s nested compiler copies it and injects `\0@oxc-project+runtime.../decorate.js` into `*.css.ts`. Apply legacy only in `blocksuiteDecorators()` / `optimizeDeps.rolldownOptions.transform`.
- vanilla-extract: `vanillaExtractPlugin({ unstable_mode: 'transform' })` plus `venus-ve-css-ts` (`@vanilla-extract/integration` `transform()`) so `*.css.ts` get `setFileScope` before prebundle. Exclude `*.css.ts` from global oxc.
- **One module graph:** `optimizeDeps.exclude` every `@blocksuite/*` plus `lit` / `@lit/*` / `@preact/signals-core` / `yjs`. Prebundling some of those but not others duplicates `stdContext` (page blocks: `this.std` is undefined). Nested CJS then served as source is converted by `venus-node-cjs-to-esm` (esbuild). Do not convert `react` / `react-dom` / `scheduler`.
- Outline (and other fragments) use vanilla-extract `*.css.ts`. Node Vitest cannot evaluate those files. Dev/build uses `@vanilla-extract/vite-plugin` (`unstable_mode: 'transform'`) plus `venus-ve-css-ts`.
- `TestWorkspace` is marked `@internal` / test-only in `@blocksuite/store`. M0 uses it as the in-memory collection; there is no public `DocCollection` on this pin. Keep unless a later milestone replaces it with a public workspace API.
- `GetStoreOptions` omits `schema`. `Store` constructs `new Schema()` and registers flavours from `BlockSchemaIdentifier` (wired by store extensions). Do not pass `schema` into `getStore`. A standalone `new Schema().register(AffineSchemas)` is unused on this path.
- `TestDoc.getStore()` already concatenates `workspace.storeExtensions`. Do not also pass those extensions in `getStore({ extensions })` or they duplicate.

## Host and Playwright notes

- Host autotest is Vitest (Node): `pnpm test`. Specifiers are checked with `import.meta.resolve` (no view execution). `createM0Workspace(provider?)` in `src/host/workspace.js` is one collection (`venus-m0`), one doc (`doc:home`), default tree plus `seedHomeNote()` (`src/host/seed.js`), `store.resetHistory()`, then `provider.connect(docId, store.spaceDoc)` (default `MemoryNoopProvider`). The App uses `providerFromEnv()`: unset `VITE_SYNC_URL` → memory; set → `OctoBaseKeckProvider`. M1 hydrates against the server and seeds **only if empty** — update this paragraph when that lands.
- Playwright (`pnpm test:e2e`): `m0-smoke.spec.ts` is the M0 close-out smoke (outline H1 + type + refresh); also `m0-editor.spec.ts` type / slash / undo; `m0-seed.spec.ts` title + H1/H2 + constructor undo; `m0-outline.spec.ts` heading list / edit-track / scroll / not a folder tree; `m0-provider.spec.ts` type then reload. M1 provider spec is `pnpm test:e2e:m1` (`PLAYWRIGHT_M1=1`, Vite `:5174` with `VITE_SYNC_URL`, needs Compose keck). Hydrate / two-tab / blob specs come in later M1 steps.
- Note selector: `affine-note affine-paragraph rich-text`. Slash: `affine-slash-menu .slash-menu` (widget tag `affine-slash-menu-widget`). Title is `doc-title` (not the note). Headings in the editor: `.affine-paragraph-rich-text-wrapper.h1|.h2`. Outline items: `[data-testid="outline-block-preview-h1"]` / `h2`. Click the note, then wait a frame before `/` — BlockSuite keeps `document.activeElement` on `affine-page-root`. Image selector: `affine-image` (custom element). Page view is `affine-page-image` inside `.affine-image-container`; the bitmap is `affine-page-image .resizable-img img` (not a raw `img` as a direct child of `affine-image`).

## Names — editor

| Design name | Likely | Actual import | Notes |
|---|---|---|---|
| Collection / workspace | `TestWorkspace` or `Workspace` from `@blocksuite/affine/store` / `@blocksuite/store` / `@blocksuite/store/test` | `TestWorkspace` from `@blocksuite/affine/store/test` | `new TestWorkspace({ id: 'venus-m0' })`. `workspace.meta.initialize()`. `createDoc(id)`. `@internal`. |
| Schema | `Schema` + `AffineSchemas` from `@blocksuite/affine/schemas` | `Schema` from `@blocksuite/affine/store`; `AffineSchemas` from `@blocksuite/affine/schemas` | Root flavour is `affine:page` (`RootBlockSchema`). Register happens inside `Store` via store extensions, not via `getStore({ schema })`. |
| Page / doc (Y.Doc wrapper) | `collection.createDoc(id)` then `.getStore()` → `Store` | `workspace.createDoc('doc:home')` then `doc.getStore()` → `Store` from `@blocksuite/affine/store` | `store.load(() => store.addBlock(...))`. `store.root.flavour === 'affine:page'`. M1: wait for sync before seeding if empty. |
| Y.Doc | `store.spaceDoc` or equivalent | `store.spaceDoc` (also `doc.spaceDoc`); type `Doc` from `yjs` | `createM0Workspace` passes this to `SyncProvider.connect`. |
| Sync provider | `SyncProvider` + `MemoryNoopProvider` | `src/host/sync-provider.js` (+ `.d.ts`) | `kind` is `memory`, `octobase`, or `y-websocket`. Default is memory no-op when sync env is unset. App `disconnect`s on unmount. Do not import the server in `mount-editor.js`. |
| Store extensions | `StoreExtensionManager` + `getInternalStoreExtensions()` from `@blocksuite/affine/ext-loader` and `@blocksuite/affine/extensions/store` | `StoreExtensionManager` from `@blocksuite/affine/ext-loader`; `getInternalStoreExtensions()` from `@blocksuite/affine/extensions/store` | `workspace.storeExtensions = manager.get('store')` then `doc.getStore()` with no extra `extensions`. |
| View / page specs | `ViewExtensionManager` + `getInternalViewExtensions()` from `@blocksuite/affine/ext-loader` and `@blocksuite/affine/extensions/view` | `ViewExtensionManager` from `@blocksuite/affine/ext-loader`; `getInternalViewExtensions()` from `@blocksuite/affine/extensions/view` | `editor.pageSpecs = [...viewManager.get('page'), FontConfigExtension(...)]`. `OutlineViewExtension` is already in `getInternalViewExtensions()`. |
| Effects (custom elements) | `import '@blocksuite/affine/effects'` (and outline view effects if not pulled by the manager) | `import '@blocksuite/affine/effects'` (type-only). Custom elements register in each view extension’s `effect()` when `viewManager.get('page')` runs. | Do not call `stdEffects()` separately. Outline registers `affine-outline-panel` via `OutlineViewExtension.effect()` when `viewManager.get('page')` runs (`mount-editor.js`). |
| Editor container | Copy of AFFiNE `TestAffineEditorContainer` (tag `affine-editor-container`). **Not** `@affine/core`. | `VenusEditorContainer` in `apps/web/src/host/editor-container.js`; `customElements.define` in `boot.js` if the tag is free. Properties: `doc`, `mode`, `pageSpecs`, `edgelessSpecs`, `host`, `std`, `autofocus`. | Mount: `mountEditor(host, store)` returns `{ editor, unmount }`. Sets `mode = 'page'`, `pageSpecs` / `edgelessSpecs` from `ViewExtensionManager(getInternalViewExtensions()).get(...)` plus `FontConfigExtension(CommunityCanvasTextFonts)`. |
| EditorHost | `editor.host` (`@blocksuite/affine/std`) | `EditorHost` from `@blocksuite/affine/std`; `editor.host` | Outline takes **host**, not the container. `waitForEditorHost` polls — Lit `updateComplete` can fire before `std.render()` connects `editor-host`. |
| Outline | `OutlinePanel` from `@blocksuite/affine/fragments/outline` | `OutlinePanel` from `@blocksuite/affine/fragments/outline` | `src/host/mount-outline.js`: `new OutlinePanel()`, `panel.editor = host` (`EditorHost`), `fitPadding: [20, 20, 20, 20]`. Tag `affine-outline-panel`. Layout: `.outline-host` 300px right of `.editor-host`. Preview testids: `outline-block-preview-h1` / `h2` / `title`. |
| Fonts | `FontConfigExtension(CommunityCanvasTextFonts)` from `@blocksuite/affine/shared/services` | `FontConfigExtension`, `CommunityCanvasTextFonts` from `@blocksuite/affine/shared/services` | Without this, glyphs look wrong or missing. |
| Theme CSS | `@toeverything/theme/style.css` + `fonts.css` | `@toeverything/theme/style.css` + `@toeverything/theme/fonts.css` | Already imported from `src/host/boot.js`. |
| Page title | `affine:page` + `Text` from store | `affine:page` + `Text` from `@blocksuite/affine/store` | `store.addBlock('affine:page', { title: new Text('Venus') })`. |
| Seed tree | `affine:page` → `affine:surface` + `affine:note` → `affine:paragraph` / `affine:heading` | `affine:paragraph` with `type` `h1` or `h2` (no `affine:heading` flavour on 0.22.4) | `seedHomeNote()` in `src/host/seed.js` after the empty paragraph. `text: new Text('Why Venus')` etc. Extra empty paragraphs between H1 and H2 for scroll. |

## Names — sync, blobs, snapshot

Fill Actual in [M1 `step-recon-sync`](./M1/plan.md#1-step-recon-sync). Append recon notes; do not delete.

Recon notes (2026-08-29, this machine: Darwin arm64, rustc 1.85.0, keck binary linked **x86_64** / Rosetta):

- OctoBase is AGPL-3.0. Keep it **out of** the `@venus/web` bundle. Server process / image only. [licensing.md](../legal/licensing.md).
- AFFiNE Cloud / `nbstore` / Socket.IO is **not** the M1 protocol. Do not copy `@affine/core`.
- Pin: `https://github.com/toeverything/OctoBase` @ `276e0e94719a652483119c5fea16be13293ee21c` (2025-03-05, last public `master` checked). Default keck features compile sqlite **and** mysql **and** postgres (`jwst` → `api` + swagger `schema`). **M1 product runtime is Postgres** via `DATABASE_URL`. SQLite is recon-only.
- **Wire:** keck `jwst-rpc` speaks the Yjs **sync protocol** (`y-protocols/sync` + awareness tags 0–3), **not** the stock `y-websocket` client. Stock `WebsocketProvider` does **not** send `Sec-WebSocket-Protocol: AFFiNE`; axum `ws.protocols(["AFFiNE"])` will not complete the handshake. Historical AFFiNE `KeckProvider` is the shape to copy (thin: `lib0` + `y-protocols`), with the subprotocol added. There is **no** npm `@toeverything/octobase` client.
- **Do not** set `USE_MEMORY_SQLITE`. sqlx in-memory SQLite is not shared across the pool; `create_workspace` fails with `Crud("Failed to create workspace …")` and the WS handler panics. If `DATABASE_URL` is unset, keck falls back to file SQLite `./data/jwst.db` — that fallback is **forbidden** as the M1 store. The recon spike used a host binary + `/tmp/venus-keck/data/jwst.db`. Product runtime is Compose **`postgres` + `octobase`** (`docker-compose.yml`).
- **Build:** `CARGO_TARGET_DIR=/tmp/keck-target cargo build -p keck` from the clone. Cursor’s default sandbox `CARGO_TARGET_DIR` hit an Apple clang segfault in the `indexmap` build script on this Mac; `/tmp/keck-target` succeeded (~3 min).
- CORS already allows `http://localhost:5173` and `http://127.0.0.1:5173` for GET/POST/DELETE/OPTIONS (not PUT). Browser can talk to `:3000` without a Vite proxy. WS is not CORS; still use the `AFFiNE` subprotocol.
- Connecting `GET ws://127.0.0.1:3000/collaboration/:workspace` **creates** the workspace. `POST /collaboration/:workspace` returns `{ "protocol": "AFFiNE" }` (auth probe only).
- keck `:workspace` path = **`venus-m0`**. Do not put `doc:home` in the URL (colon). M1 wires **`doc:home` `store.spaceDoc`** as that one keck workspace Y.Doc (one page). Collection root `TestWorkspace.doc` (subdoc map `spaces`) is **not** on the wire in M1. OctoBase also materializes maps `space:updated` / `space:meta` on its workspace doc; they can sit beside BlockSuite’s `blocks` map.
- Spike (throwaway `scripts/m1-recon-spike.mjs`): two Node `Y.Doc`s, A `getMap('spike').set('k','v')`, B saw `v` in **192ms**. Delete before M1 exit.
- Blob REST has **no list** route. `BlobSource.list` can return `[]`; image load is `get(sourceId)` from block props. Hash is SHA-256 base64url (BlockSuite `sha()` in `@blocksuite/global/utils`; keck `id` looks the same — confirm equality in step 6).
- `yjs` stays `13.6.32` via root `pnpm.overrides`. Do not let the sync client hoist a second major.

| Design name | Likely | Actual import / endpoint | Notes |
|---|---|---|---|
| Workspace id | `DocCollection.id` = OctoBase `workspace_id` | `TestWorkspace({ id: 'venus-m0' })` = keck `/collaboration/venus-m0` | Keep. Connecting the socket creates the workspace. |
| Space / doc id | `createDoc` = OctoBase space; M0 `doc:home` | `workspace.createDoc('doc:home')` → `store.spaceDoc` (Yjs subdoc guid `doc:home`) | M1 room is still `venus-m0`; the bytes on that room are this `spaceDoc`, not the collection root. Never mint a second space for the same page. |
| Memory provider | `MemoryNoopProvider` `kind: 'memory'` | `src/host/sync-provider.js` | Default when no sync env. Vitest and M0 e2e stay on this. |
| Live provider | `kind: 'octobase' \| 'y-websocket'` | `kind: 'octobase'` → `OctoBaseKeckProvider` in `src/host/providers/octobase-keck-provider.js` | App picks it via `providerFromEnv()` (`VITE_SYNC_URL`). Do not import it from `mount-editor.js`. |
| Yjs sync client | `y-websocket` `WebsocketProvider` **or** thin WS that sends/receives update v1 | `yjs` + `y-protocols/sync` + `lib0`; `new WebSocket(url, ['AFFiNE'])` | Same framing as historical `KeckProvider`. Add `y-protocols` as a **direct** `@venus/web` dep in step 3 (already transitive via store). |
| Synced signal | `provider.on('sync', …)` / `synced` | `provider.synced === true` after applying y-protocols **SyncStep2**; emit `'sync'` then | Hydrate **must** wait before seeding. Empty docs still get a Step2 (spike: 2-byte update). |
| Sync WS URL | `ws://127.0.0.1:1234` (y-websocket) or keck `:3000` | `ws://127.0.0.1:3000/collaboration/venus-m0` | Vite env `VITE_SYNC_URL` (step 3). Origin is the keck host; path includes the workspace id. |
| Vite proxy | `/sync` → sync server | none | keck CORS already lists Vite `:5173`. Add a proxy later only if mixed-content / cookie constraints appear. |
| BlobSource | BlockSuite `BlobSource` (`get`/`set`/`delete`/`list`) | `BlobSource` from `@blocksuite/affine/sync` (re-export `@blocksuite/sync`). `TestWorkspace({ blobSources: { main, shadows? } })`. Default `main` is `MemoryBlobSource`. | `name`, `readonly`, `get`/`set`/`delete`/`list`. Wire as `blobSources.main` (HTTP) with optional memory shadow — not a Venus React upload UI. |
| blobSync | `store.blobSync.set(blob)` → `sourceId` | `store.blobSync` is `BlobEngine` (`workspace.blobSync`). Insert path: `std.store.blobSync.set(file)` → `sourceId` on `affine:image` | Slash / paste uses `@blocksuite/affine-block-image` `buildPropsWith`. |
| Blob HTTP | OctoBase blob REST or `POST /blobs/:id` on a tiny Venus service | `POST /api/blobs/venus-m0` body `application/octet-stream` → `{ id, exists }`; `GET`/`HEAD`/`DELETE /api/blobs/venus-m0/:hash` | Nested under `/api` (default features). No list endpoint. CORS has POST, not PUT. |
| Image in page | slash / paste → `affine:image` | flavour `affine:image` (`ImageBlockSchema`); props `sourceId`. Tag `affine-image`; page child `affine-page-image` in `.affine-image-container`. Playwright: `affine-image .affine-image-container img` | Confirm pixels after blob HTTP in step 6. |
| Snapshot API | keck REST **or** `y-octo` `Doc::try_from_binary_v1` + `encode_update_v1` | `GET /api/block/venus-m0/export` → Yjs update v1 (`application/octet-stream`). Optional decode: y-octo `0.1.0` `Doc::try_from_binary_v1`. Init: `POST /api/block/venus-m0/init` | After the spike, export was 23 bytes and contained `spike`/`k`/`v`. Swagger: `JWST_DEV=1` → `http://127.0.0.1:3000/api/docs/`, OpenAPI `http://127.0.0.1:3000/api/jwst.json`. |
| Compose | `postgres` + `octobase` (+ `web` in step 8) | `docker-compose.yml` at repo root. `docker compose up --build postgres octobase`. Volume `pg-data`. DSN `postgres://venus:venus@postgres:5432/jwst?sslmode=disable`. | Two containers. Do not `down -v` if you need the doc. `pnpm sync:up` / `sync:down`. |

### Chosen backend (M1 recon)

| | |
|---|---|
| **kind** | `octobase` (keck server + Venus thin Yjs client; not stock `y-websocket`) |
| **Why** | keck builds and speaks Yjs update v1 over WS; blobs and export exist on the same process; AGPL stays out of `@venus/web`. Stock `y-websocket` cannot set the `AFFiNE` subprotocol. AFFiNE Cloud/nbstore is a different protocol. |
| **Server start** | From the Venus root: `docker compose up --build postgres octobase` (or `pnpm sync:up`). keck listens `http://127.0.0.1:3000`. `DATABASE_URL=postgres://venus:venus@postgres:5432/jwst?sslmode=disable` (set in Compose; must not be omitted). Do not set `USE_MEMORY_SQLITE`. Optional: `JWST_DEV=1` for `/api/docs/`. **Not DoD:** host `cargo run --bin keck` (recon used SQLite at `/tmp/venus-keck/data/jwst.db`). |
| **Client package** | `yjs@13.6.32` + `y-protocols@1.0.7` + `lib0@0.2.117`. Class: `OctoBaseKeckProvider`. `new WebSocket('ws://127.0.0.1:3000/collaboration/venus-m0', ['AFFiNE'])`. |
| **Storage** | **Postgres 16**, Compose service **`postgres`**, named volume `pg-data`. Docs + blobs in that database. Not SQLite. Never omit `DATABASE_URL`. Never `USE_MEMORY_SQLITE`. |
| **Blob store** | Same Postgres via keck HTTP: `POST /api/blobs/venus-m0`, `GET /api/blobs/venus-m0/:hash`. |
| **Snapshot command** | `curl -sSSf http://127.0.0.1:3000/api/block/venus-m0/export -o /tmp/venus-m0.yjs` |

## Forbidden imports

Always forbidden in `apps/web`:

- `@affine/core`, `@affine/graphql`, copilot / AI packages, `@affine/nbstore` as a product dependency
- AFFiNE Cloud Socket.IO protocol
- Docusaurus / VitePress / AFFiNE explorer as a TOC

`mount-editor.js` / `editor-container.js` / `boot.js` must not import OctoBase, `y-websocket`, Hocuspocus, `y-indexeddb`, `y-protocols`, `lib0`, or blob HTTP clients.

`y-protocols` / `lib0` and `OctoBaseKeckProvider` live in `src/host/providers/` and `package.json`. OctoBase must not be an npm dependency of `@venus/web` (AGPL server process only). Do not add `y-websocket` unless the backend kind changes.
