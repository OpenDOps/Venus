# Venus hub

**M3.0 board:** steps 1–9 `done` (closed 2026-09-13).

Venus-owned **collab front**. Compose service **`hub`**. Source: [`crates/venus-hub`](../../../../../crates/venus-hub). Image: [`deploy/hub/Dockerfile`](../../../../../deploy/hub/Dockerfile).

| Doc | Role |
|---|---|
| This file | Process: run, env, persist, HTTP |
| [architecture.md](./architecture.md) | Software architecture of the process (apply / fan-out / persist, **memory caps**) |
| [files.md](./files.md) | Crate tree and what each `.rs` does |

Live CRDT HA: [M3.0/high-availability.md](../../../M3.0/high-availability.md). Dirty logic/perf leftovers: [logicals-and-performance.md](../../../M3.0/logicals-and-performance.md). Plan: [M3.0/plan.md](../../../M3.0/plan.md). Dataflow of Venus: [architecture.md](../../../architecture.md). Actuals: [api-map.md](../../../api-map.md).

This folder is the **process** (run, Rust, SQL, HTTP, gRPC). The **wire** (Yjs on `spaceDoc`, seam, tab share, export as a protocol) stays in [CRDT/README.md](../../../CRDT/README.md). JSON/gRPC envelope: [rpc.md](../../../rpc.md). No Rust in the editor: [CRDT/wasm.md](../../../CRDT/wasm.md).

It is **not** BlockSuite, not JWST Block REST, not `fromDoc` / `toDoc`, not git, not `jobs`. The browser editor stays JS (`yjs@13.6.32` on `store.spaceDoc`). Rust is this process only.

```text
Tab  -- y-protocols/sync + AFFiNE WS -->  hub (one owner per workspace_id)
                                            │ apply (y-octo, one queue per room)
                                            │ broadcast (other sockets)
                                            │ persist ~1s
                                            ▼
                                         Postgres  crdt_* + blob + workspace_lease + dirty
```

## How to run

Product path is Docker. Host `cargo run -p venus-hub` is recon only (still needs Postgres; still no SQLite).

```bash
# from repo root
docker compose up --build postgres hub
# same as: pnpm sync:up

# full stack (nginx + host UI)
docker compose up --build
# same as: pnpm compose:up
```

Wait until hub listens on `:3000`. Health (same probe as M1):

```bash
curl -sSSf -X POST http://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00
# {"protocol":"AFFiNE"}
```

`GET http://127.0.0.1:3000/` returns `venus-hub`. Web UI (after `web` is up): **http://127.0.0.1:8080**.

Host Vite / Playwright still open WS to `:3000`:

```bash
VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00 pnpm dev
pnpm test:e2e:m1   # needs postgres + hub already up
```

Stop without wiping the page: `docker compose down` (keeps volume `pg-venus-data`). `down -v` deletes it.

### Postgres environment

The hub **must** reach Postgres. There is **no** SQLite fallback.

Set **either** a full DSN **or** the discrete parts (Compose sets both; `DATABASE_URL` wins):

| Variable | Required | Compose default |
|---|---|---|
| `POSTGRES_HOST` | yes, unless `DATABASE_URL` | `postgres` |
| `POSTGRES_USER` | yes, unless `DATABASE_URL` | `venus_hub` (NOSUPERUSER). Cluster superuser on service `postgres` is still `venus`. |
| `POSTGRES_PASSWORD` | yes, unless `DATABASE_URL` | `venus` |
| `POSTGRES_PORT` | no | `5432` |
| `POSTGRES_DB` | no | `venus` |
| `POSTGRES_SSLMODE` | no | `disable` (Compose). libpq tokens only (`disable`, `allow`, `prefer`, `require`, `verify-ca`, `verify-full`). Used when building from parts; ignored when `DATABASE_URL` is set. |
| `DATABASE_URL` | no | Compose sets `postgres://venus_hub:venus@postgres:5432/venus?sslmode=disable`. If set and non-empty, **wins** over the parts (not rewritten) |

