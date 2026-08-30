# M1 — OctoBase loop

| | |
|---|---|
| **planId** | `m1-octobase-loop` |
| **Milestone** | [M1 in the implementation plan](../venus-implementation-plan.md#m1--octobase-loop-week) |
| **Duration** | About a week |
| **Encoding** | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B) |
| **Board** | [M1.state.yaml](./M1.state.yaml) — steps 1–3 `done` |

Parent design: [venus-design.md](../venus-design.md). Tool choices and bindings: [venus-implementation-plan.md](../venus-implementation-plan.md). Licensing: [licensing.md](../../legal/licensing.md). M0 host: [M0/plan.md](../M0/plan.md). Installed symbols: [api-map.md](../api-map.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

## Story

As an implementer I need the M0 page to **sync**: **Postgres in Docker** holds the Y.Doc (and blobs); **OctoBase keck in a second Docker** is the WebSocket front. Refresh restores what I typed, and a second browser tab sees the same blocks (including one image). Venus still has no git, lease, catalog, or markdown pane.

If two clients cannot share one BlockSuite page, later milestones have nothing to attach to.

## Exit

All of these must be true at once:

1. **Postgres** and **OctoBase keck** each run as their **own Docker Compose service**. The browser talks to keck over **WebSocket** (`AFFiNE` subprotocol). Postgres is not exposed to the browser.
2. With sync env enabled, type in the editor, **reload**: the typed text is still there (opposite of M0).
3. Two tabs (or two windows) on the same origin edit `doc:home`; each sees the other’s typing without refresh.
4. One **image** uploaded in tab A is visible in tab B (blob store, not only a local object URL).
5. A **y-octo** (or equivalent Yjs-binary) snapshot of that doc is reachable from this repo (`curl` or a small native binary). Not markdown.
6. `SyncProvider` is still the only editor-facing seam. `mount-editor` does not import the server.
7. No `@affine/core`, nbstore, GraphQL, or copilot.
8. OctoBase keck stays an **external AGPL container**, not a dependency of the web app. [licensing.md](../../legal/licensing.md).
9. [api-map.md](../api-map.md) Actual column is filled for every design name the code uses.

M0 + M1 together are “simple BlockSuite + OctoBase deployment”: Compose with **`postgres` + `octobase` + `web`**, one workspace, **Postgres is the refresh source**. IndexedDB is optional and not required to close this plan. SQLite is not the M1 store (recon used it only for a throwaway spike).

## Non-goals (do not start)

| Later | Why not M1 |
|---|---|
| Markdown pane, adapter fixtures, sidecars of block ids | M2 |
| `wiki/` git, flush, autocomment | M3 |
| Folder tree, catalog CRDT, product header | M4 |
| Lease, freeze, CodeMirror | M5 |
| Review After/Before/Diff | M6 |
| Auth, display names, awareness UI | v1 can wait |
| IndexedDB as the source of truth | Optional cache only; server (Postgres) wins on refresh |
| SQLite as the product store | Recon spike only; M1 persist is Postgres in Docker |
| AFFiNE Cloud / Socket.IO / `nbstore` | Different protocol; pulls the product |
| Second page / page list | Still one doc |
| Edgeless as a product surface | v1 non-goal |

Do not build a wiki sidebar, kanban, or a second TOC. Outline stays the in-page heading list from M0.

## Constraints

1. **Thin host.** Same Vite + React app. BlockSuite stays web components. Do not wrap blocks in React.
2. **Swappable seam, locked backend.** Talk Yjs binaries + spaces + blobs, not OctoBase block-schema REST from Venus. M1 backend is **OctoBase keck** ([api-map Chosen backend](../api-map.md)). The `SyncProvider` kind stays `'octobase' | 'y-websocket' | 'memory'` so a later swap does not rewrite `mount-editor`. Do **not** implement `y-websocket` in this milestone.
3. **Seam.** New networking only through `SyncProvider` (and a sibling `BlobSource` wired on the store, not in `mount-editor`). `createM0Workspace(provider)` already exists; pass a live provider from the app. Keep `MemoryNoopProvider` as the default when sync env is unset so Vitest and existing M0 Playwright specs stay green.
4. **One collection, one page.** Workspace id `venus-m0` (keck room). Page id `doc:home` (`store.spaceDoc` is the Y.Doc on that room). Do not create a second doc “for later.”
5. **Do not double-seed.** M0 always `addBlock`s on load. Two clients doing that on an empty server produce two page roots. After sync, seed **only if** the store has no `affine:page` root. Prefer: wait for the provider’s synced event, then seed or attach. Playwright two-tab tests must open the second page **after** the first has a root.
6. **Pin `yjs` 13.6.32.** Same override as M0. Pair y-octo with that codec (update v1).
7. **AGPL isolation.** OctoBase is its **own Docker image/service**. Postgres is a **second** image (`postgres:16`). Neither is in `apps/web/package.json`. Keep MPL notices on BlockSuite.
8. **No `@affine/core`.** Copying AFFiNE’s `KeckProvider` from old `@affine/workspace` is not a license to import the app shell. A 50-line Y.Doc ↔ WS bridge in `src/host/providers/` is the intended shape.
9. **Docker is the runtime.** M1 sync is Compose, not `cargo run --bin keck` on the host. Host cargo is recon history only.
10. **Later milestones keep this runtime.** M2+ assume Compose **`postgres` + `octobase`**. Do not switch the product store to SQLite or merge keck and Postgres into one container.

## Target tree

Only create what M1 needs. Do **not** add empty `packages/catalog`, `packages/review`, or `wiki/`. A **small** `crates/venus-sidecar` (snapshot only) is in scope; do not add git2 yet.

```text
Venus/
  docker-compose.yml              # step 2: postgres + octobase; step 8 adds web
  deploy/
    NOTICE                        # AGPL: OctoBase keck image
    octobase/Dockerfile           # build keck from pinned git SHA
  crates/venus-sidecar/           # optional: y-octo decode of keck export
  scripts/m1-recon-spike.mjs      # throwaway; delete before M1 exit
  apps/web/
    src/host/
      workspace.js                # hydrate: wait sync, seed if empty
      sync-provider.js            # interface + MemoryNoopProvider
      providers/
        from-env.js               # Memory vs octobase from VITE_*
        octobase-keck-provider.js # yjs + y-protocols + AFFiNE WS
        blob-source.js            # BlobSource → keck /api/blobs
    e2e/
      m0-*.spec.ts                # still pass when VITE_SYNC_URL is unset
      m1-hydrate.spec.ts
      m1-two-tabs.spec.ts
      m1-blob.spec.ts
      m1-smoke.spec.ts
  docs/design/api-map.md          # shared; sync Actuals filled in step 1
  docs/design/M1/
    …
```

## Binding (what you are proving)

```text
Tab A / Tab B  (BlockSuite Store → Y.Doc)
        │  Yjs update binary (y-protocols/sync)
        ▼
SyncProvider (kind octobase)
        │  WebSocket + subprotocol AFFiNE
        ▼
OctoBase keck  (Compose service `octobase`)  :3000  /collaboration/venus-m0
        │  DATABASE_URL=postgres://…@postgres:5432/…
        ▼
Postgres       (Compose service `postgres`)  named volume
        ├── docs (Yjs updates)
        └── blobs (image bytes)

keck HTTP: POST/GET /api/blobs/venus-m0
           GET /api/block/venus-m0/export
```

Ids:

```text
TestWorkspace.id  =  venus-m0   =  keck /collaboration/:workspace
doc:home          =  store.spaceDoc on that room (one Y.Doc)
```

## Chosen stack

Locked in [step-recon-sync](#1-step-recon-sync). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.

| Piece | Actual |
|---|---|
| Editor | BlockSuite **0.22.4**, `TestWorkspace`, `store.spaceDoc`, `SyncProvider.connect(docId, ydoc)` |
| Sync server | OctoBase **keck** @ `276e0e94719a652483119c5fea16be13293ee21c` in Compose service **`octobase`** |
| Database | **Postgres** in Compose service **`postgres`** (`postgres:16`). Docs + blobs. Not SQLite. |
| JS client | `OctoBaseKeckProvider`: `yjs@13.6.32` + `y-protocols@1.0.7` + `lib0`; `new WebSocket(url, ['AFFiNE'])`. Not `y-websocket`. |
| Out of scope | AFFiNE Cloud, `nbstore`, Socket.IO, `@affine/core`, host `cargo run` as the product server |

### Storage (Postgres in Docker)

keck’s **M1 store is Postgres**, in a **separate container** from keck. Same database holds Yjs docs and blobs (`BlobStorageType::DB`).

| | |
|---|---|
| **Engine** | Postgres 16 (`jwst-storage` feature `postgres`; keck default features include it) |
| **How keck finds it** | `DATABASE_URL=postgres://venus:venus@postgres:5432/jwst?sslmode=disable` (Compose). Hostname is the `postgres` service. |
| **Volume** | Named volume on Postgres data dir (`pg-data:/var/lib/postgresql/data`). **Not** a keck `./data/jwst.db` volume. |
| **What lives there** | Yjs workspace updates **and** blob bytes |
| **SQLite** | Recon spike only (host binary, `/tmp/venus-keck/data/jwst.db`). **Forbidden** as the M1 product store. Never `USE_MEMORY_SQLITE`. Never omit `DATABASE_URL` (that would fall back to SQLite). |
| **MySQL / Redis / S3** | Out of M1 |
| **Flush** | keck batches doc writes (~1s) and `full_migrate`s on socket close. After a spike write, wait **≥2s** before restarting, or the persist test will flake. |

y-octo is MIT (`0.1.0`): `Doc::try_from_binary_v1`, `encode_update_v1`. M1 snapshot is keck `GET /api/block/venus-m0/export` (Yjs update v1); a sidecar is optional decode, not a second store.

## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).

| # | id | Adds |
|---|---|---|
| 1 | `step-recon-sync` | **Done.** [api-map](../api-map.md) + **OctoBase keck**. Spike proved two `Y.Doc`s on `venus-m0`. |
| 2 | `step-server` | Compose **`postgres` + `octobase`**. Restart without `-v` does not wipe the doc. |
| 3 | `step-provider` | A live `SyncProvider` the App can pass in. Editor mount stays ignorant of the server. Memory provider remains the no-env default. |
| 4 | `step-hydrate` | Load order: connect → wait until synced → seed only if the page is empty. Refresh restores typed text (opposite of M0). |
| 5 | `step-two-clients` | Two tabs share one CRDT over the socket. Typing in A appears in B without reload. |
| 6 | `step-blobs` | One image goes through `blobSync` to HTTP storage. The other tab and a reload still show the pixels. |
| 7 | `step-snapshot` | `GET /api/block/venus-m0/export` (Yjs binary). Not markdown. Later lease `T0` will reuse this. |
| 8 | `step-compose` | Same Compose file adds **`web`**: `postgres` + `octobase` + `web`. |
| 9 | `step-verify` | Close-out: person in Chrome/Firefox plus Playwright smoke. Marks the board `done`. |

---

## Steps

Do them in order (1–9). A step is not started until its `dependsOn` steps are done. Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

### 1. step-recon-sync

| | |
|---|---|
| **n** | 1 |
| **id** | `step-recon-sync` |
| **title** | Map sync server, JS client, blobs, snapshot |
| **dependsOn** | (none; M0 closed) |
| **kind** | implement |
| **status** | **done** ([board](./M1.state.yaml); breakpoint `human`) |

**Adds:** a decision and a map, not a feature in the app. You know which process to run, which JS client talks Yjs update v1, where blobs go, and how to fetch a snapshot. A two-`Y.Doc` spike proves the wire works before BlockSuite is involved.

Documentation plus a spike, not product UI. M1 dies if you code against AFFiNE Cloud while keck speaks Yjs, or if you seed before `synced`.

#### Work

1. Read OctoBase README / keck building guide (`cargo run --bin keck`, `/api/docs` with `JWST_DEV=1`). Note last commit date and whether the tree still builds on this machine.
2. Decide **kind**. Write it in [api-map.md](../api-map.md) **Chosen backend**. Product persist is **Postgres in Docker**; keck is a **second** container. Do not choose SQLite as the store. Do not implement `y-websocket` in M1 (the seam kind stays on the interface for a later swap).
3. Spike (throwaway, delete before exit or never commit): connect a `Y.Doc` to the server; send `Y.encodeStateAsUpdate`; apply a remote update. Log bytes. No BlockSuite UI required. A host-SQLite keck is allowed **only** for this spike.
4. Find blob HTTP (keck swagger). Find snapshot: keck REST **or** dump the Yjs update to a file and decode with y-octo. Blobs live in the same Postgres as docs once step 2 lands.
5. Fill every **Actual** cell in [api-map.md](../api-map.md). Record WS URL, room naming (`venus-m0` / `doc:home`), and the client class.
6. Confirm `store.blobSync` / `BlobSource` on affine 0.22.4 (`TestWorkspace` blob engine). Record image flavour (`affine:image` vs attachment).

#### Do not

- Import `@affine/core` or `nbstore` to “save time.”
- Start the React two-tab UI here.
- Vendor the OctoBase repo into `apps/web`.

#### Test scenarios

1. **Map complete**
   - **Given** the repo after this step, with no leftover “likely” cells for sync.
   - **When** a reviewer opens [api-map.md](../api-map.md) **Chosen backend** and the **Names — sync, blobs, snapshot** Actual column.
   - **Then** all of these are concrete strings (not empty, not “recon: …”): `kind`, why, **Server start** (one command), **Client package** (npm name + class), WS URL, room/workspace id (`venus-m0`) and space/doc id (`doc:home`), blob endpoint or “Venus PUT /blobs/:id”, **Snapshot command**.
   - **How:** read the file. Fail if any of those fields are still the template italic placeholders.

2. **Spike syncs**
   - **Given** the documented server is running, and two clients (Node scripts or two browser consoles) each have a `Y.Doc` connected with the Actual client to the same room as `doc:home` (or the recon room name).
   - **When** client A inserts `ydoc.getMap('spike').set('k', 'v')` (or `Y.encodeStateAsUpdate` + apply on A so B must receive a non-empty update).
   - **Then** within **5 seconds**, client B’s doc has the same map value (or `Y.encodeStateAsUpdate(B)` is non-trivial and applying A’s update to a third empty doc matches).
   - **How:** throwaway spike (delete before M1 exit). Log both sides. Not Playwright. Not BlockSuite.

#### Done

Chosen backend is **OctoBase keck** (`kind: octobase`), not stock `y-websocket`. Pin, WS URL, blob REST, snapshot curl, `BlobSource` / `affine:image`, and Chosen backend are in [api-map.md](../api-map.md). Spike: two Node `Y.Doc`s on `ws://127.0.0.1:3000/collaboration/venus-m0` with subprotocol `AFFiNE`; A set `spike.k=v`, B saw `v` in 192ms (`scripts/m1-recon-spike.mjs` — delete before M1 exit). That spike talked to a **host** keck using **SQLite** (`/tmp/venus-keck/data/jwst.db`). **Step 2 replaces that with Compose Postgres + a keck container.** keck SHA `276e0e94719a652483119c5fea16be13293ee21c`. Do not use `USE_MEMORY_SQLITE`.

DoD evidence:

| Scenario | How |
|---|---|
| Map complete | [api-map.md](../api-map.md) Chosen backend + Names — sync Actual column: no italic `recon:` placeholders |
| Spike syncs | 2026-08-29 `node scripts/m1-recon-spike.mjs` → `SPIKE OK` (B in 192ms) against keck on `:3000` |

---

### 2. step-server

| | |
|---|---|
| **n** | 2 |
| **id** | `step-server` |
| **title** | Compose: Postgres + OctoBase keck |
| **dependsOn** | `step-recon-sync` |
| **kind** | implement |

**Adds:** two Docker services in this repo. **`postgres`** holds Yjs docs and blobs. **`octobase`** is keck (AGPL), talks to Postgres over the Compose network, and exposes WS/HTTP on `:3000`. Vite `:5173` can open the WebSocket (keck CORS already lists it). Neither image is an npm dep of `@venus/web`.

Recon already proved the Yjs wire (`scripts/m1-recon-spike.mjs` against a hand-started SQLite keck). This step is **the product runtime**: a clone of Venus starts **both containers**, and a restart **without deleting the Postgres volume** keeps `spike.k`.

#### Two containers (required)

```text
docker-compose.yml
  postgres:     image postgres:16
                volume pg-data:/var/lib/postgresql/data
                healthcheck pg_isready
                do not publish 5432 unless debugging (keck uses the Compose DNS name)

  octobase:     build deploy/octobase/Dockerfile
                depends_on postgres (condition: service_healthy)
                environment:
                  DATABASE_URL=postgres://venus:venus@postgres:5432/jwst?sslmode=disable
                  KECK_PORT=3000
                ports: "3000:3000"
                no SQLite volume; no USE_MEMORY_SQLITE
```

| | `postgres` | `octobase` (keck) |
|---|---|---|
| Image | `postgres:16` (pin the digest or tag in Compose) | Venus `deploy/octobase/Dockerfile` — `git clone` OctoBase @ `276e0e9…`, `cargo build -p keck` |
| Persist | named volume `pg-data` | **stateless** besides the DB URL |
| Host port | none (default) | `3000` |
| License | PostgreSQL | AGPL-3.0 (`deploy/NOTICE`) |

keck **must** set `DATABASE_URL`. If that env is missing, keck falls back to `./data/jwst.db` (SQLite). That fallback is a **bug** in this milestone.

`depends_on` + Postgres **healthcheck** so keck does not boot before `pg_isready`. If keck starts too early, migrations fail and the persist test is noise.

#### Layout

```text
deploy/NOTICE                      # OctoBase keck image is AGPL-3.0
deploy/octobase/Dockerfile         # clone pinned SHA; cargo build -p keck --release
docker-compose.yml                 # postgres + octobase (web is step 8)
```

Clone OctoBase **at image build time**. Do not vendor it into `apps/web`. A submodule under `deploy/` is allowed; still keep it out of the web package.

One command from the Venus root (api-map **Server start** and the runbook):

```bash
docker compose up --build postgres octobase
```

Optional root scripts `"sync:up"` / `"sync:down"`. Do not start these from `@venus/web`’s `dev` script — memory-mode Vite stays the default.

**Not DoD:** `cargo run --bin keck` on the host (that was recon; it used SQLite). Mention it in a runbook footnote only.

#### Work

1. `deploy/octobase/Dockerfile` from the api-map SHA. Compose service **`octobase`**: `DATABASE_URL` pointing at service **`postgres`**, publish `3000:3000`, `depends_on` + healthcheck.
2. Compose service **`postgres`**: `postgres:16`, `POSTGRES_USER` / `PASSWORD` / `DB` matching the DSN, named volume `pg-data`. Do not use SQLite. Do not put keck and Postgres in one container.
3. `deploy/NOTICE`: OctoBase keck image is AGPL-3.0. Compose comments may repeat the one-liner.
4. Replace api-map **Server start**, **Storage**, and **Compose** with the repo command, DSN, volume name, and service names (`postgres`, `octobase`).
5. Runbook: **Sync (M1)** section — `docker compose up --build postgres octobase`, down, health, “do not `down -v` if you need the doc”.
6. Health (no swagger): `curl -sSSf -X POST http://127.0.0.1:3000/collaboration/venus-m0` → `{"protocol":"AFFiNE"}`. `JWST_DEV` off by default. Postgres is healthy via `pg_isready` (Compose healthcheck), not a host port.
7. CORS: keck already allows Vite `:5173`. **No Vite proxy** unless the browser cannot open `ws://127.0.0.1:3000`.
8. Persist: `docker compose up` → `node scripts/m1-recon-spike.mjs` → wait ≥2s → `docker compose restart octobase` (Postgres stays up) → a new client still sees `spike.k=v`. Then `docker compose down` **without** `-v` and `up` again — still there. Fail if you used SQLite or IndexedDB.

#### Do not

- SQLite, `USE_MEMORY_SQLITE`, or omitting `DATABASE_URL`.
- One container that runs both Postgres and keck.
- Redis, MySQL, AFFiNE Cloud.
- `pnpm add` OctoBase into `@venus/web`.
- Add the **web** Compose service yet (step 8).
- Make `pnpm dev` require Docker.
- Treat host `cargo run --bin keck` as Server up.

#### Test scenarios

1. **Server up**
   - **Given** this repo and Docker.
   - **When** you run api-map **Server start** (`docker compose up --build postgres octobase`) and wait until keck logs `listening on 0.0.0.0:3000` and Postgres is healthy.
   - **Then** `docker compose ps` shows **two** running services (`postgres` and `octobase`). `curl -sSSf -X POST http://127.0.0.1:3000/collaboration/venus-m0` returns `{"protocol":"AFFiNE"}`. A WebSocket to `ws://127.0.0.1:3000/collaboration/venus-m0` with subprotocol `AFFiNE` is not connection-refused.
   - **How:** runbook recipe. No UI. Fail if only one container is up. Fail if keck is a host cargo process. Fail if the command is still “clone OctoBase in `/tmp`”.

2. **Persists**
   - **Given** both services are up with the named **Postgres** volume (not a keck SQLite file).
   - **When** you run `node scripts/m1-recon-spike.mjs` (A writes `spike.k=v`), wait at least **2 seconds**, then `docker compose restart octobase` (Postgres keeps running). Then `docker compose down` without `-v` and `docker compose up -d postgres octobase` again.
   - **Then** after **each** restart, a **new** `Y.Doc` on `ws://127.0.0.1:3000/collaboration/venus-m0` (`AFFiNE`) has `getMap('spike').get('k') === 'v'` (or `GET /api/block/venus-m0/export` is >2 bytes and contains the spike).
   - **How:** spike or curl export. Fail if the doc is empty. Fail if persistence was `./data/jwst.db` or IndexedDB. Fail if you needed `down -v` to “fix” anything.

---

### 3. step-provider

| | |
|---|---|
| **n** | 3 |
| **id** | `step-provider` |
| **title** | Live SyncProvider behind the M0 seam |
| **dependsOn** | `step-server` |
| **kind** | implement |

**Adds:** the M0 `SyncProvider` seam grows `OctoBaseKeckProvider` (`kind: 'octobase'`). The App picks it from env; Vitest still gets `MemoryNoopProvider`. `mount-editor.js` still has no server imports. The editor can be attached to keck; hydrate (next step) is what waits and seeds.

#### Work

1. `src/host/providers/octobase-keck-provider.js`: class `OctoBaseKeckProvider` implementing `SyncProvider` with `kind: 'octobase'`. `connect(docId, ydoc)` opens `new WebSocket(url, ['AFFiNE'])` and runs `y-protocols/sync` on `ydoc` (the spike is the protocol sketch). `disconnect` closes the socket. Expose `synced` / `whenReady()` after SyncStep2. `Doc` from `yjs` as today.
2. `from-env.js`: if `VITE_SYNC_URL` (or the Actual env name) is unset, return `MemoryNoopProvider`. If set, return `OctoBaseKeckProvider`. App uses this; `createM0Workspace` keeps its default memory argument for tests.
3. Do **not** edit `mount-editor.js` except if a comment is required. Do not pass OctoBase types into the editor container.
4. Pin `y-protocols` (and `lib0` if not already reachable) as **direct** deps of `@venus/web`. Keep `yjs` 13.6.32. Do **not** add `y-websocket`.
5. Update Vitest that currently forbids sync clients in **all** host files: forbid them in `mount-editor.js` / `editor-container.js` / `boot.js` only. `providers/` may import `y-protocols`. Still forbid `@affine/core` everywhere.

#### Do not

- Delete `MemoryNoopProvider`.
- Default `pnpm test` (Node Vitest) to opening a real socket.
- Change outline mount.

#### Test scenarios

1. **Seam holds**
   - **Given** `apps/web/src/host/mount-editor.js` (and `editor-container.js`, `boot.js`).
   - **When** you search for `y-websocket`, `octobase`, `hocuspocus`, `y-indexeddb`, and blob HTTP client imports (`from '…'`).
   - **Then** none of those files import them.
   - **How:** extend `apps/web/src/host/sync-provider.test.ts` (or a sibling) so the forbid-list applies to those three files only. `src/host/providers/` **may** import the live client.

2. **Env switch**
   - **Given** Vitest with no `VITE_SYNC_URL`.
   - **When** `createM0Workspace()` is called with no provider argument.
   - **Then** `provider.kind === 'memory'` and no `WebSocket` is constructed.
   - **How:** existing `sync-provider.test.ts` pattern; keep it green.

   - **Given** the sync server from step 2 is running, and the App is built/served with `VITE_SYNC_URL` set to the api-map WS URL (or Vite proxy path).
   - **When** the browser loads `/`.
   - **Then** the workspace’s `provider.kind` is `'octobase'`, and DevTools → Network → WS shows **one** socket to `ws://127.0.0.1:3000/collaboration/venus-m0` (or the Vite-env URL) with subprotocol `AFFiNE` (not “no WS”).
   - **How:** Playwright `e2e/m1-provider.spec.ts` **or** a headed check recorded in recon notes for this step; prefer an automated assert on `page.evaluate(() => …)` if you expose kind on `window` for tests only — otherwise assert WS URL in `page.on('websocket')`. Fail if `kind === 'memory'` while env is set.

---

### 4. step-hydrate

| | |
|---|---|
| **n** | 4 |
| **id** | `step-hydrate` |
| **title** | Sync first, seed only if empty, refresh keeps text |
| **dependsOn** | `step-provider` |
| **kind** | implement |

**Adds:** correct load order on the live provider. The host waits until the server has spoken, then either attaches to the existing `affine:page` or seeds once. Refresh no longer wipes the note. M0’s “always `addBlock` in `load`” is gone for sync mode.

This is the host change M0 did not have. M0 `store.load(() => addBlock…)` always constructs the tree.

#### Work

1. After `createDoc` / `getStore`, `connect` the provider, **wait until synced** (timeout with a clear error if the server is down).
2. If `store.root` is already `affine:page` (or Actual “has page tree”), **do not** add seed blocks. Call `store.load()` only if the Actual API requires it without a constructor callback.
3. If empty: run the existing `seedHomeNote` path (title `Venus`, H1 `Why Venus`, H2 `Empty host`, spacers), then `resetHistory()`.
4. Two clients racing on an empty room: document the rule (open one tab first). Optionally a Y.Map flag `venus:seeded` so the second writer skips. Do not merge two `affine:page` roots.
5. Playwright `e2e/m1-hydrate.spec.ts`: start Vite **with** `VITE_SYNC_URL`; type `hello` into the note paragraph; reload; `hello` is still in the note. Do not run this spec in the memory project.
6. Keep `e2e/m0-provider.spec.ts` on memory: reload still **drops** text. Playwright projects or env: memory vs sync.

#### Do not

- Seed inside `connect` before remote updates apply.
- Use IndexedDB to fake refresh while the WS is a no-op.

#### Test scenarios

1. **Refresh keeps**
   - **Given** sync server up, Vite (or Compose web) with `VITE_SYNC_URL` set, Playwright Chromium, spec **not** in the memory project.
   - **When** the test opens `/`, waits until `doc-title` is `Venus` and outline H1 is `Why Venus`, clicks `affine-note affine-paragraph rich-text` (a body paragraph, not the title), types `hello`, waits until the note contains `hello`, then `page.reload()` and waits for the editor again.
   - **Then** the note still contains `hello`, `doc-title` is still `Venus`, and `[data-testid="outline-block-preview-h1"]` still has `Why Venus`.
   - **How:** `apps/web/e2e/m1-hydrate.spec.ts`. Fail if text is gone (M0 behavior). Timeout: 30s for first paint after reload.

2. **No double page**
   - **Given** the same server after **Refresh keeps** has already seeded `doc:home` (title + H1 exist on the server).
   - **When** a **new** browser context opens `/` (second session, same `VITE_SYNC_URL`).
   - **Then** there is exactly **one** `doc-title` with `Venus`, exactly **one** outline H1 `Why Venus` (not two H1s, not “Why VenusWhy Venus”). `store.root.flavour === 'affine:page'` once (assert via `page.evaluate` on the workspace if exposed, or count H1 previews `toHaveCount(1)`).
   - **How:** same file, second test, or a dedicated test that hydrates twice. Do not clear the sync volume between the two sessions.

3. **Memory unchanged**
   - **Given** Playwright **memory** project: `VITE_SYNC_URL` unset, no requirement that Docker is up.
   - **When** `apps/web/e2e/m0-provider.spec.ts` runs (type `hello`, reload).
   - **Then** after reload, `hello` is **absent** and seed H1/H2 are back (M0 contract).
   - **How:** existing spec must stay in the memory project. Fail this step if you “fixed” M0 tests by pointing them at the sync server.

---

### 5. step-two-clients

| | |
|---|---|
| **n** | 5 |
| **id** | `step-two-clients` |
| **title** | Two tabs, same page |
| **dependsOn** | `step-hydrate` |
| **kind** | implement |

**Adds:** live collaboration on `doc:home`. Two editors, one Y.Doc on the server. Not BroadcastChannel, not `localStorage`.

#### Work

1. Playwright `e2e/m1-two-tabs.spec.ts`: two `page`s (same browser, two tabs). Tab A waits until seed/H1 visible. Tab B opens `/`. Tab A types a unique string into the note. Tab B sees that string without reload (timeout ~5–10s). Optionally type in B and assert A.
2. Manual path in step-verify covers Chrome/Firefox two windows.
3. If merge conflicts on text, Yjs is doing its job; DoD is “both see a consistent CRDT,” not character-perfect OT.

#### Do not

- Use BroadcastChannel-only “sync” that dies when tabs do not share a process.
- Add user avatars or awareness UI.

#### Test scenarios

1. **A→B**
   - **Given** sync env, server up, Playwright: `const pageA = page`, `const pageB = await context.newPage()` (same origin).
   - **When** A goes to `/` and waits until outline H1 `Why Venus` is visible; **then** B goes to `/` and waits until the same H1 is visible; A clicks `affine-note affine-paragraph rich-text` and types `from-a`; **B does not reload**.
   - **Then** within **10 seconds**, B’s note locator contains `from-a` (`expect(pageB.getByText('from-a')).toBeVisible({ timeout: 10_000 })` or the note `rich-text` has that string).
   - **How:** `apps/web/e2e/m1-two-tabs.spec.ts`. Fail if B only sees it after reload. Fail if A and B never share a WebSocket to the server.

2. **Same seed**
   - **Given** both tabs have finished hydrate (after **A→B** setup, before or after typing).
   - **When** you read A and B: `doc-title` text and `[data-testid="outline-block-preview-h1"]`.
   - **Then** both titles are `Venus`, both H1 labels are `Why Venus`, and each page has **one** H1 preview (not two).
   - **How:** same spec, second `test()`. Optional third test: B types `from-b`, A sees `from-b` within 10s without A reloading.

---

### 6. step-blobs

| | |
|---|---|
| **n** | 6 |
| **id** | `step-blobs` |
| **title** | One image upload through blob sync |
| **dependsOn** | `step-two-clients` |
| **kind** | implement |

**Adds:** binary assets on the same page. `store.blobSync` talks keck `POST`/`GET /api/blobs/venus-m0` (bytes in **Postgres**, same DB as the Y.Doc). An `affine:image` in tab A is pixels in tab B and after reload — not a blob: URL that dies with the tab.

Implementation plan: “Blobs: one image upload.” Binding: `collection.blobSync` = server blob store.

#### Work

1. Implement Actual `BlobSource` against the blob HTTP from recon. `set` uploads bytes; `get` downloads; `list`/`delete` as required by BlockSuite 0.22.4.
2. Attach it to the workspace/store **before** the editor mounts (workspace factory or App), not inside `mount-editor.js`.
3. Confirm default image insert (slash Image, paste, or drag) calls `blobSync.set` and stores `sourceId` on `affine:image` (or Actual flavour).
4. Blob server is keck (`octobase` container). Bytes land in **Postgres**. A memory `Map` in tab A is **not** enough.
5. Playwright `e2e/m1-blob.spec.ts`: tab A inserts a tiny fixture PNG (`setInputFiles` or paste); wait for `affine-image .affine-image-container img` in the note; tab B sees an `img` whose decoded size is > 0.
6. CORS already allows POST. If 413, fix Compose/proxy; do not stub the image in CSS. `BlobSource.list` may return `[]` (keck has no list route); `get(sourceId)` must work.

#### Do not

- Build a Venus image toolbar. Use BlockSuite’s image block.
- Store git assets (M3).

#### Test scenarios

1. **Upload**
   - **Given** sync env, two tabs as in step 5, a fixture PNG in the repo (e.g. `apps/web/e2e/fixtures/dot.png`, 1×1 or small).
   - **When** tab A inserts that file into the note (Playwright `setInputFiles` on the Actual file input, or paste — record the selector in api-map **Image in page**).
   - **Then** A shows a real image: locator `affine-image .affine-image-container img` `toBeVisible`, `naturalWidth > 0` after load. Network has a **POST** to `/api/blobs/venus-m0` (status 2xx). The image is not a 0-size broken icon.
   - **How:** `apps/web/e2e/m1-blob.spec.ts`. Fail on 413 without fixing proxy `client_max_body_size`.

2. **Second client**
   - **Given** A already has the image on the page (after **Upload**).
   - **When** tab B is open on `/` (opened after A’s image is in the CRDT, or already open — either is fine if WS is live). B does **not** pick a file.
   - **Then** within **15 seconds** B’s `affine-image .affine-image-container img` is visible with `naturalWidth > 0`.
   - **How:** same spec. Fail if B only has an empty image block or a `blob:` URL that 404s.

3. **Refresh**
   - **Given** A has the uploaded image.
   - **When** A `page.reload()` and waits for the editor.
   - **Then** the same image is visible again (`naturalWidth > 0`). Seed title/H1 still present.
   - **How:** same spec, third test. Fail if the block remains but the `src` is missing (Y.Doc without blob store).

---

### 7. step-snapshot

| | |
|---|---|
| **n** | 7 |
| **id** | `step-snapshot` |
| **title** | y-octo snapshot API reachable from Venus |
| **dependsOn** | `step-hydrate` |
| **kind** | implement |

**Adds:** an out-of-band Yjs binary of `doc:home` that Venus can fetch without the browser. That is the snapshot clock later milestones call `T0`. It is **not** `MarkdownAdapter` and not a button in the editor.

Implementation plan: “even if only a `curl` / native call.”

#### Work

1. Snapshot is keck REST: `curl -sSSf http://127.0.0.1:3000/api/block/venus-m0/export -o /tmp/venus-m0.yjs` (already in api-map). Optional: `crates/venus-sidecar` with y-octo `0.1.0` `Doc::try_from_binary_v1` / `encode_update_v1` if you want a native decode in-repo; not required if Node `Y.applyUpdate` covers **Decodes**.
2. Document the exact command in api-map **Snapshot command** (example: `curl -sS …` or `cargo run -p venus-sidecar -- snapshot --doc doc:home`).
3. Test: after seed (and optional typed text), the snapshot bytes are **non-empty** and a second decode round-trip succeeds. Node can `Y.applyUpdate(new Y.Doc(), bytes)` **or** the Rust binary exits 0. Put the test next to the sidecar or as a Vitest that shells out (skip if no Rust in CI — then the test must run in Compose CI later; for M1, local is enough, same as M0 e2e).
4. Do not wire snapshot into the editor UI.

#### Do not

- Export markdown or block-id sidecars (M2).
- Depend on AFFiNE’s `@affine/native` N-API package.

#### Test scenarios

1. **Reachable**
   - **Given** sync server up, `doc:home` hydrated at least once (seed present — run hydrate or the spike that left a page on the server).
   - **When** you run the exact command in api-map **Snapshot command** (example: `curl -sSSf … -o /tmp/home.bin` or `cargo run -p venus-sidecar -- snapshot --doc doc:home > /tmp/home.bin`).
   - **Then** the process **exit code is 0**, and the output file (or stdout redirected) has **length > 2** bytes (larger than a trivial empty Yjs update; seeded BlockSuite pages are typically hundreds of bytes or more).
   - **How:** a shell test or Vitest `execFileSync` in `crates/venus-sidecar` / `apps/web/src/host/snapshot.test.ts`. Skip in CI only if documented; M1 local must pass. Fail if the command is still the empty template.

2. **Decodes**
   - **Given** the bytes from **Reachable**.
   - **When** you load them with y-octo (`Doc::try_from_binary_v1` / `apply_update_from_binary_v1`) **or** `const d = new Y.Doc(); Y.applyUpdate(d, bytes)` in Node.
   - **Then** apply **does not throw**, and `Y.encodeStateAsUpdate(d).byteLength > 2` (doc is not empty).
   - **How:** same test file, second assertion. Fail if you only `stat` the file. Do not parse markdown.

---

### 8. step-compose

| | |
|---|---|
| **n** | 8 |
| **id** | `step-compose` |
| **title** | Compose: postgres + octobase + web |
| **dependsOn** | `step-server`, `step-provider` |
| **kind** | implement |

**Adds:** the documented M0+M1 deployment: three services, one command. A new machine can bring up **Postgres + keck + the host** without memorizing Vite flags (or the exception is written: `postgres`+`octobase` in Compose, web is `pnpm dev` + `.env`).

Implementation plan: “Docker Compose: `postgres` + `octobase` + `venus-web`.”

#### Work

1. Compose services: **`postgres`**, **`octobase`** (from step 2) and **`web`** (nginx or `pnpm preview` / static `apps/web/dist` with `VITE_SYNC_URL` baked or runtime env). If baking Vite env is painful, document: `docker compose up postgres octobase` + host `pnpm dev` with `.env` — **and** still ship a `web` service that works for the exit demo (even if it is a one-line image).
2. One workspace. **Postgres is the only database.** Do not add SQLite, Redis, or a second Postgres.
3. Runbook: copy-paste `docker compose up --build` (or Actual). Point at this plan’s manual test.
4. `.gitignore` does not ignore Compose files. Persist the **Postgres** volume as a named volume (`pg-data`), not anonymous. `docker compose down` vs `down -v` is documented.

#### Do not

- Add lease/git services.
- Bundle OctoBase **source** into the web image.

#### Test scenarios

1. **Three services**
   - **Given** the Compose file in the repo (`docker-compose.yml` — Actual path in api-map).
   - **When** you run `docker compose config --services`.
   - **Then** the list includes **`postgres`**, **`octobase`**, and **`web`** (or the api-map names).
   - **How:** `grep` in CI or a one-line script. Fail if Postgres or OctoBase is missing. Fail if keck and Postgres share one service.

2. **Loop from Compose**
   - **Given** `docker compose up --build` (or the runbook command) has reached healthy/up, and you open the **web service URL** from the runbook (not an undocumented host port).
   - **When** a browser has two tabs on that URL and you repeat **A→B** from step 5 (type `from-a` in A; B sees it without reload, 10s).
   - **Then** the same Then as **A→B** holds.
   - **How:** manual for this step is enough if Playwright still targets host Vite; **or** point `m1-two-tabs.spec.ts` `baseURL` at Compose web once. If web is **not** in Compose, the runbook must state `compose up postgres octobase` + `pnpm dev` with `VITE_SYNC_URL` — still documented as the M1 loop.

---

### 9. step-verify

| | |
|---|---|
| **n** | 9 |
| **id** | `step-verify` |
| **title** | Browser + smoke + snapshot |
| **dependsOn** | `step-two-clients`, `step-blobs`, `step-snapshot`, `step-compose` |
| **kind** | test |

**Adds:** nothing new in the product. It is the gate that M1 is done: a person walked the loop, automated smoke is green, the board is `done`.

#### Work

1. **Manual (required).** A person follows **Manual testing** below in Chrome or Firefox. Playwright does not replace this.
2. Playwright `e2e/m1-smoke.spec.ts`: `/` with sync env; outline H1 `Why Venus`; type `hello`; second tab sees `hello` **or** same tab reload keeps `hello` (if two-tab is already `m1-two-tabs.spec.ts`, smoke may be hydrate + blob smoke only — do not drop two-tab coverage).
3. `pnpm test:e2e` remains green for **memory** specs. Add `pnpm test:e2e:m1` (or a Playwright project) that requires Compose/sync. Document in runbook.
4. Delete recon spikes.
5. Set [M1.state.yaml](./M1.state.yaml) steps to `done` when each DoD is actually met. Fill api-map Actual leftovers.

#### Manual testing

Use a real browser (Chrome or Firefox), not a screenshot and not Playwright headed mode as a substitute. Ignore extension console noise. Fail on uncaught exceptions from the host or BlockSuite.

1. Start **postgres + octobase** (and **web**, or host `pnpm dev` with `VITE_SYNC_URL`) as in the runbook. Open the app URL.
2. **Seed.** Title `Venus`, H1 `Why Venus`, H2 `Empty host`, outline lists those headings (not folders).
3. **Type.** Click a note paragraph, type `hello`. Characters appear.
4. **Refresh.** Reload. `hello` is **still there**. Outline still matches (this fails M1 if it behaves like M0).
5. **Second tab.** Open the same URL in a second tab. It shows `hello` without typing. Type `tab-b` in B; A shows `tab-b` without reload.
6. **Image.** In A, insert an image (slash / paste). It renders. B shows the same image. Reload A; image remains.
7. **WS.** DevTools → Network → WS: a sync socket is open (not “no WS” like M0).
8. **Snapshot.** Run the api-map snapshot command; it succeeds.
9. **No AFFiNE shell.** `apps/web/package.json` still has no `@affine/core`. `mount-editor.js` still has no sync imports.
10. **Memory mode (optional sanity).** Unset sync env, `pnpm dev`: refresh **drops** text (M0 still works for people without Docker).

If a step fails, M1 is not done. Fix provider, hydrate, blobs, or the server; do not fake two tabs with `localStorage`.

#### Do not

- Call M1 done from one tab’s first paint.
- Add visual-regression baselines.

#### Test scenarios

1. **Manual path**
   - **Given** Chrome or Firefox (not Playwright headed as a substitute), sync + web started as in the runbook, extension console noise ignored.
   - **When** a person performs **Manual testing** above, in order, without skipping.
   - **Then** every numbered item holds, and the console has **no** uncaught exception from the host or BlockSuite (extension `contentscript.js` / MetaMask noise does not fail).
   - **How:** human. Record date + browser on [M1.state.yaml](./M1.state.yaml) `Manual path` evidence. Playwright does **not** satisfy this scenario.

2. **Smoke**
   - **Given** sync server up, `pnpm test:e2e:m1` (or Actual script / Playwright project `m1`) as documented in the runbook.
   - **When** that command runs against Vite or Compose web with `VITE_SYNC_URL` set.
   - **Then** exit code **0**. It must include (or the project must already have run) hydrate keep-on-reload **and** two-tab A→B **and** blob image on second client — if smoke is a thin file, those other `m1-*.spec.ts` files must be in the same command.
   - **How:** `apps/web/e2e/m1-smoke.spec.ts` plus the m1 Playwright project. `pnpm test:e2e` (memory) still exits 0 separately.

3. **Refresh + second client**
   - **Given** the m1 e2e project is green.
   - **When** you read `m1-hydrate.spec.ts` and `m1-two-tabs.spec.ts` results from **Smoke**.
   - **Then** both files passed in that run (not skipped). Equivalent: manual items 4–5 already passed under **Manual path**.
   - **How:** do not close M1 if hydrate/two-tabs are skipped due to “no Docker in CI” unless local evidence is in the yaml.

4. **Snapshot**
   - **Given** the same session as **Manual path** (or immediately after smoke, server still up, `doc:home` still seeded).
   - **When** you run api-map **Snapshot command** again.
   - **Then** exit 0 (same as step 7 **Reachable**).
   - **How:** one command in the verify checklist; yaml evidence is the command string + “exit 0”.

---

## Order of work (calendar)

| When | Steps |
|---|---|
| Day 1 | 1 `step-recon-sync` → 2 `step-server` |
| Day 2 | 3 `step-provider` → 4 `step-hydrate` |
| Day 3 | 5 `step-two-clients` |
| Day 4 | 6 `step-blobs` → 7 `step-snapshot` |
| Day 5 | 8 `step-compose` → 9 `step-verify` |

If `postgres` or `octobase` will not start from this repo, **stop and fix Compose** (keck image build, `DATABASE_URL`, Postgres healthcheck). Do not fall back to host SQLite or a `y-websocket` rewrite.

## Risks

| Risk | What to do in M1 |
|---|---|
| keck image / clang on macOS | Build keck **in Docker** (Linux) from the pinned SHA; do not use host cargo as DoD |
| Missing `DATABASE_URL` | keck would silently use SQLite — fail the step |
| Postgres not ready | `depends_on` + `pg_isready` healthcheck before keck starts |
| Seed twice, two page roots | Wait `synced`; seed iff empty; two-tab e2e opens B after A has H1 |
| M0 e2e breaks (refresh keeps text) | Memory default; Playwright project split |
| `yjs` duplicate major | Keep root override `13.6.32` |
| Blobs only in-memory | Second tab / refresh fail — HTTP blob store required |
| AGPL in the web bundle | OctoBase not in `apps/web/package.json` |
| Snapshot needs markdown | Wrong milestone — binary only |
| Custom elements / HMR | Same M0 guard; two tabs are two documents, not HMR |

## Handoff to M2

M2 may assume:

- Live `SyncProvider` + blob HTTP; refresh and second client work.
- Snapshot bytes of `doc:home` can be fetched (lease `T0` will reuse this, not invent a second store).
- Still no git, lease UI, catalog, or markdown pane.
- `MarkdownAdapter.fromDoc` on the **synced** store is the next slice, plus the adapter fixture suite.

M2 exit is WYSIWYG and a read-only markdown pane staying aligned. M1 exit is “the same Y.Doc in two places.”

## Invariants (M1 only)

1. One workspace, one page, page mode.
2. Outline = headings of that page (not a folder tree).
3. No AFFiNE application shell.
4. Postgres (via keck) is the refresh source when sync env is on; memory mode remains for tests.
5. No git, lease, catalog, markdown editor, or product header.
6. Provider seam stays swappable later (Hocuspocus / `y-websocket`) without rewriting `mount-editor`. M1 implements **only** `octobase`.
