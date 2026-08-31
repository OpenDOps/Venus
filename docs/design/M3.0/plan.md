# M3.0 — Venus hub (replace keck)

| | |
|---|---|
| **planId** | `m30-venus-hub` |
| **Milestone** | [M3.0 in the implementation plan](../venus-implementation-plan.md#m30--venus-hub-replace-keck-week) |
| **Duration** | About a week |
| **Encoding** | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B) |
| **Board** | [M3.0.state.yaml](./M3.0.state.yaml) |

Parent design: [venus-design.md](../venus-design.md). Live CRDT HA: [high-availability.md](./high-availability.md). Dataflow: [architecture.md](../architecture.md). CRDT stack: [CRDT/README.md](../CRDT/README.md). Words: [glossary.md](../glossary.md). Tool choices: [venus-implementation-plan.md](../venus-implementation-plan.md). Licensing: [licensing.md](../../legal/licensing.md). M1 (keck, done): [M1/plan.md](../M1/plan.md). M2 (done): [M2/plan.md](../M2/plan.md). Installed symbols: [api-map.md](../api-map.md). Keck recon (legacy): [octobase.md](../LiveSnapshot/octobase.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

## Story

As an implementer I need the M1/M2 page to **keep syncing** after we **remove OctoBase keck**: a Venus-owned **hub** applies Yjs, broadcasts to other tabs, and persists to **Postgres tables we own**. Refresh and a second client still see the same blocks (including one image). Export still returns update v1. Live collab still does not wait on markdown or git. Venus still has no `wiki/` writer (that is M3).

If the hub is not M1-parity, M3 pins garbage. If we keep keck, AGPL and JWST stay on the product path.

## Exit

All of these must be true at once:

1. **Postgres** and **Venus hub** each run as their **own Docker Compose service**. Product Compose is **`postgres` + `hub` + `web`**. There is **no** `octobase` service on the product path.
2. Browser talks Yjs over WebSocket with subprotocol **`AFFiNE`** to the hub (same client shape as M1). Postgres is not exposed to the browser.
3. Type in the editor, **reload**: typed text remains (M1 hydrate).
4. Two tabs edit `doc:home`; each sees the other’s typing without refresh. **Y.Text marks** (bold / link / color) fan out without a server panic.
5. One **image** uploaded in tab A is visible in tab B and after reload (blob HTTP on the hub, bytes in Postgres).
6. HTTP **doc export** (`GET` of Yjs update v1 for `venus-m0`). Current tree, not markdown, not `T0`. Reads Venus **SQL** (and/or live encode — Actual in api-map).
7. **`workspace_lease`**: a second hub process does not become a second live owner for `venus-m0` while the first holds the lease. SIGTERM flushes persist then drops the lease.
8. Persist of an update **upserts** `dirty(workspace_id, doc_id, clock)`. Hub does **not** write `jobs`. Trigger must not stall persist if `dirty` is missing (or install only after `dirty` exists).
9. `SyncProvider` is still the only editor-facing seam. `mount-editor` does not import the hub.
10. No `@affine/core`, nbstore, GraphQL, copilot, Hocuspocus, or stock `y-websocket` **server**. No JWST Block CRUD routes (export/blob URL compatibility is allowed).
11. Hub source is **Venus** (MIT/Apache on new files). Not an OctoBase fork. Not `jwst-rpc` linked in. [licensing.md](../../legal/licensing.md).
12. [api-map.md](../api-map.md) **Chosen backend** is the hub. Actual column filled for every name the code uses.
13. Existing **M1 Playwright** (`pnpm test:e2e:m1`) and M2 export tests stay green against the hub (update kind/URL asserts if `kind` becomes `'venus'`).

M2 pane still photographs the **synced Store**, not export.

## Non-goals (do not start)

| Later | Why not M3.0 |
|---|---|
| `wiki/` git, flush, autocomment | M3 |
| Folder tree, catalog CRDT, many pages per wiki on one socket | M4 |
| Lease freeze, CodeMirror | M5 |
| Review After/Before/Diff | M6 |
| k8s ingress, many hub replicas in prod | Devops after lease is proven; M3.0 may be one replica |
| S3 blobs | Optional later; M3.0 blobs in Postgres |
| On-device SQLite hub | Later; M3.0 is hosted Postgres |
| nbstore / Socket.IO | Different protocol |
| Convert / `fromDoc` inside the hub | MDGate stays in the host / M3 worker |
| Dual-write keck + hub | Cut over; re-seed `venus-m0` if the volume is new |

Do not reimplement Block REST. Do not vendor OctoBase.

## Constraints

1. **Thin host.** Same Vite + React app. BlockSuite stays web components.
2. **Same wire.** `y-protocols/sync` + `Sec-WebSocket-Protocol: AFFiNE` so M1’s client can stay or be renamed without a new handshake. Path `/collaboration/:workspace_id`. Port **3000** behind nginx/`web` like keck.
3. **Swappable seam.** `SyncProvider` kind may add `'venus'` (product) while keeping `'octobase'` as a deprecated alias **or** switch `from-env` to `'venus'`. `memory` stays the no-env default. Do **not** implement a `y-websocket` server.
4. **One apply queue per room**, one persist buffer per room — **not** per socket.
5. **One collection, one page** still: `venus-m0` / `doc:home`. Do not add catalog spaces.
6. **Pin `yjs` 13.6.32.** Pair y-octo / Node apply with that codec (update v1).
7. **No OctoBase in the hub image.** No `git clone toeverything/OctoBase`. No `jwst-*` crates. AGPL keck Dockerfile may remain under `deploy/octobase/` as history; product Compose must not build it.
8. **No `@affine/core`.**
9. **Docker is the runtime.** Hub is a Compose service, not `node dist` on the host as DoD (host run is recon only).
10. **HA file wins** for owner/dirty/drain: [high-availability.md](./high-availability.md). If plan prose disagrees, the HA file wins.

## Target tree

Only create what M3.0 needs. Do **not** add `wiki/`, catalog packages, or snapshotter workers.

```text
Venus/
  docker-compose.yml                 # postgres + hub + web  (no octobase)
  deploy/
    hub/Dockerfile
    web/                             # nginx still proxies /api and /collaboration → hub:3000
  apps/hub/                          # if Node (recon Actual)
    # or crates/venus-hub/           # if Rust + y-octo (recon Actual)
  apps/web/
    src/host/providers/
      from-env.js                    # Memory vs hub URL
      venus-hub-provider.js          # or keep octobase-keck-provider.js if byte-identical
      blob-source.js                 # same /api/blobs paths
    e2e/
      m1-*.spec.ts                   # still pass against hub
      m2-*.spec.ts
  docs/design/api-map.md             # Chosen backend = venus hub
  docs/design/M3.0/
    …
```

## Binding (what you are proving)

```text
Tab A / Tab B  (BlockSuite Store → Y.Doc)
        │  y-protocols/sync
        ▼
SyncProvider (kind venus | octobase alias)
        │  WebSocket + subprotocol AFFiNE
        ▼
Venus hub  (Compose service `hub`)  :3000  /collaboration/venus-m0
        │  DATABASE_URL → postgres
        ▼
Postgres
        ├── crdt_snapshot / crdt_update
        ├── blob
        ├── workspace_lease
        └── dirty     (trigger on persist; no jobs yet)

curl / sidecar / later M3
        │  GET …/export   (path Actual; M1 curl may stay as alias)
        ▼
hub → SQL (or RAM encode) → Yjs update v1
```

Ids stay:

```text
TestWorkspace.id  =  venus-m0   =  hub /collaboration/:workspace
doc:home          =  store.spaceDoc on that room (one Y.Doc)
```

## Chosen stack

Locked in [step-recon-hub](#1-step-recon-hub). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.

| Piece | Intent (Actual in recon) |
|---|---|
| Editor | Unchanged: BlockSuite **0.22.4**, `TestWorkspace`, `store.spaceDoc` |
| Sync server | **Venus hub**, Compose **`hub`**, listen `0.0.0.0:3000` |
| Apply | `Y.applyUpdate` (Node) **or** y-octo `apply_update_from_binary_v1` (Rust). Recon picks one. |
| Database | **Postgres 16**, Compose **`postgres`**. Venus tables above. Not SQLite. Not `jwst` docs schema. |
| JS client | Same framing as `OctoBaseKeckProvider`. New class optional. `new WebSocket(url, ['AFFiNE'])`. |
| Out of scope | OctoBase keck, AFFiNE Cloud, nbstore, Socket.IO, `@affine/core` |

### Storage

| | |
|---|---|
| **Engine** | Postgres 16 |
| **DSN** | Compose `DATABASE_URL` to service `postgres`. Must not be omitted (no SQLite fallback). |
| **Volume** | Named volume. M3.0 **may** use a new volume name so keck `jwst` rows are not mistaken for hub rows. Document in api-map. Re-seed the demo wiki. |
| **Flush** | ~1s batch. After a write, wait **≥2s** before restarting the hub for persist tests. |
| **Compact** | Background. Not on the insert hot path. |

## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).

| # | id | Adds |
|---|---|---|
| 1 | [`step-recon-hub`](#1-step-recon-hub) | Language + apply engine; api-map Chosen backend **venus hub**; spike two `Y.Doc`s without keck. |
| 2 | [`step-store`](#2-step-store) | `crdt_snapshot` / `crdt_update` / `blob`; hydrate, flush, compact tests; no WS required. |
| 3 | [`step-ws`](#3-step-ws) | `AFFiNE` WebSocket: apply, broadcast, persist ~1s; marks do not panic. |
| 4 | [`step-compose-hub`](#4-step-compose-hub) | Compose **`postgres` + `hub`**. Restart hub without `-v` keeps the doc. **No** `octobase` on this path. |
| 5 | [`step-provider`](#5-step-provider) | App talks to hub. Editor mount stays ignorant. Memory default unchanged. |
| 6 | [`step-parity`](#6-step-parity) | M1 hydrate, two tabs, blobs, export green on the hub. |
| 7 | [`step-ha-owner`](#7-step-ha-owner) | `workspace_lease`; second process is not a second owner; SIGTERM drain. |
| 8 | [`step-dirty`](#8-step-dirty) | SQL trigger → `dirty`. Hub does not write `jobs`. |
| 9 | [`step-verify`](#9-step-verify) | Close-out: person + Playwright; product Compose has no keck. |

---

## Steps

Do them in order (1–9). A step is not started until its `dependsOn` steps are done. Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

### 1. step-recon-hub

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-store) · [3](#3-step-ws) · [4](#4-step-compose-hub) · [5](#5-step-provider) · [6](#6-step-parity) · [7](#7-step-ha-owner) · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 1 |
| **id** | `step-recon-hub` |
| **title** | Map hub language, apply engine, wire, schema |
| **dependsOn** | (none; M2 closed) |
| **kind** | implement |
| **status** | pending |

**Adds:** a decision and a map, not product UI. You know which binary to run, that it speaks `AFFiNE` + y-protocols, where rows live, and that OctoBase is not linked.

Prefer **Node + `yjs` + `y-protocols`** if that is the fastest path to drop-in client parity. Prefer **Rust + y-octo** if recon proves the same handshake in comparable time. Write the winner in api-map. Do not ship both.

#### Work

1. Spike (throwaway ok): two `Y.Doc`s, `y-protocols` Step1/Step2/Update, apply on the server side **without** keck. Log bytes. Prove bold/`Format` apply does not throw.
2. Decide **kind** (`venus`) and **Chosen backend** in [api-map.md](../api-map.md): listen address, WS URL, export path (keep `GET /api/block/venus-m0/export` as an alias if that keeps M1 tests), blob paths, DSN, volume name.
3. Record: one persist buffer per room; no Block REST; no `jwst-rpc`.
4. Fill HA pointers: [high-availability.md](./high-availability.md) is the owner/dirty contract.

#### Do not

- Import `@affine/core` or `nbstore`.
- Clone OctoBase into the hub.
- Start the React two-tab UI here (spike Node/Rust only).
- Implement git or `fromDoc` in the hub.

#### Test scenarios

1. **Map complete**
   - **Given** this repo.
   - **When** you read [api-map.md](../api-map.md) Chosen backend.
   - **Then** it says **venus hub** (not keck SHA), with WS URL, export curl, blob paths, `DATABASE_URL`, Compose service **`hub`**.
   - **How:** grep / review. Fail if Chosen backend is still only OctoBase keck.

2. **Spike syncs**
   - **Given** a throwaway hub (host process ok).
   - **When** client A sets a map key (or a Y.Text mark); client B is on the same room.
   - **Then** B sees the value without keck running (`docker compose ps` has no `octobase`, or keck is stopped).
   - **How:** script like `scripts/m1-recon-spike.mjs` pointed at the hub. Fail if the spike still talks to keck.

---

### 2. step-store

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · **2** · [3](#3-step-ws) · [4](#4-step-compose-hub) · [5](#5-step-provider) · [6](#6-step-parity) · [7](#7-step-ha-owner) · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 2 |
| **id** | `step-store` |
| **title** | Postgres CRDT tables: hydrate, flush, compact |
| **dependsOn** | `step-recon-hub` |
| **kind** | implement |

**Adds:** durable Yjs bytes in Venus tables. No WebSocket required. Restarting the store process reloads the same binary.

#### Work

1. Migrations: `crdt_snapshot`, `crdt_update`, `blob` (lease/dirty can wait for steps 7–8).
2. API in-process: `pushUpdate(workspace, docId, bin)`, `getDoc(workspace, docId) → snapshot squash`, `putBlob` / `getBlob`.
3. Compact: merge updates into snapshot off the insert path (timer or count threshold in background).
4. Tests against Compose Postgres or testcontainers — not SQLite as the product dialect (a sqlite test double is allowed only if marked non-product).

#### Do not

- Write into keck `docs` / `jwst` schema.
- Compact inside every `INSERT` under a global lock (keck’s 500-row pattern).
- Expose Postgres port to the browser.

#### Test scenarios

1. **Round-trip**
   - **Given** empty tables and a Yjs update v1 that contains a map `spike.k=v`.
   - **When** you `pushUpdate` then `getDoc` then `Y.applyUpdate` in Node.
   - **Then** `getMap('spike').get('k') === 'v'`.
   - **How:** hub unit/integration test. Fail if bytes were stored as JSON blocks.

2. **Restart store**
   - **Given** a pushed update flushed to Postgres.
   - **When** you drop the in-memory cache (new process) and `getDoc` again.
   - **Then** the same value is present.
   - **How:** same test suite. Fail if hydrate only worked because RAM was kept.

---

### 3. step-ws

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · **3** · [4](#4-step-compose-hub) · [5](#5-step-provider) · [6](#6-step-parity) · [7](#7-step-ha-owner) · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 3 |
| **id** | `step-ws` |
| **title** | AFFiNE WebSocket: apply, broadcast, persist |
| **dependsOn** | `step-store` |
| **kind** | implement |

**Adds:** the merge buffer. Two clients share one RAM doc; persist drains ~1s into step-2 tables.

#### Work

1. `POST /collaboration/:workspace` → `{ "protocol": "AFFiNE" }` (M1 health).
2. `GET` upgrade with `ws.protocols(['AFFiNE'])`. Handshake: Doc Step1 + awareness like keck (client already sends Step1 on open).
3. Apply on **one queue per workspace**. Broadcast updates to other sockets. Skip empty frames if you must match keck.
4. Persist buffer **per workspace**, flush ~1s. Not a task per socket.
5. Do not stringify CRDT history. Do not implement `/api/block/:id/:block`.

#### Do not

- Per-connection `save_update` duplication.
- Block REST children/flavour routes.
- Pause persist for anything.

#### Test scenarios

1. **A→B**
   - **Given** hub + Postgres from step 2, two Node `Y.Doc`s, `AFFiNE` WS.
   - **When** A writes `spike.k=v` (or a BlockSuite-free Y.Text).
   - **Then** B sees it without reload (order of ~200ms is ok).
   - **How:** recon spike against the hub. Fail if keck is required.

2. **Marks**
   - **Given** a Y.Text with a `Format` (bold) in the update.
   - **When** A applies it and B syncs.
   - **Then** the hub process does not panic/crash; B’s doc still applies.
   - **How:** spike or unit apply. Fail if the server dies (keck Format class of bug).

3. **Persist after WS**
   - **Given** A wrote over WS; wait ≥2s.
   - **When** you restart **only** the hub process (Postgres stays).
   - **Then** a new client hydrates `spike.k=v` (or export contains it).
   - **How:** spike + restart. Fail if the doc lived only in RAM.

---

### 4. step-compose-hub

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · [3](#3-step-ws) · **4** · [5](#5-step-provider) · [6](#6-step-parity) · [7](#7-step-ha-owner) · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 4 |
| **id** | `step-compose-hub` |
| **title** | Compose: postgres + hub |
| **dependsOn** | `step-ws` |
| **kind** | implement |

**Adds:** two Docker services on the product path. **`web` may still proxy to `hub:3000`.** Do not require `octobase` to pass this step.

#### Work

1. `deploy/hub/Dockerfile` from the recon stack. Service **`hub`**: `DATABASE_URL` → `postgres`, publish `3000:3000`, `depends_on` + healthcheck.
2. Remove **`octobase`** from the default `docker compose up` path (or stop building it). Comment in Compose: keck is M1 history.
3. `deploy/NOTICE`: hub license (MIT/Apache). Do not claim the product image is AGPL keck.
4. Health: `POST /collaboration/venus-m0` → `{"protocol":"AFFiNE"}`.
5. Persist: spike → wait ≥2s → `docker compose restart hub` → still there; `down` without `-v` → still there.

#### Do not

- SQLite / omit `DATABASE_URL`.
- One container for Postgres + hub.
- Keep `octobase` as a required service for `pnpm sync:up`.

#### Test scenarios

1. **Server up**
   - **Given** this repo and Docker.
   - **When** you run api-map **Server start** (`docker compose up --build postgres hub`) until hub listens `:3000` and Postgres is healthy.
   - **Then** `docker compose ps` shows **postgres** and **hub**. `curl -sSSf -X POST http://127.0.0.1:3000/collaboration/venus-m0` returns `{"protocol":"AFFiNE"}`. No `octobase` container is required.
   - **How:** runbook. Fail if keck is the process on `:3000`.

2. **Persists**
   - **Given** both services and the named volume.
   - **When** spike writes, wait ≥2s, `restart hub`, then `down` without `-v` and `up` again.
   - **Then** a new client still sees the spike (or export decodes it).
   - **How:** spike / export. Fail if persistence was keck `jwst` docs from a leftover volume without a documented cutover.

---

### 5. step-provider

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · [3](#3-step-ws) · [4](#4-step-compose-hub) · **5** · [6](#6-step-parity) · [7](#7-step-ha-owner) · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 5 |
| **id** | `step-provider` |
| **title** | SyncProvider talks to the hub |
| **dependsOn** | `step-compose-hub` |
| **kind** | implement |

**Adds:** the App uses the hub when `VITE_SYNC_URL` is set. `mount-editor.js` still has no server imports.

#### Work

1. Point `from-env.js` at the hub URL (same `VITE_SYNC_URL` / `same-origin` as M1). Class may stay `OctoBaseKeckProvider` if the wire is identical, or `VenusHubProvider` with `kind: 'venus'`.
2. BlobSource still `POST/GET /api/blobs/venus-m0` (hub implements those paths).
3. Vitest: forbid hub/keck imports in `mount-editor.js` / `editor-container.js` / `boot.js`.
4. Update e2e kind asserts if kind changes.

#### Do not

- Delete `MemoryNoopProvider`.
- Default `pnpm test` to a real socket.
- Import hub types into the editor container.

#### Test scenarios

1. **Seam holds**
   - **Given** `mount-editor.js`, `editor-container.js`, `boot.js`.
   - **When** you search for hub/keck/`y-websocket`/`y-protocols` imports.
   - **Then** none of those files import them.
   - **How:** extend `sync-provider.test.ts`.

2. **Env switch**
   - **Given** unset `VITE_SYNC_URL` → `kind === 'memory'`.
   - **Given** hub up and `VITE_SYNC_URL` set.
   - **When** the browser loads `/`.
   - **Then** WS to `/collaboration/venus-m0` with `AFFiNE`. Kind is `'venus'` or documented `'octobase'` alias.
   - **How:** Playwright provider spec (update M1 spec or add `m30-provider.spec.ts`).

---

### 6. step-parity

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · [3](#3-step-ws) · [4](#4-step-compose-hub) · [5](#5-step-provider) · **6** · [7](#7-step-ha-owner) · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 6 |
| **id** | `step-parity` |
| **title** | M1 hydrate, two tabs, blobs, export on the hub |
| **dependsOn** | `step-provider` |
| **kind** | implement |

**Adds:** product close of M1 behavior with keck gone.

#### Work

1. Hub: `POST/GET/HEAD/DELETE /api/blobs/:workspace` (and `/:hash`) as M1. Hash = BlockSuite `sha()` (SHA-256 base64url with padding).
2. Export: implement the api-map **Export command** (keep the M1 curl path if tests depend on it).
3. CORS / Vite proxy: Playwright `:5174` must reach blobs (same-origin `/api` proxy like M1).
4. Run `pnpm test:e2e:m1` (and export Vitest) against hub. Fix product code, not by starting keck.

#### Do not

- `fromDoc` for export.
- Markdown in Postgres.

#### Test scenarios

1. **Refresh keeps**
   - **Then** `m1-hydrate.spec.ts` passes with hub (type `hello`, wait ≥2s, reload).
   - **How:** `PLAYWRIGHT_M1=1` with `VITE_SYNC_URL` → hub.

2. **A→B**
   - **Then** `m1-two-tabs.spec.ts` passes (no keck).

3. **Blobs**
   - **Then** `m1-blob.spec.ts` passes (POST `/api/blobs/venus-m0`, second tab + reload).

4. **Export**
   - **Then** api-map Export command writes >2 bytes; `Y.applyUpdate` succeeds (`snapshot.test.ts` or equivalent). Fail if the handler is still keck Block REST implementation copied as a black box from OctoBase source.

---

### 7. step-ha-owner

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · [3](#3-step-ws) · [4](#4-step-compose-hub) · [5](#5-step-provider) · [6](#6-step-parity) · **7** · [8](#8-step-dirty) · [9](#9-step-verify)

| | |
|---|---|
| **n** | 7 |
| **id** | `step-ha-owner` |
| **title** | One live owner per workspace_id (lease + drain) |
| **dependsOn** | `step-parity` |
| **kind** | implement |

**Adds:** the thin instance of [high-availability.md](./high-availability.md): `workspace_lease`, refuse a second owner, flush on SIGTERM.

#### Work

1. Table `workspace_lease(workspace_id, owner, lease_until)`. Hub id = hostname+pid or random `owner`.
2. On first WS for a workspace: `INSERT` lease or steal if expired. Heartbeat TTL (e.g. 15–30s).
3. If lease is held by another live owner: do **not** apply (503/close WS). Do not split-brain.
4. SIGTERM: flush persist buffers, `DELETE`/`UPDATE` lease, then exit.
5. Document: gateway will hash `workspace_id` later; M3.0 Compose may still be one replica. Prove with a **second** hub process on another port against the **same** Postgres.

#### Do not

- Cookie/IP sticky as the owner algorithm.
- Redis as a Y.Doc.
- Two owners “for HA.”
- k8s as a required DoD (optional note in devops).

#### Test scenarios

1. **Second owner refused**
   - **Given** hub A holds `venus-m0`; hub B is a second process, same `DATABASE_URL`.
   - **When** a client tries `/collaboration/venus-m0` on B.
   - **Then** B does not apply a second live doc (connection fails, or redirects, or waits — Actual). Postgres has one lease row for `venus-m0`.
   - **How:** Compose profile `hub-b` or `docker compose run` second container. Fail if both apply and clients diverge.

2. **Drain**
   - **Given** a write on A; SIGTERM A immediately after (before 1s tick if possible).
   - **When** A exits.
   - **Then** persist was flushed **or** the test documents remaining loss ≤ one batch **and** lease is gone so B can become owner and hydrate SQL.
   - **How:** script. Fail if lease stays forever (`lease_until` in the past must be stealable).

---

### 8. step-dirty

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · [3](#3-step-ws) · [4](#4-step-compose-hub) · [5](#5-step-provider) · [6](#6-step-parity) · [7](#7-step-ha-owner) · **8** · [9](#9-step-verify)

| | |
|---|---|
| **n** | 8 |
| **id** | `step-dirty` |
| **title** | Postgres trigger upserts dirty on persist |
| **dependsOn** | `step-ha-owner` |
| **kind** | implement |

**Adds:** `dirty(workspace_id, doc_id, clock)` for M3. No snapshotter, no `jobs`.

#### Work

1. Table `dirty`. `AFTER INSERT` (and snapshot replace if that path writes) on `crdt_update` → `UPSERT` clock.
2. Map `workspace_id` + `doc_id`: M3.0 `doc_id` may equal workspace id (one page) — document the map for M3 many pages.
3. Trigger fail-safe: missing `dirty` must not roll back persist (or install trigger only after the table exists).
4. Hub code does **not** insert `jobs`.

#### Do not

- Poll export to mark dirty.
- Write dirty on apply/broadcast (RAM only).
- Pause persist.

#### Test scenarios

1. **Upsert**
   - **Given** empty `dirty`.
   - **When** a WS client writes and persist flushes (≥2s).
   - **Then** `dirty` has one row for `venus-m0` / `doc:home` (or documented ids) with a clock that moves on a second write (upsert, not a second row).
   - **How:** `psql` in CI or a hub integration test. Fail if hub inserted a `jobs` row.

2. **Persist if dirty missing**
   - **Given** trigger uninstalled or `dirty` dropped (as documented).
   - **When** persist runs.
   - **Then** `crdt_update` still lands (or the runbook only installs trigger after `dirty` exists — pick one and test that).
   - **How:** migration test.

---

### 9. step-verify

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-hub) · [2](#2-step-store) · [3](#3-step-ws) · [4](#4-step-compose-hub) · [5](#5-step-provider) · [6](#6-step-parity) · [7](#7-step-ha-owner) · [8](#8-step-dirty) · **9**

| | |
|---|---|
| **n** | 9 |
| **id** | `step-verify` |
| **title** | Close-out: hub is the product collab server |
| **dependsOn** | `step-dirty` |
| **kind** | implement |

**Adds:** board `done`. keck is not on the product Compose path.

#### Work

1. Runbook: `docker compose up --build` → postgres + hub + web. `pnpm sync:up` → postgres + hub.
2. Person: type, reload, second tab, image, export curl, optional bold mark.
3. `pnpm test:e2e:m1` and M2 tests green.
4. Confirm `docker compose config --services` lists `postgres`, `hub`, `web` — not `octobase`.
5. Mark [M3.0.state.yaml](./M3.0.state.yaml) steps 1–9 `done` with evidence.

#### Do not

- Start M3 `wiki/` writer.
- Leave keck as the documented Server start in api-map.

#### Test scenarios

1. **Three services**
   - **Then** Compose services are `postgres`, `hub`, `web`.
   - **How:** `docker compose config --services` / Vitest compose test updated.

2. **Smoke**
   - **Then** M1 smoke + two-tabs + blob + hydrate pass against hub; M2 pane still works (memory or hub).
   - **How:** `pnpm test:e2e:m1`; `pnpm test` mdgate.

3. **Manual path**
   - **Then** runbook M3.0 close-out items held (same shape as M1 close-out, keck renamed).
   - **How:** person + evidence on the board.

---

## After M3.0

[M3 — Git snapshotter](../venus-implementation-plan.md#m3--git-snapshotter-week): pin from hub export/replica, `fromDoc` off live Store, dirty already in Postgres. Gate: this plan **done** + [LiveSnapshot HA Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) (snapshotter beside **hub**, not keck).