`DATABASE_URL` example: `postgres://venus_hub:venus@postgres:5432/venus?sslmode=disable`. Special characters in user/password are percent-encoded when the URL is built from parts. Startup logs `pg_sslmode`, never the DSN. The hub image does **not** bake `POSTGRES_PASSWORD` / `DATABASE_URL`.

Also:

| Variable | Role |
|---|---|
| `HUB_LISTEN` | Bind address. Default `0.0.0.0:3000`. |
| `HUB_OWNER` | Lease id. Default `{HOSTNAME}-{pid}`. Compose `hub` sets `hub`; `hub-b` uses `hub-b`. |
| `HUB_LEASE_TTL_SECS` | `workspace_lease` TTL. Default `20`. Must be `>= 3 ×` heartbeat interval. |
| `HUB_HEARTBEAT_INTERVAL_SECS` | Lease refresh period. Default `lease_ttl / 3` (6s at TTL 20). |
| `HUB_DB_MAX_CONNECTIONS` | **Required.** sqlx pool max. Compose `32` (10 distinct cold exports + 10 blob GETs + 10 room flushes overlapping, plus heartbeat). `>= 1`. |
| `HUB_DB_MIN_CONNECTIONS` | **Required.** Warm pool size. Compose `4`. `>= 0` and `<= max`. |
| `HUB_DB_ACQUIRE_TIMEOUT_SECS` | **Required.** Wait for a free connection then fail the call. Compose `10`. `>= 1`. |
| `HUB_DB_WORK_MEM` | **Required.** Session `work_mem` for hub connections (P5): an 8 MiB flush spills to a temp file at the 4 MB server default. Compose `16MB`. Explicit unit (`kB`/`MB`/`GB`) — a bare integer would mean kB. Per session per sort/hash, so size it against `HUB_DB_MAX_CONNECTIONS`. |
| `HUB_PERSIST_INTERVAL_MS` | **Required.** Persist tick. Compose `1000`. `>= 1`. Zero would panic the interval. |
| `HUB_COMPACT_AFTER` | **Required.** Compact when trail length reaches this. Compose `32`. `>= 1`. |
| `HUB_CORS_ORIGINS` | CORS allow list. Unset → six localhost Vite/Compose origins. Empty → no CORS (same-origin nginx). Comma-separated `http(s)://host[:port]`. `*` is rejected. Methods `GET,HEAD,POST,DELETE`; headers `Content-Type`, `If-None-Match`. Does not echo the request origin. |
| `HUB_GRPC_LISTEN` | Internal gRPC (`Hub.ExportDoc` / `ListDocs`). Default `0.0.0.0:3100`. **Not served yet** (IDL in `proto/`; tonic in M4 `step-spaces`). GET `/export` is advertisement pointing here. [rpc.md](../../../rpc.md) |

Missing or blank pool / persist / compact vars is a startup error that names the variable. Missing host/user/password (and no `DATABASE_URL`) is a startup error. A `sqlite:` URL is a startup error. Do not log the DSN.

`cargo test -p venus-hub --test recon` does not need Docker. Store tests (`--test store`) start Postgres 16 via testcontainers, or use `DATABASE_URL` / `POSTGRES_*` if those are already set.

### Second owner (lease)

One live RAM doc per `workspace_id`. A second hub against the **same** Postgres must not apply:

```bash
pnpm compose:ha
# or: docker compose --profile ha up --build hub-b
# hub-b publishes 127.0.0.1:3001
# WS to 127.0.0.1:3001/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00 → 503 while hub holds the lease
```

SIGTERM on `hub`: flush the persist buffer, `DELETE` our `workspace_lease` rows, then exit. Crash without SIGTERM can lose ≤ one persist batch (~1s), same as keck.

## How it persists

Database name **`venus`**, volume **`pg-venus-data`**. Do not reuse M1’s `pg-data` / `jwst` schema — those rows are keck, not hub. Re-seed the demo wiki after cutover (`docker compose down -v` only if you intend to wipe).

