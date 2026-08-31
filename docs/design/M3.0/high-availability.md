# Venus hub — live CRDT high availability

**Status:** design for [M3.0](./README.md). Implement the **thin** column in that plan (one wiki, one hub replica, lease table, dirty trigger). This file is the scale contract so M3.0 does not paint “one process forever” into a corner.

Snapshotter fleet (jobs, `SKIP LOCKED`, pin cut): [LiveSnapshot/high-availability.md](../LiveSnapshot/high-availability.md). That file does **not** own the collab process after M3.0.

M1 keck is a **legacy** merge buffer ([octobase.md](../LiveSnapshot/octobase.md)). Product hosted collab after M3.0 is **this hub**.

## What the hub is

A Venus-owned process that **only**:

1. Applies Yjs update v1 into a RAM `Y.Doc` (one apply queue per room).
2. Broadcasts to other sockets on that room.
3. Persists off the hot path (~1s) into Postgres Venus owns.
4. Serves blob HTTP and doc export (bytes, not markdown).

It is **not** BlockSuite, not SSR, not JWST Block REST, not `fromDoc`, not git, not `jobs`.

```text
browsers  ──y-protocols / AFFiNE WS──►  hub owner (RAM apply + broadcast)
                                              │ persist ~1s
                                              ▼
                                         Postgres crdt_* + blobs
                                              │ AFTER persist UPSERT dirty
                                              ▼
                                         Venus observer → jobs   (M3, not this process)
```

## Three pipes (keep them separate)

Same split keck already had; do not glue them back:

```text
WS binary
  → apply (batch ~100ms ok) → RAM Y.Doc
  → replies on that socket + broadcast → other clients

broadcast / apply result
  → one persist buffer per room (not per socket)
  → every ~1s drain → INSERT crdt_update (compact snapshot in background)

GET …/export
  → encode from SQL (snapshot + pending updates) **or** RAM encode of the live doc
  → M3 idle pin may use SQL after ≥ persist batch; T0 prefers RAM encode or a replica
```

Live collaboration **never** waits on markdown, git, or pin convert. Persist is **never** paused for a pin.

## Rooms

| Key | Meaning | M3.0 |
|---|---|---|
| `workspace_id` | Wiki. Gateway shard / **lease**. One live owner. | `venus-m0` |
| `docId` | Page. One RAM `Y.Doc`. | `doc:home` on that room (same collapse as M1) |

M3.0 keeps **one Y.Doc per keck-shaped room** (`/collaboration/:workspace_id` = that page’s `spaceDoc`) so the existing client stays drop-in. Many pages per wiki is M4+ (then one owner still holds all docs for that `workspace_id`).

## Persist schema (Venus tables)

Do not write `jwst` docs rows. New tables in the same Postgres instance (database name may stay `jwst` or become `venus` — api-map Actual):

```text
crdt_snapshot (workspace_id, doc_id)  → { bin, clock, updated_at }
crdt_update   (workspace_id, doc_id, seq) → { bin, created_at }
blob          (workspace_id, hash) → { bytes }     # or S3 later; M3.0 = DB
workspace_lease (workspace_id) → { owner, lease_until }
```

- **Hydrate:** snapshot + apply pending updates → RAM.
- **Flush:** append `crdt_update`; `clock` monotonic (`seq` or snapshot clock + count).
- **Compact:** background, **not** on the insert path under a global lock. Merge into snapshot; delete merged rows.
- **Crash:** lose unflushed RAM (~1s). Same as keck. **SIGTERM:** flush then drop lease.

## Dirty mark (Postgres, not a hub hook)

```text
hub persist  →  INSERT crdt_update
             →  AFTER INSERT/UPDATE UPSERT dirty(workspace_id, doc_id, clock)
```

| Rule | |
|---|---|
| **When** | Persist row lands, not apply/broadcast |
| **What** | UPSERT `{ workspace_id, docId, clock }` — one row, coalesce |
| **Who writes `jobs`** | M3 observer, **not** the hub |
| **Trigger** | Tiny. Must not fail persist if `dirty` is missing (install after `dirty` exists, or `EXCEPTION` log) |
| **Do not** | Pause persist; skip `INSERT` during convert; convert in the hub; Kafka in front of Yjs |

M3.0 ships the trigger and `dirty` table. `jobs` / observer are M3.

## Fleet (one live owner)

Hub RAM and in-process broadcast are **not** shared across pods. Two hubs applying the same `workspace_id` is split-brain.

**Design:** many hub pods, **at most one live owner per `workspace_id`**, shared Postgres.

