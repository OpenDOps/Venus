# Hub crate files

Source of Compose service **`hub`**. Architecture: [architecture.md](./architecture.md). Run / HTTP / SQL: [README.md](./README.md).

## Tree

```text
Venus/
  Cargo.toml                 workspace; members = ["crates/venus-hub"]
  Cargo.lock
  rust-toolchain.toml        channel 1.90.0; rust-analyzer component
  deploy/hub/Dockerfile      build rust:1.90-bookworm; runtime debian:bookworm-slim USER venus
  crates/venus-hub/
    Cargo.toml               lib + bin `venus-hub`; y-octo, axum, sqlx, tokio
                             test: tokio-tungstenite, testcontainers, futures-util
    src/
      lib.rs                 crate root: modules + DEFAULT_WORKSPACE_ID / PAGE_DOC_ID / SUBPROTOCOL
      main.rs                process entry
      config.rs             env → Config
      protocol.rs           y-protocols + y-octo apply/encode
      room.rs                Room + Hub
      db.rs                  Postgres CRDT / blob API
      schema.sql             included by db::migrate
      lease.rs               workspace_lease
      http.rs                Axum routes + WS loop
      blobs.rs               BlockSuite sha() + content-type sniff
    tests/
      recon.rs               M3.0 step-recon-hub (no Postgres)
      store.rs               M3.0 step-store (Postgres 16)
      ws.rs                  M3.0 step-ws (AFFiNE WS + persist)
```

The workspace has **one** member. Do not add OctoBase / `jwst-*` crates here. Image build copies only `Cargo.toml`, `Cargo.lock`, and `crates/venus-hub` — not `deploy/octobase`.

## Rust modules

### `lib.rs`

Crate root. Declares `blobs`, `config`, `db`, `http`, `lease`, `protocol`, `room`.

| Symbol | Value | Meaning |
|---|---|---|
| `PAGE_DOC_ID` | `"395cd07b-bdb1-5f54-ada8-e9a3fabb6a20"` | SQL `doc_id` (UUID v5 of `doc:home`). The page `spaceDoc` on `/collaboration/:workspace_id`. BlockSuite `createDoc` stays `doc:home`. |
| `DEFAULT_WORKSPACE_ID` | `"77e4a2b1-8b40-5979-a73c-fd4477216d00"` | UUID v5 of `venus-m0`. M0 wiki in the URL and lease. |
| `SUBPROTOCOL` | `"AFFiNE"` | `Sec-WebSocket-Protocol` |

No I/O.

### `main.rs`

Binary. Tracing, `Config::from_env` (pool / persist / compact env required), `connect_with` + migrate, construct `Lease` and `Hub`, bind TCP, `Hub::heartbeat` task (`lease_ttl / 3`, `MissedTickBehavior::Delay`; sheds stolen rooms, evicts idle), `axum::serve` with graceful shutdown, then `Hub::shutdown_with_heartbeat` even if serve returned `Err`. Startup logs pool sizes, persist/compact knobs, and CORS origin count, never the DSN.

Does **not** decode Yjs. Does **not** open rooms until HTTP asks.

### `config.rs`

`Config`: `database_url`, `pg_sslmode` (logged at start; never log the DSN), `listen`, `owner`, `persist_interval` (`HUB_PERSIST_INTERVAL_MS`, required, `>= 1`), `lease_ttl` (20s, `HUB_LEASE_TTL_SECS`), `heartbeat_interval` (`ttl / 3` unless `HUB_HEARTBEAT_INTERVAL_SECS`; startup requires `ttl >= 3 × interval`), `compact_after` (`HUB_COMPACT_AFTER`, required, `>= 1`), pool (`HUB_DB_MAX_CONNECTIONS` `>= 1`, `HUB_DB_MIN_CONNECTIONS` `>= 0` and `<= max`, `HUB_DB_ACQUIRE_TIMEOUT_SECS` `>= 1`, `HUB_DB_WORK_MEM` a size with an explicit unit, normalized by `parse_work_mem` — P5), `cors_origins` (`HUB_CORS_ORIGINS`; unset → six localhost origins; empty → none; `*` rejected). Missing/blank pool or timer vars fail start naming the var.

