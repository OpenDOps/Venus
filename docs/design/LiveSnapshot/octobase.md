# OctoBase and pinning (M1 recon)

What pinned keck (`276e0e94719a652483119c5fea16be13293ee21c`) actually does. **M1 only.** Product collab after [M3.0](../M3.0/README.md) is the **Venus hub**, not this process. Live CRDT HA: [M3.0/high-availability.md](../M3.0/high-availability.md). Snapshotter HA: [high-availability.md](./high-availability.md). Pins and endpoints (M1 Actuals): [api-map.md](../api-map.md).

Do **not** copy keck fleet / `jwst` dirty into M3. Venus does **not** put convert or git inside keck (or the hub), and does **not** pause persist for a pin. Dirty on the product path is a **Postgres upsert** on hub persist ([LiveSnapshot HA](./high-availability.md#how-dirty-is-marked), [M3.0 dirty](../M3.0/high-availability.md#dirty-mark-postgres-not-a-hub-hook)).

## Y.Text `Format` (live A→B)

Stock `jwst-codec` at this pin panics `unimplemented!()` in `Value::from` for `Content::Format`. `DocPublisher` walks every item for history (`content: Value::from(&item.content).to_string()`). That kills the observer thread; `join().unwrap()` on stop is a second panic. **Postgres still stores the Yjs bytes** (export / reload work). Live fan-out does not.

The stored `venus-m0` doc is valid Yjs. Decode of `GET /api/block/venus-m0/export` (2026-08-30) had **10** `ContentFormat` items, all parent `YText`, not deleted:

| key | open value | close |
|---|---|---|
| `link` | `https://example.com/path` | `null` |
| `bold` | `true` | `null` |
| `italic` | `true` | `null` |
| `code` | `true` | `null` |
| `color` | `var(--affine-palette-line-red)` | `null` |

Those are `?md-demo=1` paragraphs (`docs` link, `bold italic code`, `Colored text`). Any BlockSuite mark uses the same `Content::Format` (also underline, strike, background if the user applies them). Encode/decode of Format in the update already worked; only the `Value` mapping was missing.

Venus overlays `deploy/octobase/patches/value.rs` and `publisher.rs` at image build: `Value::Format { key, value }` round-trips to `Content::Format`, `Display` is `format(key=value)`, observer `join` logs instead of unwrap. Same `OCTOBASE_SHA`. Not a skip/`Undefined` stub.

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
| Notify Venus of dirty `(workspace_id, docId)` | **Stock:** no. **Venus:** Postgres trigger on persist (preferred). keck `storeHook` only if SQL cannot map grain |

So: “hold the persist buffer during pin, still sync clients, flush when pin ends” is **not** a keck feature. Live sync already ignores persist. The missing piece is only a **consistent copy of now** for Venus, which the snapshotter owns. At thousands of wikis that copy is a **dirty-set cut**, not one replica per page: [high-availability.md](./high-availability.md).

## Dirty mark (Postgres, not a keck hook)

keck already writes Yjs to **Postgres `jwst`**. Venus `dirty` is the same instance. **Preferred:** an `AFTER INSERT/UPDATE` trigger on the persist tables upserts `dirty(workspace_id, docId, clock)`. Stock keck persist stays; Format overlay stays. No `storeHook` in Rust if SQL can map workspace/guid → page.

Contract: [high-availability.md — how dirty is marked](./high-availability.md#how-dirty-is-marked).

| Rule | |
|---|---|
| **When** | Persist row lands in `jwst`, not apply/broadcast |
| **What** | UPSERT `{ workspace_id, docId, clock }` — one row, coalesce |
| **Who writes `jobs`** | Venus observer |
| **Trigger** | Tiny. No `jobs` locks. No HTTP. Must not fail persist if `dirty` is missing (or install only after `dirty` exists) |
| **`storeHook`** | Fallback only if `jwst` rows cannot express `docId` / clock |
| **Do not** | Pause `save_update`; skip `update_doc` during convert; convert in keck; Venus process tailing WAL; Kafka in front of dirty |

M1/M2 used replica / idle GET. Product dirty is the M3.0 SQL trigger on hub persist.

## How Venus should pin against this keck

1. **Sidecar replica (preferred for “now”).** Connect like any client. Pin = clone encode of that `Y.Doc`. Apply/broadcast/persist on keck stay untouched. Matches the invariant: pin is Venus memory.
2. **GET export (OK for M3 idle).** After 30–120s idle the ~1s persist batch is done. Pin = those SQL bytes. Do not use this as `T0` without waiting or using (1); otherwise lease Before can miss the last second of typing.
3. **Do not** PATCH keck to skip `update_doc` during convert. Crash during a long `fromDoc` would drop durability for no gain: the pin already has its bytes.

M1 already documents: wait ≥2s after a write before restarting keck if you need persist. Same number is a decent lower bound before GET-export as a pin if the replica is not used.

## Cloud and devices (M1)

M1 hosted: keck + Postgres (`jwst`). **After M3.0:** Venus hub + Postgres (`crdt_*`); on device later is one hub + local SQLite, not OctoBase. Not Hocuspocus. Pin interface stays `{ docId, bytes, clock }[]` then convert. Do not leak persist buffers into `SyncProvider`. Dirty is Postgres on persist. Observer still produces `jobs`.

## Keck fleet (M1 recon — superseded)

**Do not implement this as the product fleet.** Same shape lives on the **hub**: [M3.0/high-availability.md](../M3.0/high-availability.md). Keep the notes below only to explain why two kecks on one `workspace_id` were split-brain.

Stock keck is **one process**: live `Workspace` and `save_update` HashMap are **RAM**. Broadcast is in-process. `GET …/export` reads **Postgres**, not that RAM. Two kecks applying the **same** `workspace_id` at once is split-brain (two live docs, persist ~1s apart, clients do not see each other).

**Best hosted design:** many keck pods, **at most one live owner per `workspace_id`**, shared Postgres (`jwst` + Venus tables).

Cookie / IP sticky is **wrong**: two browsers on the same wiki can land on different pods. Route on **`workspace_id`** (WS path `/collaboration/:workspace_id`, blob/export prefix).

```text
clients ──► gateway (hash or lease: workspace_id → keck pod)
                 │
                 ▼
            keck owner (RAM apply + broadcast + persist ~1s)
                 │
                 ▼
            Postgres jwst   (shared)     SQL trigger → Venus dirty
```

| Piece | Rule |
|---|---|
| **Shard key** | `workspace_id` (wiki). Consistent hash **or** a short TTL **lease** (etcd/Postgres) so failover is explicit. |
| **Live owner** | Exactly one keck has the in-memory Workspace for that id. All WS for that wiki go there. |
| **Postgres** | Shared. Refresh and failover **hydrate from SQL** (trail the persist batch, ~1s). |
| **Blobs / export** | May be served by any pod (SQL). Simplest: send them to the same owner as WS. |
| **Dirty** | Postgres trigger on persist (shared `jwst` + `dirty`). One owner → one write → one upsert. |
| **Drain** | On SIGTERM, flush `save_update` then drop the lease so the next owner loses less than a full batch. |
| **Do not** | Two owners; Redis as a second CRDT; Kafka in front of Yjs; replicate keck RAM. |

**Failover:** owner dies → clients reconnect → gateway picks a new owner → load `jwst`. Unflushed RAM updates can be lost (same as killing one keck today). That is snapshot **and** collab lag of the persist window, not a second architecture.

**Not the convert fleet.** Snapshotter **workers** stay `SKIP LOCKED` on Venus `jobs` — no keck sticky. **Observers** are ≥2 replicas that batch `dirty` and `INSERT jobs ON CONFLICT DO NOTHING` — not `hash(workspace_id) % replicaCount`. Git working trees may still stick `workspace_id → worker` if there is no remote. Detail: [HA — snapshotter fleet](./high-availability.md#snapshotter-fleet-competing-consumers-not-hash-shards).

**On device:** one OctoBase process. HA is sync/backup to hosted keck, not a local keck ring.
