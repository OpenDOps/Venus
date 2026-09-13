# CRDT implementation

Prototype **stack and dataflow** for the live page. Product meaning of the CRDT vs git: [datamodel](../datamodel/README.md). Product rules: [venus-design.md](../venus-design.md). System map: [architecture.md](../architecture.md). Words: [glossary.md](../glossary.md). Actuals: [api-map.md](../api-map.md). Hub **process** (run, Rust, SQL, HTTP): [hub](../components/hub/).

This is **not** a second CRDT. BlockSuite already owns a Y.Doc. Venus syncs that doc. Proof: [scenarios](../../scenarios/README.md).

**M1 (done)** proved the wire on OctoBase **keck**. **[M3.0](../M3.0/README.md)** is in progress: step 1 locks a Venus-owned **Rust + y-octo** apply engine (same `AFFiNE` + y-protocols). Live CRDT HA (wiki sticky): [M3.0/high-availability.md](../M3.0/high-availability.md). Client editor is BlockSuite only — no Rust/WASM in the tab ([wasm.md](./wasm.md)). Do not switch to nbstore, Hocuspocus, or stock `y-websocket` server.

## Stack (M3.0)

M1 Actuals proved the wire. Product **Sync server** is Compose `hub`; persist tables are Venus `crdt_*`; client kind stays `'octobase'` as a wire alias. How the process runs: [hub](../components/hub/).

| Piece | What we use | Not |
|---|---|---|
| Client CRDT | `yjs@13.6.32` on `store.spaceDoc` | A Venus-owned CRDT, markdown-as-Y.Text |
| Editor | BlockSuite 0.22.4 `TestWorkspace` | `@affine/core` |
| Sync seam | `SyncProvider` (`memory` \| `octobase` \| `y-websocket`) | Server imports in `mount-editor.js` |
| JS client | `OctoBaseKeckProvider`: `y-protocols/sync` + `lib0`, `new WebSocket(url, ['AFFiNE'])` | npm OctoBase client; stock `y-websocket` `WebsocketProvider` |
| Sync server | Venus **hub** (Compose `hub`, Rust + y-octo) | OctoBase keck; host `cargo run` as DoD; AFFiNE Cloud / nbstore |
| Persist | **Hosted:** Postgres 16 (Compose `postgres`, `pg-venus-data`) | IndexedDB as hosted refresh truth; SQLite |
| Blobs | hub `POST`/`GET /api/blobs/77e4a2b1-8b40-5979-a73c-fd4477216d00` (bytes in the same Postgres) | `blob:` URLs only |
| Doc export | `GET /api/block/77e4a2b1-8b40-5979-a73c-fd4477216d00/export` → Yjs update v1 | Markdown, `T0`, git |
| Optional decode | y-octo `0.1.0` or Node `Y.applyUpdate` | `@affine/native` |

Ids: workspace `77e4a2b1-8b40-5979-a73c-fd4477216d00` (v5 of `venus-m0`) = hub room `/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00`. Page `doc:home` = that room’s `spaceDoc`. SQL `doc_id` is UUID v5 of `doc:home`.

## Seam

The **protocol** is Yjs update v1 (`y-protocols/sync`). That is what “y-websocket-shaped” means in the design.

M1 **does not** run the `y-websocket` npm server. keck’s handshake requires `Sec-WebSocket-Protocol: AFFiNE`. Stock `WebsocketProvider` cannot set that, so it will not connect.

```text
M1 hosted (legacy)            browser ── AFFiNE WS ── keck ── Postgres jwst
Product (M3.0)                browser ── AFFiNE WS ── hub  ── Postgres crdt_*
On device (later)             app ── hub + local SQLite (optional sync to hosted hub)
```

M1: keck **was** the WS front. **M3.0:** the hub is the WS front (**Rust + y-octo**); Venus tables; sockets for one wiki stick on **`workspace_id`**. Cloud does **not** sit behind Hocuspocus or nbstore. Tests: [sync seam](../../scenarios/sync-seam.md).

## Share between clients

```text
Tab A types
  → BlockSuite applies to local Y.Doc
  → OctoBaseKeckProvider sends update v1 on the AFFiNE socket
  → hub applies (y-octo) and fans out to other sockets on that workspace
  → Tab B’s provider applies the update
  → Tab B’s editor shows the same blocks (no reload)
```

Refresh: connect → wait until `synced` → if the store already has `affine:page`, do not seed. Postgres (via the hub) is the refresh source. [M1 hydrate](../M1/plan.md#4-step-hydrate). Tests: [hydrate and persist](../../scenarios/hydrate-persist.md), [collaboration](../../scenarios/collaboration.md).

The hub batches writes (~1s) and compact in the background. After a write, wait ≥2s before restarting the hub if you are testing persist.

## Persist

```text
Yjs updates  ──hub──►  Postgres  (crdt_update / crdt_snapshot)
image bytes  ──HTTP──►  Postgres  (blob, same DB)
```

The hub is **stateless** besides Postgres and in-memory rooms. `docker compose down` without `-v` keeps `pg-venus-data`. Omitting Postgres env makes the hub refuse to start — that is required. Image bytes: [blobs](../../scenarios/blobs.md).

## Doc export (M1 step 7)

Browsers already have the tree over the socket. **Other clients** (curl, sidecar, later lease/git) are not in that session. They read the **current** Y.Doc:

```bash
curl -sSSf http://127.0.0.1:3000/api/block/77e4a2b1-8b40-5979-a73c-fd4477216d00/export -o /tmp/venus-page.yjs
```

```text
curl / sidecar / Venus service
        │  GET /api/block/77e4a2b1-8b40-5979-a73c-fd4477216d00/export
        ▼
hub  (bytes already in Postgres / live RAM encode)
```

The hub does **not** ask browsers. This is not a named version. A **pin** keeps an export (or a replica encode); lease `T0` is that pin at acquire. [LiveSnapshot](../LiveSnapshot/README.md). [M1 step 7](../M1/plan.md#7-step-snapshot). How we test: [doc export](../../scenarios/doc-export.md) (`apps/web/src/host/snapshot.test.ts`).

## Testing

| Piece | Scenarios |
|---|---|
| Seam / `OctoBaseKeckProvider` | [Sync seam](../../scenarios/sync-seam.md) |
| Share between clients | [Collaboration](../../scenarios/collaboration.md) |
| Hydrate / Postgres as refresh source | [Hydrate and persist](../../scenarios/hydrate-persist.md) |
| Blobs | [Blobs](../../scenarios/blobs.md) |
| Doc export | [Doc export](../../scenarios/doc-export.md) |

## What this folder is not

- Markdown adapter, pane, or sidecar ids — [MDGate](../MDGate/README.md), then [M2](../M2/README.md).
- **What spaces and git hold** — [datamodel](../datamodel/README.md).
- Hub **process** (run, Rust, SQL, HTTP) — [hub](../components/hub/). Software architecture: [architecture](../components/hub/architecture.md). Crate map: [files](../components/hub/files.md). Milestone: [M3.0](../M3.0/README.md).
- No Rust in the editor — [wasm.md](./wasm.md).
- Pin + git snapshotter — [LiveSnapshot](../LiveSnapshot/README.md), then [M3](../M3/README.md) (after M3.0).
- Lease freeze — M5.