`database_url_from_env`: non-empty `DATABASE_URL` wins and is not rewritten (`pg_sslmode` is the query value, or `unset` if missing). Else `POSTGRES_HOST` + `USER` + `PASSWORD` (required) plus optional port / db and `POSTGRES_SSLMODE` (libpq tokens, default `disable`). Rejects `sqlite` and unknown `sslmode` (so a typo cannot inject query params). Percent-encodes user/password when building the URL.

### `protocol.rs`

y-protocols/sync via y-octo (`read_sync_message` / `write_sync_message`). Tags 0–3: Doc, Awareness, Auth, AwarenessQuery.

| Function | Role |
|---|---|
| `encode_doc_step1` / `step2` / `update` | Wrap a payload as that `DocMessage` |
| `encode_auth_ok` | Auth with no reason string |
| `encode_awareness_empty` | Empty awareness (hello frame) |
| `empty_state_vector` | One-byte SV (zero clients) |
| `decode_sync_messages` | Drain a binary frame into `DecodedSync` (`messages`, `offset`, `remaining`). Truncation is visible to the caller; the socket is not closed. |
| `apply_v1` | `Doc::apply_update_from_binary_v1` |
| `encode_v1` | `Doc::encode_update_v1` |
| `encode_state_vector` / `encode_step2_for` | Handshake Step1/Step2; diff encode, else full encode |
| `is_noop_update` | Skip empty / all-zero bins |

Does **not** talk to Postgres or sockets.

### `room.rs`

Two types:

- **`Room`** — one apply queue (`RwLock<Doc>`: hello / export / Step1 share a read; only `apply_v1` writes), one persist `PersistBuf` (`Vec<Bytes>`, cap `PERSIST_BYTES` = 8 MiB; overflow **drops oldest**, `error!`, keeps newest; L8 in-flight prefix is not dropped), one persist `JoinHandle` on the room (not a Hub Vec), one client map (`OUTBOUND_CAP` = 256 slots **and** `OUTBOUND_BYTES` = 1 MiB queued; `Outbound` is `mpsc::Sender<Bytes>` plus a queued-byte counter; `try_send` Full/closed/over-budget detaches; `OutboundRx::recv` releases the reservation), `trail_len`, `last_empty` (P6). `attach` / `detach` / `handle_binary` / `flush` / `encode_live` / `compact_if_needed`. **`attach` hello:** encode Auth, empty awareness, Step1, Step2 under `doc` read, then `clients.insert` (encode fail never ghosts a sender). After `request_stop`, `handle_binary` / apply do not persist.
- **`Hub`** — `PgPool`, `Lease`, `rooms` map, `hydrating` single-flight (key `workspace_id`; waiters get `Arc<Room>`), `exporting` single-flight (same key; waiters get export bytes), `stopping` (`AtomicBool`). `get_room` (join in-flight or lease + hydrate + spawn persist onto the `Room`) → `Result<Arc<Room>, GetRoomError>` (`Held` vs `Store`). Stopping is `Store("hub is shutting down")` before `try_acquire` (L19); a hydrate that fails after `try_acquire` releases the lease (L16) so this process does not own a workspace it is not serving. `live_export`: hot path clones `Arc<Room>` and drops the map lock before `encode_live` (no flight). Cold path joins or leads `exporting`, re-checks RAM, then one `db::default_page_export` for all waiters. `heartbeat` refreshes owned leases, sheds misses (L4, no `try_acquire`), and evicts idle rooms (`drop_one`, P6). `shutdown` sets `stopping` first, notifies persist, takes each room’s handle, joins (timeout), leftover flush, `rooms.clear()`, `drop_all`. Idempotent. `shutdown_with_heartbeat` aborts the process heartbeat first. `abort_persist_no_flush` is `#[doc(hidden)]` (crash test, not a shutdown path).

