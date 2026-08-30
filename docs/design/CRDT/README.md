# CRDT implementation

Prototype **stack and dataflow** for the live page. Product meaning of the CRDT vs git: [datamodel](../datamodel/README.md). Product rules: [venus-design.md](../venus-design.md). System map: [architecture.md](../architecture.md). Words: [glossary.md](../glossary.md). Actuals: [api-map.md](../api-map.md).

This is **not** a second CRDT. BlockSuite already owns a Y.Doc. Venus syncs that doc. Proof: [scenarios](../../scenarios/README.md).

## Stack (M1)

| Piece | What we use | Not |
|---|---|---|
| Client CRDT | `yjs@13.6.32` on `store.spaceDoc` | A Venus-owned CRDT, markdown-as-Y.Text |
| Editor | BlockSuite 0.22.4 `TestWorkspace` | `@affine/core` |
| Sync seam | `SyncProvider` (`memory` \| `octobase` \| `y-websocket`) | Server imports in `mount-editor.js` |
| M1 client | `OctoBaseKeckProvider`: `y-protocols/sync` + `lib0`, `new WebSocket(url, ['AFFiNE'])` | npm OctoBase client; stock `y-websocket` `WebsocketProvider` |
| Sync server | OctoBase **keck** (Compose `octobase`) | Host `cargo run`; AFFiNE Cloud / nbstore |
| Persist | **Postgres 16** (Compose `postgres`, volume `pg-data`) | SQLite, IndexedDB as source of truth |
| Blobs | keck `POST`/`GET /api/blobs/venus-m0` (bytes in the same Postgres) | `blob:` URLs only |
| Doc export | `GET /api/block/venus-m0/export` → Yjs update v1 | Markdown, `T0`, git |
| Optional decode | y-octo `0.1.0` or Node `Y.applyUpdate` | `@affine/native` |

Ids: workspace `venus-m0` = keck room `/collaboration/venus-m0`. Page `doc:home` = that room’s `spaceDoc`.

## Seam

The **protocol** is Yjs update v1 (`y-protocols/sync`). That is what “y-websocket-shaped” means in the design.

M1 **does not** run the `y-websocket` npm server. keck’s handshake requires `Sec-WebSocket-Protocol: AFFiNE`. Stock `WebsocketProvider` cannot set that, so it will not connect.

```text
M1 (this repo)     browser ── AFFiNE WS ── keck ── Postgres
Cloud (later)      same SyncProvider kind 'y-websocket'
                   browser ── y-websocket/Hocuspocus ── own Postgres
                   OctoBase gone; y-octo optional for merge/export
```

Do not write “y-websocket saves into OctoBase.” OctoBase keck **is** the WS front. Postgres is the store. Cloud **replaces** keck; it does not sit behind it. Tests: [sync seam](../../scenarios/sync-seam.md).

## Share between clients

```text
Tab A types
  → BlockSuite applies to local Y.Doc
  → OctoBaseKeckProvider sends update v1 on the AFFiNE socket
  → keck applies and fans out to other sockets on venus-m0
  → Tab B’s provider applies the update
  → Tab B’s editor shows the same blocks (no reload)
```

Refresh: connect → wait until `synced` → if the store already has `affine:page`, do not seed. Postgres (via keck) is the refresh source. [M1 hydrate](../M1/plan.md#4-step-hydrate). Tests: [hydrate and persist](../../scenarios/hydrate-persist.md), [collaboration](../../scenarios/collaboration.md).

keck batches writes (~1s) and `full_migrate`s on socket close. After a write, wait ≥2s before restarting keck if you are testing persist.

## Persist

```text
Yjs updates  ──keck──►  Postgres  (docs)
image bytes  ──HTTP──►  Postgres  (blobs, same DB)
```

keck is **stateless** besides `DATABASE_URL`. `docker compose down` without `-v` keeps `pg-data`. Omitting `DATABASE_URL` makes keck fall back to SQLite — that is a bug in this product. Image bytes: [blobs](../../scenarios/blobs.md).

## Doc export (M1 step 7)

Browsers already have the tree over the socket. **Other clients** (curl, sidecar, later lease/git) are not in that session. They read the **current** Y.Doc:

```bash
curl -sSSf http://127.0.0.1:3000/api/block/venus-m0/export -o /tmp/venus-m0.yjs
```

```text
curl / sidecar / Venus service
        │  GET /api/block/venus-m0/export
        ▼
keck  (bytes already in Postgres)
```

keck does **not** ask browsers. This is not a named version. A **pin** keeps an export (or a replica encode); lease `T0` is that pin at acquire. [LiveSnapshot](../LiveSnapshot/README.md). [M1 step 7](../M1/plan.md#7-step-snapshot). How we test: [doc export](../../scenarios/doc-export.md) (`apps/web/src/host/snapshot.test.ts`).

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
- Pin + git snapshotter — [LiveSnapshot](../LiveSnapshot/README.md), then M3.
- Lease freeze — M5.