Cookie / IP sticky is **wrong**. Route on **`workspace_id`** (WS path `/collaboration/:workspace_id`, blob/export prefix).

```text
clients ──► gateway (hash or lease: workspace_id → hub pod)
                 │
                 ▼
            hub owner (RAM apply + broadcast + persist ~1s)
                 │
                 ▼
            Postgres crdt_* + blobs     SQL trigger → dirty
```

| Piece | Rule |
|---|---|
| **Shard key** | `workspace_id`. Consistent hash **or** TTL **lease** (`workspace_lease`) so failover is explicit. |
| **Live owner** | Exactly one hub has the in-memory doc(s) for that id. All WS for that wiki go there. |
| **Postgres** | Shared. Refresh and failover **hydrate from SQL** (trail the persist batch, ~1s). |
| **Blobs / export** | May be served by any pod (SQL). Simplest: send them to the same owner as WS. |
| **Dirty** | Trigger on persist. One owner → one write → one upsert. |
| **Drain** | On SIGTERM, flush persist then **drop the lease** so the next owner loses less than a full batch. |
| **Do not** | Two owners; Redis as a second CRDT; Kafka in front of Yjs; replicate hub RAM; nbstore / Socket.IO. |

**Failover:** owner dies → clients reconnect → gateway picks a new owner → load SQL. Unflushed RAM updates can be lost (same as killing keck). That is snapshot **and** collab lag of the persist window, not a second architecture.

**M3.0 thin:** one hub replica is enough if `workspace_lease` exists and a **second** hub process **refuses** (or waits) when the lease is held. Prove that before many pods. k8s gateway hashing is later devops; do not block M3.0 exit on a mesh.

**Not the convert fleet.** Snapshotter workers stay `SKIP LOCKED` on Venus `jobs` — no hub sticky. Observers are ≥2 replicas that batch `dirty` and `INSERT jobs ON CONFLICT DO NOTHING`. [LiveSnapshot HA — snapshotter fleet](../LiveSnapshot/high-availability.md#snapshotter-fleet-competing-consumers-not-hash-shards).

**On device (later):** one hub process + local SQLite with the same schema; HA is sync/backup to hosted hub, not a local ring. M3.0 is hosted Postgres only.

## Pin against this hub

Same rules as [LiveSnapshot](../LiveSnapshot/README.md), with keck renamed:

1. **Sidecar replica** (preferred for “now” / `T0`): connect like a client; pin = encode of that `Y.Doc`. Hub apply/broadcast/persist stay untouched.
2. **GET export** (OK for M3 idle): after persist has flushed (~1s, wait ≥2s). If export is SQL-only, it can trail live RAM. Prefer RAM encode for `T0`.
3. **Do not** skip persist during convert. Crash during `fromDoc` would drop durability for no gain.

## What not to take from OctoBase / AFFiNE

| Drop | Keep (as ideas) |
|---|---|
| JWST Block REST / `space:meta` as API | Opaque Yjs binaries |
| Per-socket persist tasks | One persist buffer **per room** |
| History stringify / `Content::Format` panic | Do not walk items to persist |
| Compact on the write path at 500 rows | Background compact |
| nbstore, Socket.IO, `@affine/core` | Clocks on persist (`dirty.clock`) |
| Hocuspocus / stock `y-websocket` server | `y-protocols` binary + `AFFiNE` so M1 client works |

## Acceptance (this file)

Re-accept before treating M3.0 HA as done (the plan’s `step-ha-owner` + `step-dirty` encode the thin instance).

| # | Locked |
|---|---|
| H1 | Hub is apply + broadcast + persist. No convert, git, or `jobs` inside the hub. |
| H2 | One live owner per `workspace_id`. Lease or hash. Not cookie/IP sticky. |
| H3 | Persist ~1s, never paused for pin. Crash loss ≤ persist window; SIGTERM flushes then drops lease. |
| H4 | Dirty = SQL upsert on `crdt_update` (or snapshot replace). Hub does not write `jobs`. |
| H5 | Export / blobs do not require Block REST. Wire stays Yjs v1 + `AFFiNE`. |
| H6 | Snapshotters are a different fleet ([LiveSnapshot HA](../LiveSnapshot/high-availability.md)). |

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | M3.0 slice |
| [plan.md](./plan.md) | Steps |
| [high-availability.md](./high-availability.md) | This live CRDT HA |
| [LiveSnapshot HA](../LiveSnapshot/high-availability.md) | Pin/git/jobs scale |