`apply_and_fanout` is the only place apply, persist-push, and broadcast meet: frame the Update first (L21), then `apply_v1`, then persist-push (capped), then broadcast. Persist drain is `Room::flush` → `db::flush_updates` (the whole batch in one `INSERT`; clone `Vec<Bytes>`, then drain the prefix after commit so abort is not lossy). Fan-out and the persist prefix clone are `Bytes` refcounts; inbound still clones once to build the frame. Compact is `Room::compact_if_needed` on the same timer: skip SQL when `trail_len < compact_after`; else SQL rebuild (read under the L1 lock, merge in RAM, write if `MAX(seq)` is unchanged). Not on insert.

`Room::new` is usable **without** Postgres (recon tests). `Hub::get_room` is the product path. `Room::with_budgets` is `#[doc(hidden)]` for S6/S10 tests. Memory caps (persist 8 MiB / outbound 1 MiB / WS 512 KiB, vs working set): [architecture.md](./architecture.md#memory-caps-not-working-set).

### `db.rs`

Postgres store. Opaque Yjs bytes. Public surface used by rooms and HTTP:

| Function | Role |
|---|---|
| `connect` / `connect_with` / `migrate` | Pool + `schema.sql`. Product path is `connect_with` (`PgPoolOptions` from `HUB_DB_*`). Tests use `connect` (sqlx-shaped 10 / 0 / 30s). `migrate` takes a session `pg_advisory_lock` on one pooled connection, runs tables+function, then trigger DDL **only if** `crdt_update_dirty` is missing or still `FOR EACH ROW` (`tgnewtable` unset — P13 cutover) **or** a `crdt_snapshot` dirty trigger is still there (P1 removal), then unlocks. Not the `venus_doc_lock_key` hydrate/compact lock (L20). |
| `hydrate_doc` | One RR tx: lock, snapshot `FOR SHARE`, trail, `COMMIT`, then apply → RAM `Doc`. Retry once on `40001`/`40P01`. Bad trail bins are skipped and logged (L13); a snapshot that will not apply is skipped too and hydrate falls back to the trail alone (L17, logged at `error!`). |
| `hydrate_with_trail_len` | Same hydrate, plus trail row count (including skipped bins) for `Room::trail_len` |
| `get_doc` / `encode_doc` | Same hydrate, then `encode_v1` (no snapshot write) |
| `push_update` / `flush_updates` | `INSERT crdt_update`. Flush is one statement (`unnest($3::bytea[]) WITH ORDINALITY`), so `seq` follows array order; the STATEMENT dirty trigger upserts `max(seq)` once (P13). `Room::flush` clones `Vec<Bytes>` (P15), binds `&[&[u8]]`, then drains the prefix after commit |
| `compact` | Exclusive lock, read snapshot + trail, **COMMIT**, merge in RAM (skip bad trail bins; snapshot apply fail-closed, L17). Re-lock; if `MAX(seq)` is unchanged, UPSERT snapshot, `DELETE seq <= max_seq`, return remaining trail length. If seq moved, skip the write (persist retries). Hidden `compact_after_load` (P11) / `compact_after_now_max` (D1) inject between those phases in tests. |
| `put_blob` / `get_blob` / `blob_len` / `delete_blob` | `blob` table. HEAD and GET **304** use `blob_len` (`octet_length`), not `get_blob`. |
| `default_page_export` | `get_doc` for `PAGE_DOC_ID` |

Does **not** stringify history. Does **not** write `jobs`.

### `schema.sql`

DDL applied at startup (`CREATE TABLE IF NOT EXISTS`). Tables: `crdt_snapshot`, `crdt_update`, `blob`, `workspace_lease`, `dirty`. `workspace_id` and `doc_id` are **UUID**. Dirty grain is `(workspace_id, doc_id)`; lease grain stays `workspace_id`. `venus_doc_lock_key(ws, doc)` XOR-folds both UUIDs to one `bigint` for hydrate/compact advisory locks. Existing TEXT columns from earlier boots are `ALTER`ed (`venus-m0` / `doc:home` map to the v5 constants; other slugs fail migrate — wipe `pg-venus-data`). `DROP INDEX IF EXISTS crdt_update_ws_doc_seq` (duplicate of the PK). `venus_mark_dirty` runs on **one** trigger: `crdt_update_dirty`, `FOR EACH STATEMENT` with `REFERENCING NEW TABLE AS ins`, upserting `max(seq)`. The `crdt_snapshot` dirty triggers are **dropped** (P1): compact rewrites trail rows the flush already marked at that same clock, so they only copied the whole page `bin` into a transition table. The function no-ops unless `TG_TABLE_NAME = 'crdt_update'`, so a leftover snapshot trigger on an old volume is harmless until `migrate` drops it. `ON CONFLICT` uses `GREATEST(dirty.clock, EXCLUDED.clock)` so an out-of-order flush cannot rewind (D1). Inner `EXCEPTION` `RAISE WARNING` so a missing `dirty` cannot roll back persist (D2). Function `SET search_path = public` and `INSERT INTO public.dirty` (D4). A retried flush can re-mark a pinned clock; M3 ignores `clock <= last_flushed` (D3). The trigger batch runs on first migrate, ROW→STATEMENT cutover, or snapshot-trigger removal; later boots skip `DROP`/`CREATE TRIGGER` so they do not take AccessExclusive on live tables. Hub does not write `jobs`. Not keck `jwst` docs. Included with `include_str!` — not a separate migration runner.

### `lease.rs`

Wiki sticky. `try_acquire` inserts or steals an **expired** row; `LeaseError::Held` if another owner’s `lease_until` is still in the future; `LeaseError::Store` on SQL/pool failure. `heartbeat` extends TTL (single id, for tests and L4). `heartbeat_many` is one `UPDATE … ANY($2::uuid[]) RETURNING workspace_id::text`; missed ids are the input not in that set and `Hub::heartbeat` sheds those rooms. A statement error logs once and returns no misses (not a steal). `drop_one` on idle eviction; `drop_all` on drain.

Grain is **`workspace_id`**, not cookie, not IP, not `doc_id`.

### `http.rs`

Axum `Router`. CORS: methods `GET`/`HEAD`/`POST`/`DELETE`, headers `Content-Type` and `If-None-Match`, origins from `AppState.cors_origins` (`HUB_CORS_ORIGINS`; empty list omits the layer; the request `Origin` is never echoed). Body limit 32 MiB. `{workspace_id}` must be a hyphenated UUID (`workspace_id_ok`; mixed case is lowercased) or the handler returns **400** before lease or SQL.

| Handler | Calls |
|---|---|
| `GET /` | Plain `venus-hub` |
| `POST /collaboration/{id}` | Health JSON `{ "protocol": "AFFiNE" }`; **no** lease |
| `GET /collaboration/{id}` | No `Upgrade: websocket` → same health JSON as POST (`OptionalWs`). Upgrade → `Hub::get_room` then `ws.protocols(["AFFiNE"])`; **503** only on `GetRoomError::Held`; store/hydrate fail → **500**. Bad id → **400**. |
| `GET /api/block/{id}/export` | `Hub::live_export` (hot: clone room, drop `rooms`, encode; cold: single-flight SQL per `workspace_id`) |
| `POST /api/blobs/{id}` | `blob_hash(&body)` + `db::put_blob` (no extra copy) |
| `GET`/`HEAD`/`DELETE /api/blobs/…/{hash}` | GET/HEAD: `Cache-Control: public, max-age=31536000, immutable` + quoted `ETag` of the hash; matching `If-None-Match` → `blob_len` then **304** (missing → 404); else `get_blob` / `blob_len`. DELETE: `delete_blob` |

No `/api/block/:id/:block` CRUD. `handle_socket`: `connect_client`, send `attach` hello, then select (binary → `handle_binary`; outbound bounded mpsc → socket; Full/closed/over-budget → detach, `recv` None closes the socket). Attach `Err` or a failed hello send **`detach`**. Server ping every 30s (`Option<Interval>`; `ws_ping = 0` is `None`, not `interval_at(0)`); missed pong detaches. Client ping still answered here, not in `Room`. Upgrade caps message/frame at 512 KiB.

### `blobs.rs`

`blob_hash`: SHA-256, base64url **with padding** (BlockSuite `sha()`). `sniff_content_type` for GET `Content-Type` (HEAD does not sniff). No I/O. Fixture: body `hello-blob` → `V6JWxl21rxj4oEx6Qt8mwwCd0BOB-6qux7Qy_DDUyNA=`.

## Tests

| File | Needs | Covers |
|---|---|---|
| `tests/recon.rs` | Nothing | api-map Chosen backend; y-octo apply of browser fixtures; two `Room` clients Step2/Update; lagged client Full-detach (256 slots) and byte-budget detach; truncated frame applies prefix; fan-out `Bytes` share storage; crate has no keck deps |
| `tests/store.rs` | Docker (testcontainers `postgres:16`) or `DATABASE_URL` / `POSTGRES_*` | `push_update` → `get_doc`; new connection still hydrates; compact merges trail; `get_doc` during compact keeps `spike.k=v`; garbage trail bin skipped, both good keys kept; corrupt snapshot skipped (trail hydrates, row kept, compact still fails); hydrate failure releases the lease; flush put-back after closed pool; abort during INSERT then leftover flush; P3 skip compact SQL when trail short; SQL rebuild compact; compact skips write if the trail moved during merge; foreign trail row survives compact; blob put/get/`blob_len`; HTTP GET `Cache-Control` + quoted `ETag`, `If-None-Match` **304**, HEAD `Content-Length` without body; concurrent `get_room` single-flight; concurrent cold `live_export` one SQL per `workspace_id`; one-statement heartbeat miss set (SQL error is not a steal); migrate drops leftover `crdt_update_ws_doc_seq`; two migrates serialize; persist cap drops oldest and keeps newest; `get_room` after shutdown is Store and does not acquire; `compact_after < 1` skips compact; garbage workspace_id is 400 on blob/export; `connect_with` honors max/min connections; CORS preflight allowlist (DELETE yes, PUT no; empty list has no Allow-Origin) |
| `tests/ws.rs` | Docker (same Postgres as store) | `cargo test -p venus-hub --test ws`. tokio-tungstenite + y-octo `Doc` (no Node, no keck). A→B `spike.k=v`; late joiner Step2; second `Hub` WS is 503; stolen lease sheds RAM (no further `INSERT`); idle room sheds and drops the lease; Format/bold fan-out without crash; wait ≥2s, `Hub::shutdown`, new `Hub` hydrates; SIGTERM before tick flushes and drops lease (reopen is a **new** Hub); abort persist without flush, client Update restores; oversized WS binary closes; missed pong closes; zero `ws_ping` does not panic; plain GET `/collaboration` is health JSON; garbage `workspace_id` is 400 and does not acquire |
| `src/*` `#[cfg(test)]` | Nothing | protocol round-trip, trailing-bytes decode, format mark, DSN/sqlite reject, blob hash, `POSTGRES_SSLMODE` allowlist; `request_stop` stores a Notify permit; `RwLock<Doc>` readers overlap, apply waits for a write; persist prefix clone shares `Bytes` storage; zero `ws_ping` interval is `None`; `workspace_id` UUID; blob `If-None-Match` vs quoted hash / `*`; `compact_after < 1`; pool/timer env parse and required-var error; CORS origin parse (`*`, path, empty) |

## Deploy

[`deploy/hub/Dockerfile`](../../../../deploy/hub/Dockerfile): build on `rust:1.90-bookworm`, copy the binary into `debian:bookworm-slim` with `ca-certificates`, run as `USER venus` (uid 65532). No `POSTGRES_PASSWORD` / `DATABASE_URL` in the image — Compose injects them (`venus_hub`). `EXPOSE 3000`. Do **not** `COPY deploy/octobase`.
