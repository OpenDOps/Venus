# M3.0 — steps 1–3 review

Review of `crates/venus-hub` against [plan.md](./plan.md) DoD for [step-recon-hub](./plan.md#1-step-recon-hub), [step-store](./plan.md#2-step-store), and [step-ws](./plan.md#3-step-ws). Board: [M3.0.state.yaml](./M3.0.state.yaml) (those three **done**). Crate map: [hub files](../components/hub/files.md). Process: [hub architecture](../components/hub/architecture.md). HA contract (not closed): [high-availability.md](./high-availability.md).

**This does not reopen those steps.** Named Given/When/Then scenarios passed. Findings below are remaining **correctness**, **performance**, and **resource exhaustion** in that crate, each with a fix proposal. Do not invent extra plan scenarios from this file.

**Auth / authorization is out of scope here.** WS, export, and blob routes stay open until a separate authorization module exists. Do not bolt tokens onto `handle_socket` as part of these fixes. [Security surface](#security-surface-not-authorization) is what remains after a token check — unbounded queues, missing caps, image hygiene — not a proposal to add auth here.

**Date:** 2026-09-12. Source: crate as of step-ws.

**Second pass:** 2026-09-12, after [L1](#l1--hydrate-vs-compact)–[L3](#l3--503-lies), [L5](#l5--drain-race), [L6](#l6--ghost-client), [P1](#p1--attach-holds-clients-during-encode)–[P3](#p3--compact-every-tick), [P5](#p5--head-blob-loads-body) landed. [L7](#l7--compact-from-ram-deletes-foreign-rows)–[L15](#l15--malformed-frame-truncates-a-batch-silently), [P7](#p7--update-copied-per-persist-and-per-client)–[P9](#p9--cold-room-hydrate-stampede), and [S1](#s1--unbounded-outbound-queue-per-client)–[S5](#s5--runtime-image-is-the-toolchain-image-as-root) are from that pass. [L7](#l7--compact-from-ram-deletes-foreign-rows), [L8](#l8--flush-is-not-cancellation-safe), [L10](#l10--heartbeat_many-aborts-on-the-first-sql-error), [L11](#l11--lease-ttl-is-only-2-heartbeats), [L12](#l12--serve-error-skips-shutdown), [L9](#l9--non-upgrade-get-is-400-not-health-json), [L13](#l13--one-corrupt-trail-bin-bricks-a-room), [L14](#l14--notify_waiters-can-lose-the-stop-wakeup), [L15](#l15--malformed-frame-truncates-a-batch-silently), [P7](#p7--update-copied-per-persist-and-per-client), [P8](#p8--persist_tasks-grows-forever), [P9](#p9--cold-room-hydrate-stampede), [S1](#s1--unbounded-outbound-queue-per-client), [S3](#s3--no-ws-message-cap-no-server-ping), and [S5](#s5--runtime-image-is-the-toolchain-image-as-root) are **done**.

**Third pass:** 2026-09-12, re-read of the crate with every second-pass fix landed. [L16](#l16--hydrate-failure-squats-the-lease)–[L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist), [P10](#p10--mutexdoc-serializes-reads-against-apply)–[P17](#p17--pool-defaults-are-the-only-pool-config), and [S6](#s6--outbound_cap-bounds-frames-not-bytes)–[S10](#s10--persist-buffer-is-unbounded) are new; [L16](#l16--hydrate-failure-squats-the-lease), [L17](#l17--corrupt-snapshot-bin-is-still-fail-closed), [L18](#l18--zero-ws_ping-panics-interval_at), [L19](#l19--shutdown-has-no-stopping-gate), [L20](#l20--migrate-recreates-triggers-with-no-lock), [L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist), [S6](#s6--outbound_cap-bounds-frames-not-bytes), [S8](#s8--sslmodedisable-is-hardcoded), [S10](#s10--persist-buffer-is-unbounded), [P10](#p10--mutexdoc-serializes-reads-against-apply), [P11](#p11--compact-holds-the-exclusive-lock-across-the-merge), [P12](#p12--heartbeat-is-n-round-trips-and-bursts), [P14](#p14--duplicate-index-on-crdt_update), [P15](#p15--persist-buffer-clones-every-bin-per-flush), and [P16](#p16--blob_post-copies-the-body) are **done**. [S7](#s7--unauthenticated-export-is-a-read-amplifier) crate slice is **done** (closing the route is auth). [P17](#p17--pool-defaults-are-the-only-pool-config) is **done**. [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction) is **done** (step 8). [S9](#s9--cors-allows-any-method-from-six-localhost-origins) is **done**. [S10](#s10--persist-buffer-is-unbounded) was added after the pass was first written up, when rating [S6](#s6--outbound_cap-bounds-frames-not-bytes) surfaced the same unbounded shape on the persist side. Third-pass rows carry a two-axis severity — see [How severity is rated](#how-severity-is-rated). Nothing from the first two passes regressed, and the plan’s step 1–3 **Do not** lists still hold. This pass also corrects two stale claims in the earlier text: the blob hash is already server-computed ([S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered)) and there is no `compact_after` env override ([Not wired / cleanup](#not-wired--cleanup)).

## How severity is rated

Third-pass rows carry `Sev` as **today → later** plus a fix `Effort`. Two facts suppress almost every third-pass finding right now, and both are scheduled to stop being true:

- **The port is not reachable by untrusted callers.** [step-compose-hub](./plan.md#4-step-compose-hub) is what exposes it, and the auth module is deferred by design, so “later” for a security row means *once the port is reachable*. Compose host ports bind `127.0.0.1` (LAN cannot open `:3000` / `:8080`); localhost and the Compose network still can.
- **There is one workspace with one small page.** “Later” for a performance row means *once a page is large or many wikis share one process* — which is also when [P6](#p6--rooms-never-evicted) stops being a single leaked room.

So a `None → High` row is not a contradiction: it is a finding with no impact on today’s deployment and a bad one on the next. Rate the fix by `Effort` against the `later` column, not the `today` column. First- and second-pass rows keep their original single `Sev`, which was written against the same “later” assumption.

Four pairs are worth more than the sum of their rows, because in each case one item sets the other’s real severity:

- [S7](#s7--unauthenticated-export-is-a-read-amplifier) with [P17](#p17--pool-defaults-are-the-only-pool-config) — both crate slices **done**. Cold export is one SQL flight per id; pool / persist / compact are required env (Compose `32` / `4` / `10s`). Size `HUB_DB_MAX_CONNECTIONS` for a storm of *distinct* ids plus blob GETs and flushes. Closing the export route is auth.
- [S7](#s7--unauthenticated-export-is-a-read-amplifier) with [P10](#p10--mutexdoc-serializes-reads-against-apply) — [P10](#p10--mutexdoc-serializes-reads-against-apply) is **done** (`RwLock`); joiners and exports share. Cold export is now one SQL flight per id; the hot path still encodes concurrently. A long `encode_v1` still takes a read, so a storm of a *hot* wiki can still stall typing, it just no longer serializes the joiners against each other.
- [S10](#s10--persist-buffer-is-unbounded) with [P15](#p15--persist-buffer-clones-every-bin-per-flush) — both **done**: persist is `Bytes` (clone is a refcount) and capped at 8 MiB, dropping oldest bins loudly rather than refusing applies.
- [P12](#p12--heartbeat-is-n-round-trips-and-bursts) with [L11](#l11--lease-ttl-is-only-2-heartbeats) and [L4](#l4--lease-steal-ram-stays) — the only performance row that can cost correctness. A heartbeat round slower than its interval fires catch-up beats, spends L11’s three-beats-per-TTL margin, and opens L4’s split-brain window.

## Status


|                       |                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Named DoD             | recon + store + ws green, including crash/client resend                                                                                                                                                                                                                                                                                                                                                                                                      |
| High correctness open | —                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| High correctness done | [L1](#l1--hydrate-vs-compact), [L2](#l2--flush-drops-bins), [L7](#l7--compact-from-ram-deletes-foreign-rows), [L8](#l8--flush-is-not-cancellation-safe)                                                                                                                                                                                                                                                                                                      |
| Medium/low done       | [L3](#l3--503-lies), [L4](#l4--lease-steal-ram-stays), [L5](#l5--drain-race), [L6](#l6--ghost-client), [L9](#l9--non-upgrade-get-is-400-not-health-json), [L10](#l10--heartbeat_many-aborts-on-the-first-sql-error), [L11](#l11--lease-ttl-is-only-2-heartbeats), [L12](#l12--serve-error-skips-shutdown), [L13](#l13--one-corrupt-trail-bin-bricks-a-room), [L14](#l14--notify_waiters-can-lose-the-stop-wakeup), [L15](#l15--malformed-frame-truncates-a-batch-silently) / [P1](#p1--attach-holds-clients-during-encode), [P2](#p2--export-holds-rooms-lock), [P3](#p3--compact-every-tick), [P4](#p4--sequential-insert), [P5](#p5--head-blob-loads-body), [P6](#p6--rooms-never-evicted), [P7](#p7--update-copied-per-persist-and-per-client), [P8](#p8--persist_tasks-grows-forever), [P9](#p9--cold-room-hydrate-stampede) |
| Resource exhaustion   | [S1](#s1--unbounded-outbound-queue-per-client) **done**; [S3](#s3--no-ws-message-cap-no-server-ping) **done**; [S5](#s5--runtime-image-is-the-toolchain-image-as-root) **done**; [S6](#s6--outbound_cap-bounds-frames-not-bytes) **done**; [S10](#s10--persist-buffer-is-unbounded) **done**; [S2](#s2--unauthenticated-room-creation-is-the-amplifier) id shape **done** (eviction [P6](#p6--rooms-never-evicted) **done**); [S7](#s7--unauthenticated-export-is-a-read-amplifier) crate slice **done** (closing the route is auth); [S9](#s9--cors-allows-any-method-from-six-localhost-origins) **done** |
| Third pass done        | [L16](#l16--hydrate-failure-squats-the-lease), [L17](#l17--corrupt-snapshot-bin-is-still-fail-closed), [L18](#l18--zero-ws_ping-panics-interval_at), [L19](#l19--shutdown-has-no-stopping-gate), [L20](#l20--migrate-recreates-triggers-with-no-lock), [L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist), [S6](#s6--outbound_cap-bounds-frames-not-bytes), [S8](#s8--sslmodedisable-is-hardcoded), [S10](#s10--persist-buffer-is-unbounded), [P10](#p10--mutexdoc-serializes-reads-against-apply), [P11](#p11--compact-holds-the-exclusive-lock-across-the-merge), [P12](#p12--heartbeat-is-n-round-trips-and-bursts), [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction), [P14](#p14--duplicate-index-on-crdt_update), [P15](#p15--persist-buffer-clones-every-bin-per-flush), [P16](#p16--blob_post-copies-the-body), [S7](#s7--unauthenticated-export-is-a-read-amplifier) crate slice, [P17](#p17--pool-defaults-are-the-only-pool-config), [S9](#s9--cors-allows-any-method-from-six-localhost-origins) |
| Third pass open        | Closing [S7](#s7--unauthenticated-export-is-a-read-amplifier)’s route is auth. |
| Remaining             | [Still to fix](#still-to-fix)                                                                                                                                                                                                                                                                                         |
| Auth                  | Deferred — separate authorization module, not this crate’s merge buffer                                                                                                                                                                                                                                                                                                                                                                                      |
| Board next            | [step-verify](./plan.md#9-step-verify)                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Steps 4–9             | Steps 4–8 **done**. 9 not claimed.                                                                                                                                                                                                                                                                                                                                                                              |

Apply is y-octo, one queue per room, opaque `BYTEA`, no keck, no Block REST. Compact/hydrate isolation, batched flush (drain after commit), typed `get_room` errors, encode-then-insert attach, export that does not hold `Hub.rooms` during encode, compact skip (SQL rebuild, not RAM), and SIGTERM join-then-`drop_all` are **done**. Hard crash before a persist tick can still lose ≤~1s; surviving clients resend ([plan step 3](./plan.md#3-step-ws)).

## What already matches the plan

One `RwLock<Doc>` per room (reads share; apply writes). One persist buffer per room, not per socket. Origin is not echoed. Empty / all-zero bins are skipped. Apply errors are logged instead of panicking. SQL is parameterized. Lease `UPSERT` only steals when expired or same owner. Compact is off the insert path. Those plan **Do not** items hold.

## Logical errors

Lease/drain completeness is [step-ha-owner](./plan.md#7-step-ha-owner) (**done**). L4/L5/P6 are the crate shape that step used.


| ID                                                        | Sev  | Status   | Bug                                        | Where                                    | What happens                                                                                                                                                        |
| --------------------------------------------------------- | ---- | -------- | ------------------------------------------ | ---------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [L1](#l1--hydrate-vs-compact)                             | High | **done** | Hydrate vs compact                         | `db.rs` `hydrate_doc`                    | Was: snapshot `SELECT` then trail `SELECT` not one isolation snapshot. Compact could replace the snapshot and `DELETE` the trail between them.                      |
| [L2](#l2--flush-drops-bins)                               | High | **done** | Flush drops bins                           | `Room::flush`                            | Was: `mem::take` then `INSERT`; SQL error dropped RAM copies.                                                                                                       |
| [L3](#l3--503-lies)                                       | Med  | **done** | 503 lies                                   | `http.rs` `collaboration_get`            | Was: any `get_room` `Err` is 503 “owned by another hub”.                                                                                                            |
| [L4](#l4--lease-steal-ram-stays)                          | Med  | **done** | Lease steal, RAM stays                     | `lease.rs`, `Hub`                        | Was: heartbeat miss only logged. Old process kept apply + persist. `Hub::heartbeat` sheds the room (no `try_acquire`). |
| [L5](#l5--drain-race)                                     | Med  | **done** | Drain race                                 | `Hub::shutdown`                          | Was: `request_stop` + flush + `drop_all` without join or `rooms.clear()`.                                                                                           |
| [L6](#l6--ghost-client)                                   | Low  | **done** | Ghost client                               | `handle_socket`                          | Was: `attach` inserted then encoded; `Err` returned without `detach`.                                                                                               |
| [L7](#l7--compact-from-ram-deletes-foreign-rows)          | High | **done** | Compact from RAM deletes foreign rows      | `Room::compact_if_needed`, `db::compact` | Was: `live` bin is this process’s RAM, but `DELETE seq <= max_seq` removes **any** owner’s rows.                                                                    |
| [L8](#l8--flush-is-not-cancellation-safe)                 | High | **done** | Flush is not cancellation-safe             | `Room::flush`, `Hub::shutdown`           | Was: `mem::take` before the `INSERT` await; join-timeout `abort()` dropped the batch.                                                                               |
| [L9](#l9--non-upgrade-get-is-400-not-health-json)         | Low  | **done** | Non-upgrade `GET` is 400, not health JSON  | `http.rs` `collaboration_get`            | Was: `WebSocketUpgrade` rejected before the body. `OptionalWs` returns POST’s protocol JSON; malformed upgrade still 400. |
| [L10](#l10--heartbeat_many-aborts-on-the-first-sql-error) | Med  | **done** | `heartbeat_many` aborts on first SQL error | `lease.rs`                               | Was: `self.heartbeat(id).await?` skipped every later workspace.                                                                                                     |
| [L11](#l11--lease-ttl-is-only-2-heartbeats)               | Med  | **done** | Lease TTL is only 2 heartbeats             | `config.rs`, `main.rs`                   | Was: TTL 20s, heartbeat 10s. Now interval = `ttl / 3` (6s) and `ttl >= 3 × interval`.                                                                               |
| [L12](#l12--serve-error-skips-shutdown)                   | Low  | **done** | Serve error skips shutdown                 | `main.rs`                                | Was: `serve.await.context("serve")?` returned before `hub.shutdown()`.                                                                                              |
| [L13](#l13--one-corrupt-trail-bin-bricks-a-room)          | Med  | **done** | One corrupt trail bin bricks a room        | `db::hydrate_with_trail_len`             | Was: first `apply_v1` error fails the whole hydrate → `get_room` 500 forever. Skip-and-log; `trail_len` still counts the row.                                        |
| [L14](#l14--notify_waiters-can-lose-the-stop-wakeup)      | Low  | **done** | `notify_waiters` can lose the stop wakeup  | `Room::request_stop`                     | Was: stores no permit. A stop between the `stopped()` pre-check and `notified()` was missed.                                                                        |
| [L15](#l15--malformed-frame-truncates-a-batch-silently)   | Low  | **done** | Malformed frame truncates a batch silently | `protocol.rs` `decode_sync_messages`     | Was: `break` on `Err(_)` / non-advancing tail with no log. Now returns `remaining`; `handle_binary` warns once per frame. Socket stays open.                          |
| [L16](#l16--hydrate-failure-squats-the-lease)             | High | **done** | Hydrate failure squats the lease           | `Hub::open_room`                         | Was: lease acquired **before** hydrate, and a hydrate error left the row owned by this process with no room — nothing heartbeated it, retries renewed it, other hubs got 503 for a wiki nobody served. |
| [L17](#l17--corrupt-snapshot-bin-is-still-fail-closed)    | Med  | **done** | Corrupt snapshot bin is still fail-closed  | `db::hydrate_with_trail_len`             | Was: [L13](#l13--one-corrupt-trail-bin-bricks-a-room) skipped bad **trail** bins but the snapshot `apply_v1` still propagated, so one bad snapshot row was a permanent 500 (and with [L16](#l16--hydrate-failure-squats-the-lease) a permanent lease squat). |
| [L18](#l18--zero-ws_ping-panics-interval_at)              | Low  | **done** | Zero `ws_ping` panics `interval_at`        | `handle_socket`                          | Was: `ping_enabled` guarded the `select!` arm, but `interval_at(…, ping)` ran unconditionally and tokio panicked on a zero period. |
| [L19](#l19--shutdown-has-no-stopping-gate)                | Low  | **done** | `shutdown` has no stopping gate            | `Hub::shutdown`                          | Was: a `get_room` racing the drain re-`try_acquire`s and spawned a persist task after `rooms.clear()` + `drop_all`. |
| [L20](#l20--migrate-recreates-triggers-with-no-lock)      | Low  | **done** | `migrate` recreates triggers with no lock  | `db::migrate`                            | Was: `DROP TRIGGER` / `CREATE TRIGGER` on every start with no advisory lock. Two hubs starting together contended on the same DDL. |
| [L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist) | Low | **done** | Fan-out lost if frame encode fails after persist | `Room::apply_and_fanout`           | Was: apply and persist push, then `encode_doc_update` `?`. Now the frame is built first; a framing error leaves RAM and the persist buffer untouched. |


### L1 — Hydrate vs compact — **done**

Default Postgres is **READ COMMITTED**. Wrapping the two `SELECT`s in a transaction does **not** fix it — the second statement can see compact’s commit.

**Proposal**

1. `hydrate_doc` / `get_doc`: one transaction, `SET TRANSACTION ISOLATION LEVEL REPEATABLE READ` (or sqlx equivalent). Read snapshot, then trail, **then** `COMMIT`, then `apply_v1` in RAM. Keep the tx short (I/O only). Postgres RR takes the snapshot at the first statement, so compact’s later `DELETE` is invisible. Serialization failure → retry the hydrate once.
2. `compact`: do not `COUNT(*)` outside a lock. `BEGIN`; `SELECT bin FROM crdt_snapshot WHERE … FOR UPDATE` (if no row, `INSERT … ON CONFLICT DO NOTHING` then `FOR UPDATE` again, or `pg_advisory_xact_lock(hashtext(workspace_id), hashtext(doc_id))`); re-read trail length; if still `≥ threshold`, `MAX(seq)`, read trail `seq <= max_seq`, merge, UPSERT snapshot, `DELETE … seq <= max_seq`; `COMMIT`. Hydrate with `FOR SHARE` on the same row waits behind this, which is stronger than RR alone.
3. Test in `tests/store.rs` (not a new plan scenario): push ≥ `compact_after` updates, compact, `get_doc` still applies; a second task hydrating during compact must not drop keys. A tight loop of compact vs `get_doc` is enough.

Prefer row locks (2) plus RR hydrate (1). Compact from RAM was [P3](#p3--compact-every-tick) step (2); it is reverted ([L7](#l7--compact-from-ram-deletes-foreign-rows)).

**Landed** (2026-09-12): `hydrate_doc` uses REPEATABLE READ + `pg_advisory_xact_lock_shared` + snapshot `FOR SHARE`, then `COMMIT`, then apply. One retry on `40001`/`40P01`. `compact` takes `pg_advisory_xact_lock` then snapshot `FOR UPDATE` before `COUNT(*)`. Tests: `compact_merges_trail_get_doc_keeps_value`, `get_doc_during_compact_does_not_drop_value`.

### L2 — Flush drops bins — **done**

`Room::flush` takes the persist `Vec`, then `flush_updates` inserts one row at a time. On error the taken bins are dropped (not put back). A mid-batch failure can leave some rows in SQL and the rest only in a dropped `Vec`.

**Proposal** (same patch as [P4](#p4--sequential-insert))

1. `flush_updates`: one transaction. `INSERT INTO crdt_update (workspace_id, doc_id, bin) SELECT $1, $2, unnest($3::bytea[])` (or a loop **inside** the same `BEGIN`/`COMMIT`). No autocommit per row. Dirty triggers still fire per row inside the tx.
2. `Room::flush`: `take` the buffer. On `Ok`, done. On `Err`, **put back at the front** (failed batch, then any bins applied during the SQL): preserve order. Do not put back if the tx committed.
3. Do not retry forever on the persist task; log and retry next tick with the restored buffer.
4. Test: inject a closed pool (or a CHECK that fails once) and assert the next flush after reconnect still hydrates `spike.k=v`.

**Landed** (2026-09-12): `flush_updates` is atomic for the batch — first a loop inside `BEGIN`/`COMMIT`, now one statement ([P4](#p4--sequential-insert)). `Room::flush` no longer `take`s before the await ([L8](#l8--flush-is-not-cancellation-safe)); SQL error leaves the buffer in place. Test: `flush_put_back_on_sql_error_then_succeeds`. Hard crash before the persist tick is **not** L2 — see plan **Crash, client resend**.

### L3 — 503 lies — **done**

**Proposal**

1. Typed error, not string match. `lease::try_acquire` already `bail!`s two owner cases — make that `LeaseError::Held { workspace_id, owner: Option<String> }` vs `sqlx`/`anyhow` for the rest. `Hub::get_room` returns `Result<Arc<Room>, GetRoomError>` with `Held` and `Store(anyhow::Error)` (hydrate/apply).
2. `collaboration_get`: `Held` → 503 + existing JSON. Anything else → 500 (log the error). Do not say “owned by another hub” on Postgres down.
3. Test with a Hub whose pool is closed: upgrade must not be 503. Lease 503 stays [step-ha-owner](./plan.md#7-step-ha-owner) DoD.

**Landed** (2026-09-12): `LeaseError::Held` vs `Store`. `GetRoomError` same split. HTTP: Held → 503; store/hydrate → 500 `"hub store failed"`. Test: `ws_upgrade_closed_pool_is_not_503`.

### L4 — Lease steal, RAM stays

`heartbeat_many` warns on miss and continues. The old `Arc<Room>` stays in `Hub.rooms` and still flushes. Two live owners until process exit. Do not treat current `workspace_lease` as HA close-out.

**Proposal** (crate shape for step 7)

1. `Hub` heartbeat (from `main`’s `heartbeat_interval` loop) calls a new `Hub::heartbeat` that maps each `workspace_id`. If `lease.heartbeat` returns `false`, **shed**: `request_stop`, `flush`, remove from `rooms`, close clients (drop the map entry so `mpsc` senders die and `handle_socket` breaks), do **not** keep applying. The missed ids already come from `heartbeat_many`.
2. Do not `try_acquire` again from the shed path. The thief owns SQL. This process must refuse new WS (`get_room` cache miss → `try_acquire` → 503) until the thief’s lease expires.
3. Prove in step 7: two processes, steal after TTL, old hub’s next apply does not `INSERT` (or clients get close). Until then, landing shed in `Hub::heartbeat` is enough to stop split-brain writes.

**Landed** (2026-09-13): `Hub::heartbeat` consumes `heartbeat_many` misses: remove from `rooms`, `request_stop`, leftover flush, drop client senders. Apply after stop does not persist. Shed does not `try_acquire`. Tests: `heartbeat_miss_sheds_room_no_further_insert`; Compose `pnpm compose:ha`.

### L5 — Drain race — **done**

**Proposal**

1. Keep a `JoinHandle` (or `Notify`) per persist task on `Hub`. `spawn_persist` loop: `select!` on the ~1s tick **and** a stop signal, so drain does not wait for the next tick.
2. `shutdown`: `request_stop` / notify all → last `flush` (in the task or on the room, mutex-safe) → **join** tasks (timeout ~persist_interval) → `rooms.clear()` → then `lease.drop_all`.
3. After `rooms.clear()`, a late `get_room` hydrates from SQL and `try_acquire`s. Do not return a drained `Arc<Room>` from the map.
4. Tests: existing `persist_after_ws_survives_hub_restart` already waits ≥2s. Add that `workspace_lease` rows for that owner are gone after `shutdown` (step 7 can own the second-process half).

**Landed** (2026-09-12): persist `select!` on tick vs `Notify`. `shutdown` notifies, joins (cap 5s), leftover `flush`, `rooms.clear()`, then `drop_all`. Tests: `shutdown_flushes_without_tick_and_drops_lease`; `persist_after_ws_survives_hub_restart` asserts lease rows are gone. Second process still step 7. Stop wakeup is [L14](#l14--notify_waiters-can-lose-the-stop-wakeup); abort during flush is [L8](#l8--flush-is-not-cancellation-safe).

### L6 — Ghost client — **done**

**Proposal**

1. `Room::attach`: encode hello **before** inserting into `clients` (only `doc` lock for encode). If encode fails, the map never saw this id. Then insert `tx`. That also helps [P1](#p1--attach-holds-clients-during-encode).
2. If insert-first is kept: `handle_socket` must `detach` on `attach` `Err` (same as the send-hello failure path). Prefer (1).

**Landed** (2026-09-12): encode Auth/awareness/Step1/Step2 under `doc` only, then `clients.insert`. `handle_socket` still `detach`s on attach `Err` (no-op if insert never ran). Recon: `two_clients_step2_and_update_without_keck` still fans out; detach then no late frame.

### L7 — Compact from RAM deletes foreign rows — **done**

[P3](#p3--compact-every-tick) step (2) is only sound while this process is the **sole** writer. `db::compact` with `live = Some(bin)` skipped the trail `SELECT` entirely, UPSERTed that bin as the snapshot with `clock = max_seq`, then `DELETE … seq <= max_seq`. `max_seq` is `MAX(seq)` over **all** rows, so a second owner’s flushed updates were deleted while the snapshot only contained this process’s RAM. Before P3 those rows were read and merged, so [L4](#l4--lease-steal-ram-stays) cost duplicate work; with live encode it cost committed edits. The L1 exclusive lock does not help — it serializes compact against hydrate, not against a second hub’s `INSERT`.

**Proposal** (not taken)

1. `Room::compact_if_needed` passes its expected `trail_len` into `db::compact`. Honour the `live` bin only when in-tx `COUNT(*)` equals that value, otherwise SQL rebuild.
2. Do not gate this on [L4](#l4--lease-steal-ram-stays) shed landing.

**Landed** (2026-09-12): dropped P3 step (2). Compact always rebuilds snapshot + trail from SQL under the L1 lock. The `trail_len` skip ([P3](#p3--compact-every-tick) step 1) stays. `db::compact` no longer takes a `live` bin. Tests: `compact_if_needed_sql_rebuild_merges_trail`; `compact_if_needed_keeps_foreign_trail_row` (out-of-band `INSERT`, both keys survive). [L4](#l4--lease-steal-ram-stays) shed is **done** — two owners must not apply in parallel.

### L8 — Flush is not cancellation-safe — **done**

`Room::flush` takes the buffer, then awaits `db::flush_updates`. The [L2](#l2--flush-drops-bins) put-back is in the `Err` arm, so it never runs if the future is **dropped** at that await. `Hub::shutdown` does exactly that: `drain_join_timeout` is the persist interval (1s default, 200ms floor / 5s cap) and the timeout arm calls `abort()`. A Postgres stall longer than that during SIGTERM loses a batch, and the leftover `room.flush` afterwards finds an empty buffer. This is a *graceful* path, so plan step 3’s “hard crash may lose ≤~1s” does not cover it.

**Proposal**

1. `Room::flush`: clone (or `Arc`) the bins for the `INSERT` and drain the buffer only **after** the transaction commits. Then a drop at any await point leaves the buffer intact and the next tick — or the leftover flush in `shutdown` — retries. Do not rely on `mem::take` + put-back.
2. Keep the abort as a last resort after the join timeout, but log the room id. With (1) it is no longer lossy.
3. `abort_persist_no_flush` stays the `kill -9` model (RAM dies with the process), so `crash_unflushed_client_resend_restores` is unaffected.
4. Test: a pool that blocks/errors slowly, `shutdown`, then a new `Hub` on the same DSN must still hydrate the value.

**Landed** (2026-09-12): clone the persist prefix, `INSERT`, drain that prefix only after `Ok`. Applies that land during SQL stay after the prefix. A `flush_mu` serializes leftover drain against an in-flight persist flush. SQL `Err` leaves the buffer as-is ([L2](#l2--flush-drops-bins)). Join timeout still `abort()`s and logs `workspace`. Test: `flush_abort_during_sql_then_leftover_succeeds` (1-connection pool held so `flush_updates` blocks after clone; abort; leftover flush hydrates). `abort_persist_no_flush` is unchanged.

### L9 — Non-upgrade GET is 400, not health JSON — **done**

Verified against a running hub: plain `GET /collaboration/<id>` returns `400 Bad Request`, body `Connection header did not include 'upgrade'`. `WebSocketUpgrade` is a handler argument, so axum runs its extractor (and its rejection) before the body, and `if !wants_websocket(&headers)` is unreachable. Compose’s `hub` healthcheck is a raw TCP probe today, so nothing is broken — but [step-compose-hub](./plan.md#4-step-compose-hub) or nginx pointing an HTTP probe at that URL would be.

**Proposal**

Pick one and make the docs match:

1. Keep the documented behaviour: take `Option<WebSocketUpgrade>` (or the `OptionalFromRequestParts` form) and return the protocol JSON when it is `None`.
2. Or delete `wants_websocket` and the branch, and fix the three places that claim otherwise: [hub README](../components/hub/README.md), [hub architecture](../components/hub/architecture.md), [hub files](../components/hub/files.md).

Prefer (1) — `POST` already answers the same JSON, and a GET-able health URL is cheap. Either way add a `tests/ws.rs` assertion on the status, so this cannot drift again.

**Landed** (2026-09-12): decision **1**. Axum 0.8 `WebSocketUpgrade` is not `OptionalFromRequestParts`, so `OptionalWs` extracts `None` when `Upgrade` is not `websocket` and otherwise runs the real extractor (malformed upgrade still 400). Handler returns POST’s `{ "protocol": "AFFiNE" }` and does not take a lease. Test: `get_collaboration_without_upgrade_is_health_json`.

Verified against a running hub: plain `GET /collaboration/<id>` returns `400 Bad Request`, body `Connection header did not include 'upgrade'`. `WebSocketUpgrade` is a handler argument, so axum runs its extractor (and its rejection) before the body, and `if !wants_websocket(&headers)` is unreachable. Compose’s `hub` healthcheck is a raw TCP probe today, so nothing is broken — but [step-compose-hub](./plan.md#4-step-compose-hub) or nginx pointing an HTTP probe at that URL would be.

**Proposal**

Pick one and make the docs match:

1. Keep the documented behaviour: take `Option<WebSocketUpgrade>` (or the `OptionalFromRequestParts` form) and return the protocol JSON when it is `None`.
2. Or delete `wants_websocket` and the branch, and fix the three places that claim otherwise: [hub README](../components/hub/README.md), [hub architecture](../components/hub/architecture.md), [hub files](../components/hub/files.md).

Prefer (1) — `POST` already answers the same JSON, and a GET-able health URL is cheap. Either way add a `tests/ws.rs` assertion on the status, so this cannot drift again.

### L10 — heartbeat_many aborts on the first SQL error — **done**

```text
for id in workspace_ids {
    if !self.heartbeat(id).await? {   // <- one Err ends the loop
```

A transient error on the first workspace skips the heartbeat for all the rest that tick. With [L11](#l11--lease-ttl-is-only-2-heartbeats) that is one tick away from a stealable lease on rooms this process is actively serving.

**Proposal**

Collect per-id outcomes instead of `?`: log `Err` per workspace, keep going, and return the count of misses (or `Vec<String>` of missed ids) so [L4](#l4--lease-steal-ram-stays)’s shed path can consume it. One SQL failure must not decide the fate of unrelated workspaces.

**Landed** (2026-09-12): `heartbeat_many` logged `Err` per id and continued. `Ok(false)` ids go in the returned `Vec<String>` (shed later). SQL errors are not treated as steals.

**P12** (2026-09-13): one `UPDATE … ANY($2)` for the tick. A statement error logs once and returns no misses — still not a steal, now at statement grain. Test: `heartbeat_many_continues_after_per_id_sql_error`.

### L11 — Lease TTL is only 2 heartbeats — **done**

`lease_ttl` is 20s (`config.rs`); `main.rs` beats every 10s. Two attempts per TTL means a single slow or failed beat leaves the row stealable while the RAM doc is still live and accepting applies — the [L4](#l4--lease-steal-ram-stays) window, entered by ordinary jitter rather than by a crash.

**Proposal**

Derive the interval from the TTL rather than hardcoding both: `heartbeat_interval = lease_ttl / 3` (≈6s at TTL 20s), or raise TTL to 30s and keep 10s. Assert `lease_ttl >= 3 * heartbeat_interval` in `Config` so env overrides cannot break the ratio. Step 7 owns the steal semantics; this is just the safety margin.

**Landed** (2026-09-12): default TTL 20s, `heartbeat_interval = ttl / 3` (6s). `Config::from_env` reads `HUB_LEASE_TTL_SECS` / `HUB_HEARTBEAT_INTERVAL_SECS` and refuses `ttl < 3 × interval`. Tests: `heartbeat_interval_is_ttl_divided_by_three`, `lease_ratio_requires_three_beats_per_ttl`.

### L12 — Serve error skips shutdown — **done**

`serve.await.context("serve")?` propagates, so a bind/accept failure exits `main` without `hub.shutdown()`: no final flush and no `lease.drop_all`, leaving rows for another hub to wait out the full TTL.

**Proposal**

Bind the result, run `hub.shutdown().await` unconditionally, then propagate. Same for the heartbeat task’s `JoinHandle` — abort it before the drain so it cannot re-`UPSERT` a lease that `drop_all` just removed.

**Landed** (2026-09-12): `main` binds `serve_result`, then `Hub::shutdown_with_heartbeat` (`abort` + join, then `shutdown`). Heartbeat starts only after listen succeeds. Test: `shutdown_with_heartbeat_aborts_then_drops_lease`.

### L13 — One corrupt trail bin bricks a room — **done**

`hydrate_with_trail_len` applies trail bins in a loop and propagates the first `apply_v1` error, so `get_room` returns `Store` → 500 with no path back: every reconnect re-reads the same bad row. The live apply path deliberately logs and continues ([What already matches the plan](#what-already-matches-the-plan)); hydrate is fail-closed for the same class of input.

**Proposal**

1. Skip a bin that fails to apply, `warn!` with `workspace_id`, `doc_id`, `seq`, and byte length, and count the skips. Still count skipped bins in `trail_len` so compact eventually merges past them.
2. Do not `DELETE` the bad row automatically — leave it for an operator, and let [step-8](./plan.md) observability surface the counter.
3. Test in `tests/store.rs`: `INSERT` a garbage `bin` between two good ones; `get_doc` still returns both good keys.

**Landed** (2026-09-12): `apply_trail_skip_bad` warns per failed trail bin (`workspace_id`, `doc_id`, `seq`, `bytes`) and keeps going. `trail_len` is the SQL row count, including skips. Hydrate does not `DELETE` the row. Compact uses the same skip so a later merge can rebuild the snapshot from the good bins (snapshot apply stays fail-closed). Test: `get_doc_skips_corrupt_trail_bin_keeps_good_keys`.

### L14 — notify_waiters can lose the stop wakeup — **done**

`Notify::notify_waiters` wakes only already-registered waiters and stores no permit. `spawn_persist` checks `stopped()`, then registers `notified()` inside `select!`; a `request_stop` landing in between is lost, so the task sleeps a full `persist_interval` and the drain relies on the join timeout — i.e. [L8](#l8--flush-is-not-cancellation-safe)’s abort.

**Proposal**

Use `notify_one` (stores a permit, so a later `notified()` returns immediately) or a `tokio_util::sync::CancellationToken` per room. With `CancellationToken` the `stop: AtomicBool` and the `Notify` collapse into one thing and `cancelled()` is level-triggered.

**Landed** (2026-09-12): `request_stop` uses `notify_one`. One persist waiter per room; a stop before `notified()` leaves a permit. Test: `request_stop_stores_a_permit_for_a_later_notified`. `shutdown_flushes_without_tick_and_drops_lease` still covers the drain path.

### L15 — Malformed frame truncates a batch silently — **done**

`decode_sync_messages` `break`s on `Err(_)` and on a tail that does not advance, returning whatever it parsed so far. `handle_binary` logs apply errors but never learns that messages were dropped, so a client encoding bug looks like “some edits just don’t arrive”.

**Proposal**

Return the parsed messages plus a trailing-bytes count, and `warn!` once per frame with `workspace_id`, offset, and remaining length. Keep the lenient behaviour (do not close the socket) — the point is that it becomes visible.

**Landed** (2026-09-12): `decode_sync_messages` returns `DecodedSync { messages, offset, remaining }`. `Room::handle_binary` warns once when `remaining > 0` (`workspace`, `offset`, `remaining`) and still applies the prefix. Tests: `decode_reports_trailing_bytes`, `truncated_frame_applies_prefix_without_closing`.

### L16 — Hydrate failure squats the lease — **done**

Third pass. `Hub::open_room` acquires the lease first and hydrates second:

```text
self.lease.try_acquire(workspace_id).await?;
let (doc, trail_len) = db::hydrate_with_trail_len(...).map_err(GetRoomError::Store)?;
```

On the hydrate error path the `workspace_lease` row stays but no `Room` is inserted. `main`’s heartbeat refreshes `hub.workspace_ids()`, which reads `Hub.rooms`, so this lease is never beaten — and never dropped until `shutdown`. That alone would cost one TTL. It is worse: a reconnecting browser calls the upgrade again, `try_acquire` matches on `owner` and succeeds, hydrate fails again, and `lease_until` is pushed out another TTL. As long as any client keeps retrying, this hub **renews** a lease for a workspace it cannot serve and a healthy hub gets `Held` → 503 indefinitely. That is a worse failure than the 503 [L3](#l3--503-lies) was written to avoid, because it is the *other* process that is wrongly refused.

Reachable today via [L17](#l17--corrupt-snapshot-bin-is-still-fail-closed) (one bad snapshot row) or any transient Postgres error between the two statements.

**Proposal**

1. On the hydrate error path, `lease.drop_one(workspace_id)` before returning `Store`. Log the drop. A failed open must not leave ownership behind.
2. Do not reorder to hydrate-then-acquire: two hubs would then both hydrate, and the lease is what makes the merge buffer single-writer.
3. Test in `tests/store.rs`: a `Hub` on a pool that fails hydrate (garbage snapshot row, or a closed pool after `try_acquire`), assert `get_room` is `Err(Store)` **and** that `workspace_lease` has no row for that owner, so a second `Lease` with a different owner can `try_acquire`.

**Landed** (2026-09-12): `open_room` matches on the hydrate result and calls `Hub::release_failed_open` before returning `Store`. That `drop_one`s the lease and `warn!`s; a failed drop is an `error!` and the row falls back to expiring on its TTL. Only reachable with no `Room` in the map for that id, so no live room can lose its lease. Order is unchanged (acquire, then hydrate) — hydrate-first would let two hubs hydrate. Test: `get_room_hydrate_failure_releases_the_lease` (lease pool live, hub pool closed; asserts no row for the owner and that a second owner can `try_acquire` immediately).

### L17 — Corrupt snapshot bin is still fail-closed — **done**

Third pass. [L13](#l13--one-corrupt-trail-bin-bricks-a-room) fixed the trail loop only. `hydrate_with_trail_len` still does `apply_v1(&mut doc, &bin)?` for the snapshot, so one unreadable `crdt_snapshot` row is a permanent 500 for that workspace with no path back — the exact failure mode L13 named, moved one row over. With [L16](#l16--hydrate-failure-squats-the-lease) it also squats the lease.

**Proposal**

1. Skip-and-log the snapshot the same way, then hydrate from the trail alone. `warn!` with `workspace_id`, `doc_id`, `clock`, and byte length, and make it loud — losing the snapshot is real data loss, unlike skipping one trail bin.
2. Do not `DELETE` or overwrite the bad snapshot from the hydrate path. Compact would then rebuild a snapshot from the trail and silently discard whatever the old snapshot held; that decision belongs to an operator.
3. Test: `UPDATE crdt_snapshot SET bin = …` to garbage with a good trail on top; `get_doc` still returns the trail’s keys.

**Landed** (2026-09-12): `db::hydrate_base_skip_bad_snapshot` builds the base doc; on `apply_v1` failure it `error!`s (`workspace_id`, `doc_id`, `bytes`, `error`) and returns a **fresh** `Doc`, because a failed apply may already have mutated the one it was given. Hydrate then continues from the trail. `compact` keeps `apply_v1(…).context("compact snapshot apply")?` on purpose, so it can never write a trail-only rebuild over the bad row: that workspace stops compacting (the persist task logs `compact` and keeps flushing) until an operator acts. Test: `hydrate_skips_corrupt_snapshot_and_keeps_the_row` (garbage snapshot + one good trail row; hydrate returns the trail’s key, compact is `Err`, the row is still there).

### L18 — Zero ws_ping panics interval_at

Third pass. `handle_socket` computes `let ping_enabled = ping > Duration::ZERO;` and guards the `select!` arm with it, but `interval_at(Instant::now() + ping, ping)` runs before the loop and `tokio::time::interval_at` panics on a zero period. So the “ping disabled” path the guard implies aborts the socket task instead. `ws_max_message` has the same class of guard done right (`.max(1)`).

**Proposal**

Either drop `ping_enabled` and document that `ws_ping` must be non-zero, or make it real: build the interval as `Option<Interval>` (`None` when zero) and keep the guard. Prefer the second — `AppState` is public and tests already override these knobs.

**Landed** (2026-09-13): `ws_ping_interval` returns `None` when `ping` is zero, so `interval_at` is never constructed with a zero period. `handle_socket` still guards the `select!` arm. Tests: `http::l18::ws_ping_interval_is_none_when_zero` (needs a tokio runtime), `ws_zero_ping_does_not_panic` (A→B apply with `ws_ping = ZERO`).

### L19 — shutdown has no stopping gate

Third pass. `Hub::shutdown` stops rooms, joins, clears `rooms`, then `drop_all`. Nothing marks the hub as stopping, so a `get_room` that interleaves can `try_acquire` a fresh lease and `spawn_persist` a task after the drain finished — a lease row and a live task outliving `shutdown`. `main` is safe because `axum::serve` has already returned when `shutdown_with_heartbeat` runs, so this is latent rather than live; it bites tests and any future embedder that serves and drains concurrently.

**Proposal**

An `AtomicBool` on `Hub` set at the top of `shutdown`. `get_room` returns `Store` (“hub is shutting down”) when set, before `try_acquire`. Cheap, and it makes `shutdown` idempotent as a side effect.

**Landed** (2026-09-13): `Hub.stopping` is set first in `shutdown`. `get_room` / `open_room` return `Store("hub is shutting down")` before `try_acquire`, and again after acquire (L16 `release_failed_open` if the lease was already taken) and before insert. The same `Hub` must not reopen rooms after drain; a new process hydrates SQL. Tests: `get_room_after_shutdown_is_store_and_does_not_acquire`; `shutdown_flushes_without_tick_and_drops_lease` opens a **new** `Hub` on the same pool to `encode_live`.

### L20 — migrate recreates triggers with no lock

Third pass. `db::migrate` runs `schema.sql` on every start, and that file ends with `DROP TRIGGER IF EXISTS` + `CREATE TRIGGER` for both tables. sqlx `raw_sql` uses the simple query protocol, so the batch is one implicit transaction and there is no window with the trigger missing — but two hubs booting together (Compose `hub-b`, or step 7’s second process) take the same DDL locks in the same order and one can fail or block behind the other.

**Proposal**

Wrap `migrate` in `pg_advisory_lock(<fixed key>)` / `pg_advisory_unlock`, or make the trigger install idempotent without the `DROP` (`CREATE OR REPLACE TRIGGER`, Postgres 14+). Prefer the advisory lock: it also covers the `CREATE TABLE IF NOT EXISTS` races and does not pin a server version.

**Landed** (2026-09-13): one pooled connection takes a **session** `pg_advisory_lock` (`SCHEMA_MIGRATE_LOCK`, not the `venus_doc_lock_key` hydrate/compact int8), runs `schema.sql` on **that** connection, then unlocks even if DDL fails. Tables+function commit first; `DROP`/`CREATE TRIGGER` is a second batch and runs only when the dirty trigger state is stale (`crdt_update_dirty` missing or `tgnewtable` unset, or a leftover snapshot trigger — [P1](./logicals-and-performance.md#p1--new-table-copies-bin)) — `CREATE OR REPLACE TRIGGER` still takes AccessExclusive, and hydrate vs compact grab `crdt_snapshot` / `crdt_update` in opposite orders, so recreating triggers on every boot deadlocks a live hub. The function is `CREATE OR REPLACE` on each start (that is the body). Test: `migrate_serializes_two_hubs` (`tokio::join!`, not `spawn`; one STATEMENT `crdt_update_dirty`, zero snapshot dirty triggers after [P1](./logicals-and-performance.md#p1--new-table-copies-bin)).

### L21 — Fan-out is lost if frame encode fails after persist

Third pass. `apply_and_fanout` applies, pushes the bin into the persist buffer, then `?`s on `encode_doc_update(bin)`. If that last step fails, the update is in the RAM doc and headed for SQL but no peer is told, so other tabs stay stale until they reconnect. `handle_binary` logs the error, which is the only reason this is Low.

**Proposal**

Encode the frame **before** the persist push (or before the apply), so a framing failure means nothing was half-committed. Ordering-only change, no new state.

**Landed** (2026-09-13): `apply_and_fanout` builds the Update frame first, then `apply_v1`, then the persist push, then broadcast. A framing error returns before RAM or the persist buffer change. Apply still takes the write lock only around `apply_v1` (P10). Existing recon fan-out tests still pass.

## Performance

Acceptable for one wiki and a few tabs. Apply is already exclusive on the room `RwLock`. Extra lock hold times and SQL chatter show up as soon as a page is large or many wikis share one process.


| ID                                                  | Issue                                                                                                                                     | Where                                  | Effect                                                                                                                      |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| [P1](#p1--attach-holds-clients-during-encode)       | **done** — encode under `doc`, then insert                                                                                                | `Room::attach`                         | Was: `clients` mutex held during full `encode_v1`.                                                                          |
| [P2](#p2--export-holds-rooms-lock)                  | **done** — clone `Arc<Room>`, drop map, then encode                                                                                       | `Hub::live_export`                     | Was: `Hub.rooms` lock held during `encode_live`.                                                                            |
| [P3](#p3--compact-every-tick)                       | **done** — skip unless `trail_len >= compact_after`; SQL rebuild (live encode reverted, [L7](#l7--compact-from-ram-deletes-foreign-rows)) | `spawn_persist`                        | Was: `COUNT(*)` + compact every ~1s.                                                                                        |
| [P4](#p4--sequential-insert)                        | **done** — one `INSERT … unnest($3::bytea[])`                                                                                             | `flush_updates`                        | Was: `INSERT` per bin inside one tx — N+2 round-trips per 1s window.                                                        |
| [P5](#p5--head-blob-loads-body)                     | **done** — `octet_length`; `Content-Length` from `i64`                                                                                    | `blob_head`                            | Was: `HEAD` loaded `BYTEA` via `get_blob`.                                                                                  |
| [P6](#p6--rooms-never-evicted)                      | **done** — idle eviction (no clients, empty persist, idle ≥ lease TTL) → shed + `drop_one` | `Hub.rooms`                            | Was: empty wiki kept `Doc`, persist task, and lease heartbeat forever. |
| [P7](#p7--update-copied-per-persist-and-per-client) | **done** — `Outbound` is `Bytes`; fan-out is a refcount bump | `apply_and_fanout`, `broadcast_except` | Was: persist clone + frame + `frame.clone()` per recipient. |
| [P8](#p8--persist_tasks-grows-forever)              | **done** — persist `JoinHandle` on the `Room`                                                                                             | `Room.persist_task`                    | Was: `Hub.persist_tasks` Vec of every room ever opened. Idle rooms drop the task in [P6](#p6--rooms-never-evicted). |
| [P9](#p9--cold-room-hydrate-stampede)               | **done** — single-flight per `workspace_id`                                                                                               | `Hub::get_room`                        | Was: N concurrent connects each `try_acquire` + full hydrate; N−1 `Doc`s discarded.                                      |


Third pass adds P10–P17. P10 and P11 are the two that show up first on a large page. `Sev` is read as **today → later**, per [How severity is rated](#how-severity-is-rated).


| ID | Sev | Effort | Issue | Where | Effect |
| --- | --- | --- | --- | --- | --- |
| [P10](#p10--mutexdoc-serializes-reads-against-apply) | **done** (Low → Med) | Small | `Mutex<Doc>` serialized reads against apply | `Room.doc` | Was: every join/export held exclusive `doc` for a full `encode_v1`. Now `RwLock`: hello, export, and Step1 share; only `apply_v1` writes. |
| [P11](#p11--compact-holds-the-exclusive-lock-across-the-merge) | **done** (Low → Med) | Medium | Compact held the exclusive lock across the merge | `db::compact` | Was: trail decode + `encode_v1` inside the tx holding `pg_advisory_xact_lock`. Now read+COMMIT, merge in RAM, re-lock and write only if `MAX(seq)` is unchanged. |
| [P12](#p12--heartbeat-is-n-round-trips-and-bursts) | **done** (None → Med) | Tiny | Heartbeat is N round-trips and bursts | `heartbeat_many`, `main.rs` | Was: one `UPDATE` per workspace per beat, and `main`’s `interval` kept `Burst`. Now one `UPDATE … ANY($2)` and `Delay`, matching persist. |
| [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction) | **done** (Low → Med) | Small | Dirty trigger is per row with a subtransaction | `schema.sql` | Was: `FOR EACH ROW` + per-row `EXCEPTION` savepoints. Now one STATEMENT `max(seq)` per flush; EXCEPTION at statement grain. Snapshot triggers later dropped ([P1](./logicals-and-performance.md#p1--new-table-copies-bin)). |
| [P14](#p14--duplicate-index-on-crdt_update) | **done** (Low → Low) | Trivial | Duplicate index on `crdt_update` | `schema.sql` | Was: `crdt_update_ws_doc_seq` matched the primary key’s columns. `DROP INDEX IF EXISTS` on every migrate. |
| [P15](#p15--persist-buffer-clones-every-bin-per-flush) | **done** (Low → Med) | Small | Persist buffer clones every bin per flush | `Room::flush` | Was: `Vec<Vec<u8>>` clone copied every queued bin each tick. Now `Vec<Bytes>`: clone is refcounts; L8 prefix drain unchanged. |
| [P16](#p16--blob_post-copies-the-body) | **done** (Low → Low) | Trivial | `blob_post` copies the body | `http.rs` `blob_post` | Was: `body.to_vec()` duplicated up to 32 MiB. Now `&body` into `blob_hash` / `put_blob`. |
| [P17](#p17--pool-defaults-are-the-only-pool-config) | **done** (Low → Med) | Small | Pool defaults were the only pool config | `db::connect_with`, `config.rs` | Was: `PgPool::connect` (~10 / 30s) and hardcoded persist 1s / compact 32. Now required `HUB_DB_MAX_CONNECTIONS` / `MIN` / `ACQUIRE_TIMEOUT_SECS`, `HUB_PERSIST_INTERVAL_MS`, `HUB_COMPACT_AFTER`. Compose `32` / `4` / `10s` / `1000` / `32`. Missing var names itself. |


Handshake always sends a full Step2. Plan allows that. Diff encode exists for inbound Step1 (`encode_step2_for`). Do not drop the immediate Step2; the M1 client uses it to mark `synced`.

### P1 — Attach holds clients during encode — **done**

**Proposal**

Same order as [L6](#l6--ghost-client): encode under `doc` only, drop that lock, then `clients.insert`. Broadcasts during encode are not blocked. Hello is still sent before `handle_socket` reads `mpsc`, so queued Updates land after Step2 (Yjs apply of a prefix then an Update is OK).

**Landed** with L6.

### P2 — Export holds rooms lock — **done**

**Proposal**

```text
let room = { let g = rooms.lock(); g.get(id).cloned() };
if let Some(room) = room { return room.encode_live().await; }
drop is already done — hydrate from SQL
```

Clone the `Arc<Room>`, drop `Hub.rooms`, then `encode_live`. One-line change.

**Landed** (2026-09-12): clone then encode. Test: `live_export_uses_ram_get_room_does_not_wait_on_rooms_lock` (unflushed RAM export; `get_room` of another wiki joined with export).

### P3 — Compact every tick — **done**

**Proposal**

1. Track `trail_len` on the room: add flushed batch length on successful flush; set to remaining (0 if all merged) after compact. Persist loop calls `compact` only when `trail_len >= compact_after`. Idle rooms do not `COUNT(*)`.
2. After a successful flush, compact may encode the **live** RAM doc (`encode_live`) instead of re-applying SQL. Only legal if flush just committed (RAM is a prefix of snapshot+trail, plus maybe a race apply). Sequence: `flush` (L2 tx) → if `trail_len >= threshold` → `encode_live` → compact SQL writes that bin and `DELETE seq <= max_seq` **under the L1 lock**. Applies that land after `encode_live` stay in the persist buffer / new trail rows (`seq > max_seq`) and must not be deleted. Do not hold the apply mutex across SQL.
3. First patch can be (1) only. (2) needs the L1 lock so hydrate does not see a new snapshot with a deleted trail it had not read.

**Landed** (2026-09-12): `Room::trail_len` (hydrate + successful flush). `compact_if_needed` returns without SQL when below threshold. Step (2) `encode_live` as the snapshot is **reverted** — see [L7](#l7--compact-from-ram-deletes-foreign-rows). Compact still runs under the L1 exclusive lock and always rebuilds from SQL. Tests: `compact_if_needed_skips_sql_when_trail_short`, `compact_if_needed_sql_rebuild_merges_trail`.

### P4 — Sequential INSERT — **done**

**Proposal**

Do [L2](#l2--flush-drops-bins) with `unnest($3::bytea[])` in one `INSERT`. That is the batch. `COPY` is not needed at this size (≤1s of typing). Dirty trigger stays `FOR EACH ROW`.

**Landed** (2026-09-12): `flush_updates` is one statement — `INSERT … SELECT $1, $2, bin FROM unnest($3::bytea[]) WITH ORDINALITY AS t(bin, ord) ORDER BY ord RETURNING seq`. N+2 round-trips (`BEGIN`, N × `INSERT`, `COMMIT`) become 1; a lone `INSERT` is its own transaction, so [L2](#l2--flush-drops-bins)’s put-back still sees an all-or-nothing failure. `WITH ORDINALITY` + `ORDER BY ord` pin `seq` to array order, which hydrate depends on via `ORDER BY seq`. Trigger was `FOR EACH ROW` at this landing; [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction) later made it STATEMENT. Test: `flush_batch_keeps_array_order_and_marks_dirty` (order in SQL, ascending seqs, `dirty.clock` = last seq). [L8](#l8--flush-is-not-cancellation-safe) is a separate drain-after-commit change.

### P5 — HEAD blob loads body — **done**

**Proposal**

`GET` still `SELECT bytes`. `HEAD`: `SELECT octet_length(bytes) FROM blob WHERE workspace_id = $1 AND hash = $2` — no `BYTEA` to the hub. Set `Content-Length` from that `i64`. Same 404 when no row. Do not add a `byte_len` column (schema is `CREATE IF NOT EXISTS`; extra columns need a real migrator).

**Landed** (2026-09-12): `db::blob_len`. `blob_head` sets `Content-Length` only (no sniff; that needs bytes). Test: `blob_put_get_head_len_without_loading_body`.

### P6 — Rooms never evicted

**Proposal**

Idle eviction is the same shed as [L4](#l4--lease-steal-ram-stays), on a timer instead of a stolen lease:

1. Room tracks `clients.len()` and last-empty time. Persist task: if no clients, persist buffer empty, and idle ≥ some TTL (e.g. 60s, or `lease_ttl`) → flush, stop, `rooms.remove`, `lease.drop_one`.
2. Next WS `get_room` hydrates from SQL and acquires again.
3. Do this with L4/L5, not as a one-off. Until then, one wiki (`venus-m0`) leaking one room is acceptable for M3.0 — but see [S2](#s2--unauthenticated-room-creation-is-the-amplifier): “one wiki” is only true while nothing untrusted can reach the port.

**Landed** (2026-09-13): `Hub::heartbeat` after the L4 miss loop evicts rooms with no clients, empty persist buffer, and `last_empty` ≥ lease TTL. Same shed as L4 plus `lease.drop_one`. Next `get_room` hydrates and acquires. Test: `idle_room_is_shed_and_lease_dropped`.

### P7 — Update copied per persist and per client — **done**

An inbound update is cloned into the persist buffer, moved into `encode_doc_update` to build the frame, then `frame.clone()`d once per recipient in `broadcast_except`. Three copies of every keystroke batch at 2 clients, N+2 at N.

**Proposal**

Frame as `Arc<[u8]>` (or `bytes::Bytes`) and make `Outbound` carry that, so fan-out is a refcount bump. `axum::extract::ws::Message::Binary` already takes `Bytes`, so the send path needs no extra copy either. The persist copy has to stay (different lifetime), but it can be the only one.

**Landed** (2026-09-12): `Outbound` is `mpsc::Sender<Bytes>`. [L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist) clones for the frame first, then `push`es the original into persist. `broadcast_except` / `send_to` clone `Bytes`. `handle_socket` sends `Message::Binary(frame)` with no extra copy. Test: `fanout_recipients_share_frame_bytes`.

An inbound update is cloned into the persist buffer, moved into `encode_doc_update` to build the frame, then `frame.clone()`d once per recipient in `broadcast_except`. Three copies of every keystroke batch at 2 clients, N+2 at N.

**Proposal**

Frame as `Arc<[u8]>` (or `bytes::Bytes`) and make `Outbound` carry that, so fan-out is a refcount bump. `axum::extract::ws::Message::Binary` already takes `Bytes`, so the send path needs no extra copy either. The persist copy has to stay (different lifetime), but it can be the only one.

### P8 — persist_tasks grows forever — **done**

`Hub.persist_tasks` is a `Vec<JoinHandle<()>>` pushed once per `get_room` miss and drained only by `shutdown` / `abort_persist_no_flush`. Finished handles are never reaped, so the Vec is a permanent record of every room the process ever opened.

**Proposal**

Move the handle onto the `Room` (it is 1:1) and drop it with the room in [P6](#p6--rooms-never-evicted)’s eviction, or keep a `JoinSet` and reap completed tasks each heartbeat. `shutdown` then iterates `rooms` instead of a parallel Vec, which also removes the ordering assumption between the two collections.

**Landed** (2026-09-12): `Room.persist_task` holds the `JoinHandle`. `spawn_persist` stores it while `Hub.rooms` is still locked so shutdown cannot miss it. `shutdown` / `abort_persist_no_flush` take the handle off each live room. No `Hub.persist_tasks` Vec. Idle rooms drop the task in [P6](#p6--rooms-never-evicted). Test: `concurrent_get_room_single_flight_per_workspace` (handle present after `get_room`; gone after abort).

### P9 — Cold-room hydrate stampede — **done**

`get_room` checks the map, and on a miss does `try_acquire` + full hydrate + insert, re-checking the map only at insert time. N sockets arriving together on a cold room each pay a full snapshot+trail read and N−1 throw the result away. Correct, but N× the hydrate cost on a large page — the same shape as a browser refresh storm after a deploy.

**Proposal**

Single-flight per `workspace_id`: keep `HashMap<String, Shared<…>>` of in-progress hydrations (or a per-id `Mutex`/`OnceCell` stored in the map so followers await the leader’s result). Do not hold `Hub.rooms` across the hydrate — that would undo [P2](#p2--export-holds-rooms-lock).

**Landed** (2026-09-12): `Hub.hydrating` is a `workspace_id` → waiter list. One leader runs `try_acquire` + hydrate; followers await a oneshot. `Hub.rooms` is not held across hydrate. A panicked leader drops waiters so they retry. Key is `workspace_id`, not `doc_id`. Test: `concurrent_get_room_single_flight_per_workspace` (8 tasks, `Arc::ptr_eq`, `hydrate_attempts() == 1`; a second wiki is a second flight).

### P10 — Mutex&lt;Doc&gt; serializes reads against apply

Third pass. [P1](#p1--attach-holds-clients-during-encode) moved the `clients` lock out of the hello encode, but the `doc` lock is still held for the whole of it: `Room::attach` calls `hello_frames`, which does `encode_state_vector` **and** a full `encode_v1`, under `self.doc.lock()`. `encode_live` (export) does the same. So on a large page every new tab and every export stalls the apply queue for the duration of a full encode, and simultaneous joiners serialize against each other as well as against typing.

The lock is stronger than the work needs. Every read path already takes `&Doc` — `encode_v1`, `encode_state_vector`, `encode_step2_for` — and only `apply_v1` needs `&mut Doc`.

**Proposal**

1. `Room.doc: RwLock<Doc>`. `hello_frames`, `encode_live`, and the Step1 reply take `read()`; `apply_and_fanout` takes `write()`. Joiners and exports then overlap and only apply excludes.
2. Keep the write section as narrow as it is now (apply only — the persist push and the framing are already outside it, and [L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist) keeps them outside).
3. Do not reach for a second `Doc` or a snapshot cache. One doc per room is the plan’s shape; this is a lock-kind change, not an architecture change.
4. Test: hold a read (an in-flight export on a room with a big doc) and assert an apply from another task completes; then the reverse. `live_export_uses_ram_get_room_does_not_wait_on_rooms_lock` is the pattern.

**Landed** (2026-09-12): `Room.doc` is `tokio::sync::RwLock<Doc>`. `attach` / `encode_live` / Step1 take `read()`; `apply_and_fanout` takes `write()` only around `apply_v1` (persist push and framing stay outside). One doc per room; no snapshot cache. Apply still waits while a reader is in `encode_v1` — that is the exclusive-write half, not a Mutex leftover. Tests: `doc_readers_overlap_apply_excludes` (parked reader; export and attach finish; apply stays pending until the reader drops), `encode_live_waits_for_a_write`.

### P11 — Compact holds the exclusive lock across the merge

Third pass. `db::compact` takes `pg_advisory_xact_lock`, then inside the same transaction reads the trail, `apply_v1`s every bin, `encode_v1`s the whole doc, and only then UPSERTs and `DELETE`s. The CPU-bound merge is inside the lock, so every hydrate for that workspace (which takes the shared lock, [L1](#l1--hydrate-vs-compact)) waits on decode-plus-encode, every `compact_after` updates. It also holds a pool connection idle-in-transaction for that time, which [P17](#p17--pool-defaults-are-the-only-pool-config) makes worse.

**Proposal**

1. Read snapshot + trail + `max_seq` under the lock, `COMMIT`, merge in RAM, then re-open a transaction, retake the exclusive lock, re-check that `MAX(seq)` is unchanged, and write. Bail to the next tick if it moved. Longer, but no CPU under the lock.
2. Cheaper alternative if (1) is too much for now: leave the shape alone and note the hold time. Compact is on the persist task, so the only victim is hydrate for the same workspace — acceptable at one wiki, not at many.
3. Do not move the merge back to RAM encode; that is [L7](#l7--compact-from-ram-deletes-foreign-rows).

**Landed** (2026-09-12): `load_compact` takes the exclusive lock, reads snapshot + trail + `max_seq`, **COMMIT**. `merge_compact_bin` applies in RAM (snapshot still fail-closed, L17). `commit_compact` retakes the lock, checks `MAX(seq)` is the planned value, then UPSERT + `DELETE seq <= max_seq`. If seq moved, skip (`merged: false`, trail left intact) and the persist tick retries. Still SQL rebuild, not live RAM ([L7](#l7--compact-from-ram-deletes-foreign-rows)). Tests: existing compact/hydrate races still pass; `compact_skips_write_when_trail_moves_during_merge` inserts in the gap via `compact_after_load`, asserts skip, then a plain `compact` merges.

### P12 — Heartbeat is N round-trips and bursts

Third pass. Two separate costs in the same loop.

`heartbeat_many` runs one `UPDATE` per workspace sequentially, so a hub with N rooms pays N round-trips every `ttl / 3` (6s by default) just to say it is alive. [L10](#l10--heartbeat_many-aborts-on-the-first-sql-error) needed per-id outcomes, not per-id statements.

`main`’s heartbeat uses `tokio::time::interval` with the default `MissedTickBehavior::Burst`, unlike `spawn_persist` which sets `Delay`. If one round takes longer than the interval, tokio fires the backlog back-to-back — the heartbeat amplifies its own load exactly when Postgres is already slow, and that is the moment [L11](#l11--lease-ttl-is-only-2-heartbeats)’s margin is being spent.

**Proposal**

1. One statement: `UPDATE workspace_lease SET lease_until = now() + make_interval(secs => $3) WHERE owner = $1 AND workspace_id = ANY($2) RETURNING workspace_id`. The returned set is what was refreshed; the difference against the input is the missed set, which is exactly what `heartbeat_many` returns today. L10’s guarantee is preserved — a whole-statement error logs once and returns no misses, so no id is mistaken for stolen.
2. `set_missed_tick_behavior(MissedTickBehavior::Delay)` on the heartbeat interval, matching the persist loop.
3. Keep single-id `heartbeat` for tests and for the [L4](#l4--lease-steal-ram-stays) shed path.
4. Test: extend `heartbeat_many_continues_after_per_id_sql_error` so a mix of owned and stolen ids returns exactly the stolen ones in one statement.

**Landed** (2026-09-13): `heartbeat_many` is one `UPDATE … workspace_id = ANY($2) RETURNING workspace_id`. Missed ids are the input not in that set. A statement error logs once and returns no misses (L10 at statement grain). `main` sets `MissedTickBehavior::Delay` on the heartbeat interval, matching persist. Single-id `heartbeat` is unchanged. Test: `heartbeat_many_continues_after_per_id_sql_error` (injected UPDATE error is not a steal; stolen + missing ids come back in input order, owned id refreshes).

### P13 — Dirty trigger is per row with a subtransaction

Third pass. `venus_mark_dirty` is `FOR EACH ROW` and its body wraps the `INSERT … ON CONFLICT` in a plpgsql `BEGIN … EXCEPTION` block, which Postgres implements as a subtransaction (a savepoint) per invocation. So [P4](#p4--sequential-insert)’s single-statement flush of N bins still costs N trigger calls, N savepoints, and N upserts contending on the same `dirty` row — the round-trips collapsed but the per-row server work did not. The `EXCEPTION` block itself is the expensive part, and it exists only to survive a missing `dirty` table, which cannot happen now that `schema.sql` creates it in the same batch.

**Proposal**

1. Statement-level trigger with `REFERENCING NEW TABLE AS ins` and one `INSERT INTO dirty … SELECT workspace_id, doc_id, max(clock) FROM ins GROUP BY 1, 2 ON CONFLICT … DO UPDATE`. One upsert per flush instead of N.
2. Drop the `EXCEPTION` wrapper, or keep it at statement level where it costs one savepoint per flush rather than one per row.
**Why this is not merely cosmetic.** Postgres caches at most 64 subtransaction ids per top-level transaction in shared memory. Past that the cache overflows and every other backend has to consult `pg_subtrans` for visibility checks, which degrades the whole cluster, not just this statement. One flush is one top-level transaction with one subtransaction per row, so a flush of more than 64 bins crosses that line — and one second of several people typing in one room is enough.

3. `dirty` is [step-8](./plan.md#8-step-dirty) scope, so land the trigger change with that step unless the flush cost shows up sooner. `flush_batch_keeps_array_order_and_marks_dirty` already asserts `dirty.clock` is the last seq, which the `max(clock)` form preserves.

**Landed** (2026-09-13): `venus_mark_dirty` is `FOR EACH STATEMENT` with `REFERENCING NEW TABLE AS ins`. `crdt_update` upserts `max(seq)` once per flush; snapshot upserts `max(clock)`. Postgres cannot attach one transition table to `INSERT OR UPDATE`, so snapshot is `crdt_snapshot_dirty_ins` + `crdt_snapshot_dirty_upd`. Inner `EXCEPTION` stays at statement grain (`undefined_table` / `undefined_column`) so a missing `dirty` cannot roll back persist. ROW body remains only for the one cutover boot that recreates leftover `FOR EACH ROW` triggers. `db::migrate` probes `tgnewtable IS NOT NULL` on all three names and skips the trigger batch when `n >= 3`. Tests: `flush_batch_keeps_array_order_and_marks_dirty` (`tgnewtable = ins`, one dirty row, no `jobs`); `flush_lands_when_dirty_table_is_dropped` (rename `dirty` in a tx); `persist_after_ws_upserts_dirty_clock_not_jobs`; `compact_if_needed_sql_rebuild_merges_trail` (snapshot clock); `migrate_serializes_two_hubs` (three STATEMENT triggers).

**Superseded in part** (2026-09-13, [P1](./logicals-and-performance.md#p1--new-table-copies-bin)): the **snapshot** triggers are dropped. Compact merges trail rows `crdt_update_dirty` already marked at that same clock, so `crdt_snapshot_dirty_ins` / `_upd` only copied the whole page `bin` into a transition table. Statement-level `max(seq)` on `crdt_update` is unchanged. The `migrate` probe now requires `crdt_update_dirty` STATEMENT **and** zero snapshot dirty triggers instead of counting to 3.

### P14 — Duplicate index on crdt_update

Third pass. `crdt_update` has `PRIMARY KEY (workspace_id, doc_id, seq)` and then `CREATE INDEX IF NOT EXISTS crdt_update_ws_doc_seq ON crdt_update (workspace_id, doc_id, seq)` — the same columns in the same order. The primary key already provides that index, so the second one is pure insert, WAL, and vacuum overhead on the table that takes every keystroke batch.

**Proposal**

`DROP INDEX IF EXISTS crdt_update_ws_doc_seq;` in `schema.sql` (idempotent, safe to run on every boot alongside the `CREATE TABLE IF NOT EXISTS` statements). Nothing references it by name.

**Landed** (2026-09-13): `schema.sql` drops `crdt_update_ws_doc_seq` on migrate. Test: `migrate_drops_duplicate_crdt_update_index` asserts it is gone after migrate and `crdt_update_pkey` remains.

### P15 — Persist buffer clones every bin per flush

Third pass. [L8](#l8--flush-is-not-cancellation-safe) made `Room::flush` clone the buffer and drain the prefix only after commit, which is the right shape — but `persist` is `Vec<Vec<u8>>`, so the clone copies every queued update’s bytes on every tick that has work. [P7](#p7--update-copied-per-persist-and-per-client) removed the fan-out copies and left this one.

At steady state the buffer holds about a second of edits and the copy is noise. It stops being noise exactly when flushes are failing: the buffer grows ([S10](#s10--persist-buffer-is-unbounded)) while every tick still copies all of it, so the total work across an outage is quadratic in the queued bins rather than linear. That interaction, not the steady-state cost, is why this is rated `Low → Med`.

**Proposal**

`persist: Mutex<Vec<Bytes>>`. The prefix clone becomes N refcount bumps, and the prefix-equality check after commit stays valid (`Bytes` compares by content). `db::flush_updates` needs the bins as something sqlx can bind to `bytea[]`, so either keep a `Vec<&[u8]>` at the call site or bind a slice of `Vec<u8>` built once. Do not remove the clone — it is what makes the flush cancellation-safe.

**Landed** (2026-09-13): `Room.persist` is a `PersistBuf` of `Vec<Bytes>` (S10 wrap). `apply_and_fanout` `push`es `Bytes::from(bin)`. `flush` still clones the prefix, then binds `Vec<&[u8]>` into `flush_updates` (`&[&[u8]]` → `bytea[]`) and drains only after commit (L8). Still SQL persist, not RAM merge ([L7](#l7--compact-from-ram-deletes-foreign-rows)). Test: `persist_flush_clone_shares_storage`; L2/L8 flush tests still pass.

### P16 — blob_post copies the body

Third pass. `blob_post` does `let bytes = body.to_vec();` and then only ever passes `&bytes`. Both consumers (`blob_hash`, `db::put_blob`) take `&[u8]`, and `axum::body::Bytes` already derefs to one, so the `to_vec` is a full extra copy of up to the 32 MiB `DefaultBodyLimit` per upload.

**Proposal**

Use `&body` directly. One-line change.

**Landed** (2026-09-13): `blob_post` passes `&body` to `blob_hash` and `put_blob`. No extra copy.

### P17 — Pool defaults are the only pool config

Third pass. `db::connect` is `PgPool::connect(database_url)`, so the whole process runs on sqlx defaults (max ~10 connections, 30s acquire timeout) shared by hydrate, one flush per room per second, compact’s long transaction ([P11](#p11--compact-holds-the-exclusive-lock-across-the-merge)), and blob reads that hold a connection while a 32 MiB `BYTEA` streams out. Nothing surfaces exhaustion either — an `acquire` timeout becomes a `Store` 500 with no distinct signal. `persist_interval` and `compact_after` are likewise hardcoded in `Config` (see [Not wired / cleanup](#not-wired--cleanup)), so an operator cannot trade flush frequency against pool pressure.

**Proposal**

1. `PgPoolOptions` with `max_connections`, `acquire_timeout`, and `min_connections` from env (`HUB_DB_MAX_CONNECTIONS`, …), defaulting to today’s behaviour so Compose is unchanged.
2. Add `HUB_PERSIST_INTERVAL_MS` and `HUB_COMPACT_AFTER` at the same time; [step-compose-hub](./plan.md#4-step-compose-hub) is the step that will want them.
3. Do not add PgBouncer thinking here — see [LiveSnapshot high-availability](../LiveSnapshot/high-availability.md) for why hub persist must not share a tiny `max_connections` with a worker stampede.

**Landed** (2026-09-13): required env, no sqlx defaults on the product path. `HUB_DB_MAX_CONNECTIONS` (`>= 1`), `HUB_DB_MIN_CONNECTIONS` (`>= 0`, `<= max`), `HUB_DB_ACQUIRE_TIMEOUT_SECS` (`>= 1`), `HUB_PERSIST_INTERVAL_MS` (`>= 1`), `HUB_COMPACT_AFTER` (`>= 1`). Missing or blank names the var. `db::connect_with` is `PgPoolOptions`; tests keep `db::connect` at 10 / 0 / 30s. Compose: `32` / `4` / `10` / `1000` / `32` (sized for 10 distinct cold exports + 10 blob GETs + 10 room flushes overlapping). Startup logs the knobs, never the DSN. Tests: `pool_and_timer_env_parse`, `pool_and_timer_env_reject_zero_and_garbage`, `required_env_names_the_var`, `connect_with_honors_pool_settings`.

**Extended** (2026-09-13, [P5](./logicals-and-performance.md#p5--cap-sized-flush-spills-at-default-work_mem)): `HUB_DB_WORK_MEM` joins the required set (Compose `16MB`) and `PoolSettings` carries a session `work_mem`, because a cap-sized flush spills to a temp file at the 4 MB default.

## Security surface (not authorization)

Authorization is deferred by design (see the header) and none of these are a substitute for it. They are listed separately because they are **unauthenticated writes and unbounded allocations** that a token check would not fix on its own, and because [step-compose-hub](./plan.md#4-step-compose-hub) is what first exposes the port.


| ID                                                                 | Issue                                                            | Where                                | Effect                                                                                                                                                                                                                       |
| ------------------------------------------------------------------ | ---------------------------------------------------------------- | ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [S1](#s1--unbounded-outbound-queue-per-client)                     | **done** — bounded `mpsc` (256); `try_send` Full/closed detaches | `Outbound`, `Room::attach`           | Was: unbounded queue; a stalled reader buffered every update forever.                                                                                                                                                        |
| [S2](#s2--unauthenticated-room-creation-is-the-amplifier)          | **done** (id shape + idle eviction) — UUID route; [P6](#p6--rooms-never-evicted) sheds unused rooms | `collaboration_get`, blob, export | Was: any random `workspace_id` took a lease, hydrated, and spawned an immortal persist task. Garbage ids are 400 before lease or SQL. A valid unknown id still opens a room; P6 evicts it when idle. |
| [S3](#s3--no-ws-message-cap-no-server-ping)                        | **done** — 512 KiB cap; ping 30s / pong wait 10s                 | `collaboration_get`, `handle_socket` | Was: 64 MiB default, no server ping; half-open sockets kept their `clients` entry. Landed 4 MiB, then 512 KiB.                                                                                                                |
| [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) | Blob write/delete unauthenticated and unmetered                  | `blob_put`, `blob_delete`            | 32 MiB accepted into any workspace id with no quota; `DELETE` destroys rows. Quota is a separate concern from auth.                                                                                                          |
| [S5](#s5--runtime-image-is-the-toolchain-image-as-root)            | **done** — `debian:bookworm-slim` + `USER venus`                 | `deploy/hub/Dockerfile`              | Was: `rust:1.90-bookworm` (compilers, ~1.5 GB) as uid 0.                                                                                                                                                                     |


Third pass adds S6–S10. S6 is the one that changes an existing fix’s claim: [S1](#s1--unbounded-outbound-queue-per-client) bounded the queue in **frames**, which is not a memory bound. [S10](#s10--persist-buffer-is-unbounded) is the same class on the persist side and was missed in the first write-up of this pass. `Sev` is read as **today → later**, per [How severity is rated](#how-severity-is-rated).


| ID | Sev | Effort | Issue | Where | Effect |
| --- | --- | --- | --- | --- | --- |
| [S6](#s6--outbound_cap-bounds-frames-not-bytes) | **done** (Med → High) | Small–med | `OUTBOUND_CAP` bounded frames, not bytes | `Room`, `handle_socket` | Was: 256 slots × up to 4 MiB (`WS_MAX_MESSAGE`) ~1 GiB retained per stalled reader. Now a 1 MiB queued-byte budget (two × 512 KiB frames); detach on over-budget the same as Full. |
| [S7](#s7--unauthenticated-export-is-a-read-amplifier) | **done** (Med → Med-high) crate slice | Small | Unauthenticated export is a read amplifier | `Hub::live_export`, `blob_get` | Was: every cache-miss export was a full SQL hydrate+encode, no single-flight; blob GET always loaded `BYTEA`. Now cold export is one SQL flight per `workspace_id`; blob GET/HEAD send `Cache-Control: public, max-age=31536000, immutable` and a quoted `ETag` of the hash, and answer **304** on `If-None-Match` after `blob_len`. Closing the route is auth. |
| [S8](#s8--sslmodedisable-is-hardcoded) | **done** (None → High) | Tiny | `sslmode=disable` was hardcoded | `config.rs` | Was: DSN from `POSTGRES_*` always `sslmode=disable`. Now `POSTGRES_SSLMODE` (default `disable`); `DATABASE_URL` is not rewritten. Startup logs `pg_sslmode`, not the DSN. |
| [S9](#s9--cors-allows-any-method-from-six-localhost-origins) | **done** (Low → Low) | Tiny | CORS allows any method from six localhost origins | `http.rs` `router` | Was: `AllowMethods::any()` + `AllowHeaders::any()`. Now `GET,HEAD,POST,DELETE` and `Content-Type` / `If-None-Match`. `HUB_CORS_ORIGINS` (unset → six localhost; empty → no layer; `*` rejected). Does not echo `Origin`. |
| [S10](#s10--persist-buffer-is-unbounded) | **done** (Med → High) | Medium | Persist buffer was unbounded | `Room.persist` | Was: [L2](#l2--flush-drops-bins) / [L8](#l8--flush-is-not-cancellation-safe) keep bins on SQL failure by design, with no cap. Now 8 MiB, drop oldest, keep newest. |


### S1 — Unbounded outbound queue per client — **done**

`type Outbound = mpsc::UnboundedSender<Vec<u8>>` with `mpsc::unbounded_channel()`. Nothing bounds the per-client backlog, so one stalled reader (throttled tab, dead TCP, deliberate slow-read) turns every broadcast into retained hub memory. With [P7](#p7--update-copied-per-persist-and-per-client) it retains a private copy per client.

**Proposal**

Bounded `mpsc::channel(N)` (N in the low hundreds). On `try_send` `Full`, treat that client as lagged: `detach` and drop the socket. The client reconnects and gets a fresh Step2, which is exactly the recovery the protocol already has — do not block the apply queue waiting for a slow reader.

**Landed** (2026-09-12): `OUTBOUND_CAP = 256`, `mpsc::channel`. `send_to` / `broadcast_except` use `try_send`; Full or closed removes the client. `handle_socket` already breaks on `rx.recv() == None`. Apply never `.await`s a slow reader. Test: `lagged_client_is_detached_without_blocking_apply`. WS ping/size caps are [S3](#s3--no-ws-message-cap-no-server-ping).

### S2 — Unauthenticated room creation is the amplifier

A WS upgrade to `/collaboration/<anything>` reaches `get_room`, which acquires a lease, hydrates, inserts into `rooms`, and spawns a persist task that never exits ([P6](#p6--rooms-never-evicted)). Nothing evicts and nothing validates the id, so a loop over random ids grows hub RAM, task count, and `workspace_lease` rows without bound, and each of those rooms keeps heartbeating.

**Proposal**

1. Land [P6](#p6--rooms-never-evicted) idle eviction — it is the only backstop that works before the auth module exists.
2. Validate the `workspace_id` shape at the route (length cap, charset) and reject early, before any lease or SQL.
3. Optional stopgap: refuse `get_room` for a workspace with no `crdt_snapshot`/`crdt_update` row unless creation is explicit, so unknown ids cost one `SELECT` instead of a room.
4. When the auth module lands, the check goes in front of `get_room`, not inside `handle_socket`.

**Landed** (2026-09-13): id-shape slice. `workspace_id_ok` is a hyphenated UUID on every `{workspace_id}` route **before** `get_room` / SQL. 400 `{ "error": "invalid workspace_id" }` — the raw id is not echoed. Does not refuse unknown-but-well-shaped ids. Tests: `http::s2::workspace_id_shape`; `bad_workspace_id_is_400_and_does_not_acquire` (no lease); `bad_workspace_id_blob_and_export_are_400` (no blob row). Idle eviction is [P6](#p6--rooms-never-evicted).

### S3 — No WS message cap, no server ping — **done**

The upgrade uses `WebSocketUpgrade` defaults, so a single frame is bounded only by the library default, not by the 32 MiB `DefaultBodyLimit` (that layer is HTTP-body only). There is also no server-initiated ping and no read timeout, so a half-open socket is never reaped and keeps its `clients` entry alive.

**Proposal**

`ws.max_message_size(…)` / `max_frame_size(…)` sized to a realistic update batch (a few MiB), plus a ping every ~30s in `handle_socket` with a missed-pong deadline that `detach`es. Cheap, and it bounds [S1](#s1--unbounded-outbound-queue-per-client)’s worst case too.

**Landed** (2026-09-12): `WS_MAX_MESSAGE` 4 MiB on both `max_message_size` and `max_frame_size`. `handle_socket` pings every 30s; no Pong within 10s detaches (loop breaks, existing `detach`). `AppState` carries the three knobs so tests can use 1 KiB / 50ms / 80ms. Tests: `ws_oversized_binary_is_closed`, `ws_missed_pong_closes_socket`.

**Later** (2026-09-13): `WS_MAX_MESSAGE` is **512 KiB**; outbound budget is two of those (`OUTBOUND_BYTES` = 1 MiB). Compile-time `assert`s keep both ≥ one max frame.

### S4 — Blob write and delete are unauthenticated and unmetered

`POST /api/blobs/:workspace_id` accepts up to 32 MiB into any id; `DELETE /api/blobs/:workspace_id/:hash` removes rows. The auth module will gate the caller, but nothing today caps **how much** an authorized caller may store either.

**Proposal**

Keep the routes as they are for M3.0 and record the quota as auth-module scope: per-workspace byte total and object count checked in the same transaction as the `INSERT`. Note the shape here so it is not forgotten.

**Correction** (third pass): an earlier version of this section said the hash is client-supplied and should be verified server-side. It is not. `blob_post` computes `blob_hash(&bytes)` itself and never reads a hash from the request; the path hash in `blob_get` / `blob_head` / `blob_delete` is only a lookup key. Content addressing already holds, so there is nothing to verify and **no fix is owed here**. What remains is quota (auth module). Cross-origin `DELETE` from a listed origin is still [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) until auth; [S9](#s9--cors-allows-any-method-from-six-localhost-origins) only stopped `any()` methods and made the origin list env-driven.

### S5 — Runtime image is the toolchain image, as root — **done**

`deploy/hub/Dockerfile`’s final stage is `rust:1.90-bookworm` with no `USER`, so the deployed container carries cargo, rustc, and a full build toolchain and runs privileged inside the namespace.

**Proposal**

Two-stage: build on `rust:1.90-bookworm`, copy the binary into `debian:bookworm-slim` (or `gcr.io/distroless/cc` — rustls means no OpenSSL runtime dep), add a non-root `USER`, and keep only the binary plus CA certs. This belongs to [step-compose-hub](./plan.md#4-step-compose-hub) but it is the image that step will ship.

**Landed** (2026-09-12): runtime `debian:bookworm-slim` + `ca-certificates` (rustls, no libssl). `USER venus` (uid/gid 65532). Compose healthcheck stays `bash /dev/tcp` — slim has bash; distroless would not. Build stage is still `rust:1.90-bookworm`.

### S6 — OUTBOUND_CAP bounds frames, not bytes

Third pass. [S1](#s1--unbounded-outbound-queue-per-client) replaced the unbounded channel with `mpsc::channel(OUTBOUND_CAP)` where `OUTBOUND_CAP = 256`, and that fixed the unbounded case. It did not produce a memory bound: the cap counts **frames**, and [S3](#s3--no-ws-message-cap-no-server-ping) sets `WS_MAX_MESSAGE` to 4 MiB, so one stalled reader can hold ~1 GiB of `Bytes` alive before `try_send` returns Full. Multiple slow readers on the same room share storage for the same frame (that is [P7](#p7--update-copied-per-persist-and-per-client)’s refcount), so the worst case is per distinct frame rather than per client — but a single client sending large updates is enough to reach it, and the [S3](#s3--no-ws-message-cap-no-server-ping) ping deadline only reaps *half-open* sockets, not a reader that is merely slow.

Ordinary editing produces small updates, which is why this has not been seen. It is still an attacker-controlled multiplier, and it is exactly what S1 was meant to close.

**Proposal**

1. Track queued bytes per client alongside the sender and detach when the sum crosses a budget (a few MiB — the point is to be far below `WS_MAX_MESSAGE × OUTBOUND_CAP`). Same recovery as S1: drop the socket, the client reconnects and gets a fresh Step2.
2. A cheaper first cut: lower `OUTBOUND_CAP` so `OUTBOUND_CAP × WS_MAX_MESSAGE` is a number worth writing down, and record that product in the constant’s doc comment so the two knobs stop being independent.
3. Do not make the apply path `await` a slow reader — S1’s `try_send` contract stands.
4. Test: extend `lagged_client_is_detached_without_blocking_apply` with large updates and assert detach happens on the byte budget rather than after 256 frames.

**Landed** (2026-09-13): `OUTBOUND_BYTES = 8 MiB` per client (far below `256 × 4 MiB`). `Outbound` reserves queued bytes then `try_send`; over budget is Full (detach), never an apply `.await`. `OutboundRx::recv` releases the reservation. Slot cap 256 stays (S1). Test: `lagged_client_detaches_on_byte_budget` (budget = 2 × frame, third send detaches; apply still fans out to a caught-up peer).

**Later** (2026-09-13): `OUTBOUND_BYTES` is **1 MiB** (two × 512 KiB `WS_MAX_MESSAGE`).

### S7 — Unauthenticated export is a read amplifier

Third pass. `GET /api/block/:workspace_id/export` reaches `Hub::live_export`, which encodes RAM if this hub owns the room and otherwise calls `db::default_page_export` — a full snapshot + trail read plus a full `encode_v1` — per request. There is no cache, no single-flight (unlike [P9](#p9--cold-room-hydrate-stampede)’s `get_room`), no rate limit, and no auth. Unlike [S2](#s2--unauthenticated-room-creation-is-the-amplifier) it leaves no lease or room behind, so it is not unbounded growth; it is unbounded *work* per request, on both the hub CPU and the pool ([P17](#p17--pool-defaults-are-the-only-pool-config)).

Listed here rather than under Performance because the trigger is untrusted: a token check in front of the route removes the amplifier, and nothing in the crate does today.

**Proposal**

1. Route goes behind the auth module with `get_room` and the blob routes. That is the real fix.
2. Independently useful now: single-flight `live_export` per `workspace_id` on the cold path, reusing [P9](#p9--cold-room-hydrate-stampede)’s `Hub.hydrating` shape, so a refresh storm costs one read.
3. Blob reads are the same class and cheaper to fix: blobs are content-addressed and immutable, so `blob_get` can send `Cache-Control: public, max-age=31536000, immutable` and an `ETag` of the hash, and answer `304` on `If-None-Match`. That removes most repeat reads without touching auth.

**Landed** (2026-09-13): crate slice (proposal 2 and 3). `Hub.exporting` single-flight on the cold path (same waiter shape as [P9](#p9--cold-room-hydrate-stampede)’s `hydrating`, but waiters want bytes, not `Arc<Room>`). Hot path is still RAM `encode_live` with no flight — concurrent RAM encodes are OK. Leader re-checks `live_room` after becoming leader so a room that appeared during the wait is not a second SQL. `export_sql_attempts` counts SQL hydrates. Blob GET/HEAD: `Cache-Control: public, max-age=31536000, immutable`, quoted `ETag` of the path hash, **304** on `If-None-Match` (`*`, weak tags, comma list) after `blob_len` so a deleted hash is 404 not 304. Tests: `http::s7::etag_matches_quoted_hash_and_star`; `blob_put_get_head_len_without_loading_body` (cache headers + 304 empty body); `concurrent_cold_export_is_single_flight` (8 waiters, one SQL; a second wiki is a second attempt). Closing the route (proposal 1) stays the auth module.

### S8 — sslmode=disable is hardcoded

Third pass. When `DATABASE_URL` is unset, `database_url_from_env` builds the DSN and appends `?sslmode=disable` unconditionally. That is correct for Compose, where Postgres is on a private network and not exposed to the browser (plan step 2 **Do not**). It is wrong the moment the hub talks to managed Postgres from [Kubernetes](../../devops/kubernetes.md): credentials and every CRDT byte go in clear text, and the only way to opt out is to abandon the `POSTGRES_*` path entirely and hand-build a full `DATABASE_URL`. A silent downgrade with no log line is the problem, not the default itself.

**Proposal**

`POSTGRES_SSLMODE`, defaulting to `disable` so Compose is unchanged, and log the effective mode at startup next to `listen` and `owner`. Do not log the DSN — it carries the password.

**Landed** (2026-09-12): `POSTGRES_SSLMODE` is an allowlisted libpq token (`disable`, `allow`, `prefer`, `require`, `verify-ca`, `verify-full`); empty/unset is `disable`. Unknown values fail startup so a typo cannot inject query params. `DATABASE_URL` is still honored verbatim; `Config.pg_sslmode` is that query’s `sslmode` or `unset`. `main` logs `pg_sslmode` next to `listen` / `owner` and never the DSN. Tests: `parse_pg_sslmode_allowlist`, `build_postgres_url_uses_sslmode`, `sslmode_from_dsn_reads_query_and_leaves_url_alone`. Compose stays `disable` without a new env var.

### S9 — CORS allows any method from six localhost origins

Third pass. `router` allows six hardcoded `localhost` / `127.0.0.1` dev origins with `AllowMethods::any()` and `AllowHeaders::any()`. No `allow_credentials`, so this does not leak an authenticated response. But allowing every method is what makes a cross-origin `DELETE /api/blobs/:workspace_id/:hash` pass preflight, so any page the user visits on those ports — another dev server, a stale app — can destroy blob rows while [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) leaves the route open. The dev origin list also ships in the product image, where nginx proxies same-origin and no CORS is needed at all.

**Proposal**

1. List the methods actually used (`GET`, `HEAD`, `POST`, `DELETE` today) and the headers actually used, rather than `any()`. That alone does not stop the `DELETE` — closing it is auth — but it stops the list from being wider than the API.
2. Make the origin list env-driven (`HUB_CORS_ORIGINS`), defaulting to the dev list, so a deployment can ship an empty list and rely on same-origin nginx.
3. Do not echo the request origin. The current code does not, and [What already matches the plan](#what-already-matches-the-plan) records that as a property to keep.

**Landed** (2026-09-13): methods `GET`/`HEAD`/`POST`/`DELETE`; headers `Content-Type` and `If-None-Match`. `HUB_CORS_ORIGINS` comma-separated `http(s)://host[:port]`; unset → the six localhost origins; empty → no CORS layer. `*` and paths are rejected. Request `Origin` is not echoed. Compose sets the six so Playwright can preflight `hub:3000`. Tests: `cors_origins_empty_star_path_and_default`; `cors_preflight_is_allowlist_not_any_method`; `cors_empty_list_has_no_allow_origin`. Closing `DELETE` is still [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) / auth.

**Scope, so this is not over-rated.** A page on an arbitrary origin cannot use this: it is not in the allow list, so its `DELETE` preflight fails. The precondition is a page **served from** `localhost:5173`, `:5174`, or `:8080` — realistically another project’s dev server on a port this one also uses. Note separately that CORS never blocked the *request*, only reading the response, so a cross-origin blob `POST` with a safelisted content type already reaches the handler from any origin regardless of this config. That is [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered)’s open route, not a CORS bug, and no CORS change fixes it.

### S10 — Persist buffer is unbounded

Third pass, added after the first write-up of this pass. `Room.persist` is a `Vec<Bytes>` that `apply_and_fanout` pushes every accepted update onto, and the only thing that shrinks it is a successful `db::flush_updates`. Keeping bins on SQL failure is deliberate and correct ([L2](#l2--flush-drops-bins), [L8](#l8--flush-is-not-cancellation-safe)) — losing them would be data loss — but nothing caps the result. While Postgres is unreachable and clients keep editing, the buffer grows without limit. [P15](#p15--persist-buffer-clones-every-bin-per-flush) made the per-tick clone a refcount bump, so an outage is no longer quadratic copy work; the allocation itself is still unbounded.

This is the same class as [S1](#s1--unbounded-outbound-queue-per-client) and [S6](#s6--outbound_cap-bounds-frames-not-bytes) — an unbounded allocation driven by untrusted input — and it is the one the second pass did not touch. With `WS_MAX_MESSAGE` at 4 MiB a writer does not even need a slow reader to grow it.

**Proposal**

1. Cap the buffer in **bytes** per room. On overflow the choice is data loss either way, so make it explicit and loud rather than an OOM: `error!` with the workspace and the dropped byte count, and drop the **oldest** bins (the newest state is what clients will resend on reconnect, and y-protocols recovery is the plan’s answer for a lost prefix — see [plan step 3](./plan.md#3-step-ws) *Crash, client resend*).
2. Consider refusing new applies for that room instead of dropping — detach clients so they hold their own updates and resend. That keeps the hub honest at the cost of an outage being visible to users, which is arguably right.
3. Do not silently drop, and do not go back to dropping bins on SQL error ([L2](#l2--flush-drops-bins)).
4. Decide (1) vs (2) explicitly; it is a product call about what an outage looks like, not a lock detail. Until then the cap alone stops the OOM.
5. Test: a room on a closed pool, apply until the cap, assert the buffer stops growing and that the newest update still hydrates after the pool comes back.

**Landed** (2026-09-13): **drop oldest, loudly** — not refuse-applies. `PERSIST_BYTES = 8 MiB` (≥ `WS_MAX_MESSAGE`). `PersistBuf` drops from in front of the queued suffix (`error!` workspace, dropped bytes/bins, cap); the L8 in-flight prefix is not dropped, so a successful INSERT cannot mismatch and duplicate on retry. Overflow during a flush can temporarily exceed the cap by that prefix; after a SQL error `in_flight = 0` and oldest can drop. Do not drop on SQL error ([L2](#l2--flush-drops-bins)). Test: `persist_cap_drops_oldest_keeps_newest` (tiny cap, closed pool, newest hydrates, `k0` does not).

## Gaps vs plan

Not extra scenarios. Work items that shipped untested, or wording vs actual tests.

### Covered DoD


| Step | Scenario              | How it was proved                                                      |
| ---- | --------------------- | ---------------------------------------------------------------------- |
| 1    | Map complete          | `recon` greps Chosen backend (M3.0)                                    |
| 1    | Spike syncs           | In-process two `Room` clients, no Postgres                             |
| 2    | Round-trip            | `push_update` BYTEA → `get_doc` → `spike.k=v`                          |
| 2    | Restart store         | New pool, value still hydrates                                         |
| 3    | A→B / Marks / Persist | `tests/ws.rs` tokio-tungstenite + Postgres; `Hub::shutdown`, new `Hub` |
| 3    | Late joiner           | `late_joiner_step2_has_spike_after_a_writes`                           |
| 3    | Lease refuse (crate)  | `second_hub_ws_is_503_while_lease_held`                                |


### Coded but not in named tests


| Item                 | Plan                      | Status                                        | Proposal                                                                |
| -------------------- | ------------------------- | --------------------------------------------- | ----------------------------------------------------------------------- |
| Node `Y.Doc` over WS | Step 3 scenario 1 wording | Tests send `yjs@13.6.32` fixtures via y-octo. | Keep fixtures. Do not add a Node test harness. Wire is still update v1. |


Compact, blobs, late joiner, and crate lease-503 are in [Covered DoD](#covered-dod). Do not add a Node harness as a new plan scenario.

### Not wired / cleanup

Second pass. None of these are plan scenarios; they are loose ends visible in the crate.


| Item                                    | Where              | Note                                                                                                                                                                                                                                                                                                              |
| --------------------------------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Awareness is never fanned out           | `Room::handle_msg` | `AwarenessQuery` gets an empty reply and inbound `Awareness(_)` is dropped, so no presence or cursors. Handshake sends `encode_awareness_empty` only. Correct for M3.0 (`apps/web` does not use awareness), but it is a wire feature the client will expect later — decide it explicitly rather than by omission. |
| `chrono` is unused                      | `Cargo.toml`       | **done** — dropped the crate dep and the sqlx `chrono` feature. Timestamps stay `now()` in SQL. |
| `abort_persist_no_flush` is test-only   | `Hub`              | **done** — `#[doc(hidden)]`. Stays `pub` so `tests/ws.rs` can model `kill -9` (integration tests are a separate crate; `#[cfg(test)]` on the lib would hide it from them). |
| `compact_after = 0` compacts every tick | `config.rs`        | **done** — `validate_compact_after` requires `>= 1`. `HUB_COMPACT_AFTER` is required at start ([P17](#p17--pool-defaults-are-the-only-pool-config)). `compact_if_needed` skips when `threshold < 1`. |
| `venus_doc_lock_key` 64-bit fold | `db.rs` / `schema.sql` | Two UUIDs XOR-fold to one `bigint` advisory key. Distinct `(workspace_id, doc_id)` can still collide (64-bit pigeonhole). Not `hashtext`. Harmless for correctness; latency graphs can still couple unrelated wikis. |


### Ahead of the board (not step 9 close-out)

`docker-compose.yml` already has `postgres` + `hub` + `web` and an HA profile `hub-b`. Schema has `workspace_lease` and the STATEMENT dirty triggers.

- Step 4 Compose restart-without-volume evidence is **done** (`pnpm compose:dod`).
- Step 7 second process refuses and sheds on steal ([L4](#l4--lease-steal-ram-stays)); SIGTERM join/`drop_all` is [L5](#l5--drain-race); idle eviction is [P6](#p6--rooms-never-evicted). **done** (`pnpm compose:ha`).
- Step 8 **done** — dirty observer contract + [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction) statement-level trigger.

## Fix order

Do in the crate, still without reopening steps 1–3 as board work. Auth stays out.

1. [L1](#l1--hydrate-vs-compact) + compact store test. **done**
2. [L2](#l2--flush-drops-bins) / [P4](#p4--sequential-insert) (one flush statement + put-back). **done**
3. [L3](#l3--503-lies) typed `GetRoomError`. **done**
4. [L6](#l6--ghost-client) + [P1](#p1--attach-holds-clients-during-encode) (encode, then insert). **done**
5. [P2](#p2--export-holds-rooms-lock) (clone `Arc`, drop map lock). **done**
6. [P5](#p5--head-blob-loads-body) (`octet_length`). **done**
7. [P3](#p3--compact-every-tick) `trail_len` skip. **done** (RAM encode reverted; see [L7](#l7--compact-from-ram-deletes-foreign-rows)).
8. [L5](#l5--drain-race) join persist tasks. **done**. [L4](#l4--lease-steal-ram-stays) / [P6](#p6--rooms-never-evicted) shed. **done** (step 7).

Second pass. Next up are the small HA-correctness items, not regressions.

1. [L7](#l7--compact-from-ram-deletes-foreign-rows) drop P3 live encode; compact always SQL rebuild. **done**
2. [L8](#l8--flush-is-not-cancellation-safe) drain the persist buffer only after commit, with [L14](#l14--notify_waiters-can-lose-the-stop-wakeup) `notify_one`. **done**
3. [S1](#s1--unbounded-outbound-queue-per-client) bounded outbound, detach on Full. **done**. [S3](#s3--no-ws-message-cap-no-server-ping) 4 MiB cap + ping/pong deadline. **done**.
4. [L10](#l10--heartbeat_many-aborts-on-the-first-sql-error) / [L11](#l11--lease-ttl-is-only-2-heartbeats) / [L12](#l12--serve-error-skips-shutdown) — three small HA-correctness fixes, one patch, before step 7 depends on them. **done**
5. [L9](#l9--non-upgrade-get-is-400-not-health-json) decision 1: GET without upgrade is health JSON. **done**
6. [L13](#l13--one-corrupt-trail-bin-bricks-a-room) skip-and-log a bad trail bin. **done**. [L15](#l15--malformed-frame-truncates-a-batch-silently) log truncated frames. **done**.
7. [P7](#p7--update-copied-per-persist-and-per-client) `Bytes` fan-out. **done**. [P8](#p8--persist_tasks-grows-forever) / [P9](#p9--cold-room-hydrate-stampede) **done** (handle on `Room`; single-flight per `workspace_id`). Idle eviction is [P6](#p6--rooms-never-evicted) **done**. ([P4](#p4--sequential-insert) batch `INSERT` is **done**.)
8. [S5](#s5--runtime-image-is-the-toolchain-image-as-root) slim non-root image. **done**. [S2](#s2--unauthenticated-room-creation-is-the-amplifier) id shape **done**; eviction [P6](#p6--rooms-never-evicted) **done**. Remaining S4 quota: [Still to fix](#still-to-fix).

Third pass. Nothing here is a regression; the ordering is by blast radius, not by pass.

1. [L16](#l16--hydrate-failure-squats-the-lease) drop the lease when hydrate fails, with [L17](#l17--corrupt-snapshot-bin-is-still-fail-closed) snapshot skip-and-log. One patch — L17 is the reachable trigger for L16 and neither is complete alone. **done**
2. [P10](#p10--mutexdoc-serializes-reads-against-apply) `RwLock<Doc>`. **done**
3. Cheap and independent, one patch: [P14](#p14--duplicate-index-on-crdt_update) drop the duplicate index, [P16](#p16--blob_post-copies-the-body) drop `body.to_vec()`, [P12](#p12--heartbeat-is-n-round-trips-and-bursts) one-statement heartbeat + `Delay`, [L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist) frame before persist. **done**
4. [S6](#s6--outbound_cap-bounds-frames-not-bytes) byte budget on the outbound queue — it is the missing half of [S1](#s1--unbounded-outbound-queue-per-client) — with [S10](#s10--persist-buffer-is-unbounded)’s persist cap. Same class, same patch; S10 needs the drop-vs-refuse call first. **done** (drop oldest, not refuse).
5. [L18](#l18--zero-ws_ping-panics-interval_at) / [L19](#l19--shutdown-has-no-stopping-gate) / [L20](#l20--migrate-recreates-triggers-with-no-lock) small latent-correctness items; L20 before a second hub runs in Compose. **done**
6. [S8](#s8--sslmodedisable-is-hardcoded) `POSTGRES_SSLMODE`. **done**. [P17](#p17--pool-defaults-are-the-only-pool-config) pool / persist / compact env. **done**. [S9](#s9--cors-allows-any-method-from-six-localhost-origins) narrow CORS. **done**.
7. [P11](#p11--compact-holds-the-exclusive-lock-across-the-merge) merge outside the exclusive lock. **done**. [P15](#p15--persist-buffer-clones-every-bin-per-flush) `Bytes` persist buffer. **done**
8. [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction) statement-level dirty trigger — **done** with [step-8](./plan.md#8-step-dirty). [S7](#s7--unauthenticated-export-is-a-read-amplifier) export single-flight and blob cache headers **done**. Closing the route is auth.

## Still to fix

Every first- and second-pass fix is landed, plus third-pass [L16](#l16--hydrate-failure-squats-the-lease)–[L21](#l21--fan-out-is-lost-if-frame-encode-fails-after-persist) / [S6](#s6--outbound_cap-bounds-frames-not-bytes) / [S8](#s8--sslmodedisable-is-hardcoded) / [S10](#s10--persist-buffer-is-unbounded) / [P10](#p10--mutexdoc-serializes-reads-against-apply)–[P16](#p16--blob_post-copies-the-body) / [S7](#s7--unauthenticated-export-is-a-read-amplifier) crate slice / [P17](#p17--pool-defaults-are-the-only-pool-config) / [S9](#s9--cors-allows-any-method-from-six-localhost-origins). [S2](#s2--unauthenticated-room-creation-is-the-amplifier) id shape, [P6](#p6--rooms-never-evicted) idle eviction, [L4](#l4--lease-steal-ram-stays) shed, and the cleanup row (`chrono`, `compact_after < 1`, `abort_persist_no_flush`) are **done**. Nothing is left in this crate except auth and [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) quota. Closing [S7](#s7--unauthenticated-export-is-a-read-amplifier)’s route is auth, not this crate. Do not reopen steps 1–8. Do not invent plan scenarios. Auth stays a separate module. Board next is [step-verify](./plan.md#9-step-verify).

### This crate (Compose port is the product path)

Crate config / CORS / lease-shed / dirty work is **done**. Remaining items are auth ([S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) quota, [S7](#s7--unauthenticated-export-is-a-read-amplifier) closing the route).

### [step-ha-owner](./plan.md#7-step-ha-owner) (step 7)

**done.** Same shed path. Two processes: `pnpm compose:ha`.

| ID | Fix |
| --- | --- |
| [L4](#l4--lease-steal-ram-stays) | **done** — heartbeat miss sheds the RAM room (`request_stop`, flush, `rooms.remove`, clients close). Does not `try_acquire` from shed. Old hub stops `INSERT`. |
| [P6](#p6--rooms-never-evicted) | **done** — idle eviction: no clients, empty persist buffer, idle ≥ TTL → same shed + `lease.drop_one`. |
| [S2](#s2--unauthenticated-room-creation-is-the-amplifier) remainder | **done** — P6 is the unbounded-room backstop until auth exists. |

### Auth module (not this crate)

- Token check in front of `get_room`, export, and blob routes, not inside `handle_socket`.
- [S7](#s7--unauthenticated-export-is-a-read-amplifier) closing `GET /api/block/:id/export` (crate single-flight / blob cache headers are **done**).
- [S4](#s4--blob-write-and-delete-are-unauthenticated-and-unmetered) quota: per-workspace byte total and object count in the same transaction as the `INSERT`.

### Later plan steps (not this review)

- [Step 4](./plan.md#4-step-compose-hub): **done** — Compose restart-without-volume evidence (`pnpm compose:dod`). [P17](#p17--pool-defaults-are-the-only-pool-config), [S8](#s8--sslmodedisable-is-hardcoded), and [S9](#s9--cors-allows-any-method-from-six-localhost-origins) knobs are already in Compose.
- [Step 8](./plan.md#8-step-dirty): **done** — dirty observer contract; [P13](#p13--dirty-trigger-is-per-row-with-a-subtransaction) statement-level trigger.

