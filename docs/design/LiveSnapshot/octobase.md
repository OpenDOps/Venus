# OctoBase and pinning

What pinned keck (`276e0e94719a652483119c5fea16be13293ee21c`) actually does. Policy: [README.md](./README.md). Pins and endpoints: [api-map.md](../api-map.md).

Venus does **not** fork keck for a pin-WAL. Cloud replaces keck anyway ([CRDT — seam](../CRDT/README.md#seam)).

## Three pipes (already separate)

```text
WS binary
  → apply_change (batch ~100ms) → in-memory Workspace.sync_messages
  → replies on that socket + broadcast channel → other clients

broadcast raw content
  → save_update thread: HashMap<guid, Vec<update>> in RAM
  → every ~1s drain → docs.update_doc → Postgres (insert blob rows;
     compact after ~500 updates per guid)

GET /api/block/:workspace/export
  → docs.get_doc(workspace_id) from Postgres
  → encode_update_v1
  → does not encode the live Workspace cache
```

Source: `jwst-rpc` `context.rs` (`apply_change`, `save_update`), `jwst-storage` `JwstStorage::export_workspace` / `full_migrate`, keck `export_workspace` HTTP.

## Does pinning stuck CRDT work?

**Apply + broadcast: no**, if Venus pins a **copy** (sidecar replica or GET bytes) and converts off that copy. Export does not take the live Workspace lock; it reads SQL.

**Persist: already delayed ~1s**, independent of export. Incoming updates still land in the in-memory Workspace and still fan out while `save_update` holds blobs in a `Mutex<HashMap>`. That RAM map is the closest thing keck has to “buffer not yet flushed to store.” It is **always** on; it is not gated on a Venus pin.

**Encode of the live Workspace:** `full_migrate` calls `workspace.sync_migration()` and can hit “wait transact timeout.” That is compact-to-SQL, not Venus pin. Socket close calls `full_migrate(..., force=false)` (also rate-limited to 5s unless `force`).

## What keck cannot do

| Wanted | Stock keck |
|---|---|
| Pause Postgres writes until pin copy finishes, then flush | No API. `save_update` ticks every 1s regardless of HTTP export |
| Export the live memory doc as a named snapshot | `GET …/export` rebuilds from **Postgres**, so it can trail the live doc by the persist batch |
| Atomic pin of many workspaces | One GET per workspace id |
| Notify Venus of dirty `docId`s | No; Venus tracks clocks / replica updates |

So: “hold the persist buffer during pin, still sync clients, flush when pin ends” is **not** a keck feature. Live sync already ignores persist. The missing piece is only a **consistent copy of now** for Venus, which the snapshotter owns.

## How Venus should pin against this keck

1. **Sidecar replica (preferred for “now”).** Connect like any client. Pin = clone encode of that `Y.Doc`. Apply/broadcast/persist on keck stay untouched. Matches the invariant: pin is Venus memory.
2. **GET export (OK for M3 idle).** After 30–120s idle the ~1s persist batch is done. Pin = those SQL bytes. Do not use this as `T0` without waiting or using (1); otherwise lease Before can miss the last second of typing.
3. **Do not** PATCH keck to skip `update_doc` during convert. Crash during a long `fromDoc` would drop durability for no gain: the pin already has its bytes.

M1 already documents: wait ≥2s after a write before restarting keck if you need persist. Same number is a decent lower bound before GET-export as a pin if the replica is not used.

## Cloud

Hocuspocus / y-websocket + own Postgres can implement copy-on-write persist later. The Venus pin interface stays: `{ docId, bytes, clock }[]` then convert. Do not leak keck `save_update` into `SyncProvider`.