Schema is applied at process start (`crates/venus-hub/src/schema.sql`) under a session advisory lock. Trigger `DROP`/`CREATE` runs only when `crdt_update_dirty` is missing or still `FOR EACH ROW` (`tgnewtable` unset), or when an old volume still has a `crdt_snapshot` dirty trigger, so a second hub boot does not take AccessExclusive on live tables.

```text
crdt_snapshot (workspace_id, doc_id) → { bin, clock, updated_at }
crdt_update   (workspace_id, doc_id, seq) → { bin, created_at }
blob          (workspace_id, hash) → { bytes }
workspace_lease (workspace_id) → { owner, lease_until }
dirty         (workspace_id, doc_id) → { clock, first_dirty_at }
```

- **Hydrate:** snapshot bytes, then each `crdt_update` in `seq` order, `apply_update_from_binary_v1`.
- **Flush:** append update binaries. `dirty.clock` is `max(seq)` for that statement.
- **Compact:** background. New updates with `seq` greater than the compacted max are left in the trail. Compact does **not** mark dirty — it merges rows the flush already marked, at that same clock.
- **Dirty:** one statement-level `AFTER INSERT` on `crdt_update` (`crdt_update_dirty`) upserts `dirty` with `max(seq)`, monotonic via `GREATEST`, and `INSERT … ON CONFLICT DO NOTHING` into `dirty_wiki`. Grain is `(workspace_id, doc_id)` for `dirty`; wiki grain for `dirty_wiki`. The function catches `undefined_table` (logs a `WARNING`) so a missing `dirty` / `dirty_wiki` table cannot roll back persist. Hub migrate also creates empty `jobs` and `last_flushed`. The hub process does **not** `INSERT` into `jobs` (M3 observer).
- **Blobs:** Postgres `BYTEA`, keyed by workspace + hash. Not S3 in M3.0.

After a write, wait **≥2s** before `docker compose restart hub` if you are testing persist.

## HTTP / WS (M1-compatible collab) + export advertisement

| Method | Path | Body / notes |
|---|---|---|
| `POST` | `/collaboration/:workspace_id` | `{"protocol":"AFFiNE"}` (health; no lease) |
| `GET` | `/collaboration/:workspace_id` | No upgrade → `{"protocol":"AFFiNE"}` (health; **no** lease). `Upgrade: websocket` → protocol `AFFiNE`. Lease held by another hub → **503** `{ "error": { "code": "lease_held", … } }`. Store/hydrate failure → **500** `{ "error": { "code": "store_failed", … } }` |
| `GET` | `/api/block/:workspace_id/export` | Advertisement JSON. Bare GET **200** `{ "advertisement": { … } }` (no `error`). `?doc=` **400** `export_http_disabled`. Yjs is gRPC `Hub.ExportDoc` (`HUB_GRPC_LISTEN`, default `:3100`). [rpc.md](../../../rpc.md) |
| `POST` | `/api/blobs/:workspace_id` | `application/octet-stream` → `{ id, exists }` |
| `GET`/`HEAD`/`DELETE` | `/api/blobs/:workspace_id/:hash` | bytes / `Content-Length` from `octet_length` (no body) / 404 / 204 |

No `/api/block/:id/:block` CRUD. CORS: `GET`/`HEAD`/`POST`/`DELETE` and `Content-Type` / `If-None-Match`, origins from `HUB_CORS_ORIGINS` (default Vite `:5173`/`:5174` and Compose `:8080`). Empty list → no CORS layer.

## Client

The host still uses `OctoBaseKeckProvider` (`kind: 'octobase'`) as a **wire alias**. Bytes and URLs did not change; `mount-editor.js` still does not import the server. `VITE_SYNC_URL` unset → memory. Product kind name `venus` may replace the alias later; e2e asserts `octobase` until then.

## License

New hub files: **MIT OR Apache-2.0**. y-octo is MIT. This image is **not** AGPL keck. keck Dockerfile may remain under `deploy/octobase/` as history; product Compose does not build it. [licensing.md](../../../../legal/licensing.md).
