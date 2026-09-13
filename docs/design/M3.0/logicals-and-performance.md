# M3.0 — logicals and performance (dirty / persist)

Audit of [step-dirty](./plan.md#8-step-dirty) after it closed: **logical errors** and **performance** in the persist → `dirty` path. Security for [step-compose-hub](./plan.md#4-step-compose-hub) is not this file (Compose bind / hub role / image secrets landed in `docker-compose.yml`).

**This does not reopen step 8.** Named DoD (upsert one row, second write moves `clock`, persist if `dirty` is missing) stays green. Do not invent extra plan scenarios from this file. Hub still must not write `jobs`, poll export, or pause persist.

Crate map: [hub files](../components/hub/files.md). Contract: [high-availability.md](./high-availability.md) (clock monotonic; trigger must not stall persist). Board: [M3.0.state.yaml](./M3.0.state.yaml). Earlier crate review: [steps-1-3-review.md](./steps-1-3-review.md) ([P13](./steps-1-3-review.md#p13--dirty-trigger-is-per-row-with-a-subtransaction) **done**).

**Date:** 2026-09-13. Source: `schema.sql` `venus_mark_dirty` + `db::compact` / `flush_updates`. **D1–D4 landed** (clock `GREATEST`; `RAISE WARNING`; observer ignores `clock <= last_flushed`; `search_path = public`). **P1 snapshot half landed:** the `crdt_snapshot` dirty triggers are **gone**; only `crdt_update_dirty` marks dirty. Its flush-side copy is now measured (`p1_flush_transition_table_cost`) and is **not** negligible at the cap — see [P1](#p1--new-table-copies-bin). **P2 / P3 closed as by-design** (the `EXCEPTION` block is the persist fail-safe; the ROW branch is cutover compatibility). Measuring P1 turned up **[P5](#p5--cap-sized-flush-spills-at-default-work_mem)**, a real spill on the paste path that was nothing to do with dirty; that one **landed** (`HUB_DB_WORK_MEM`). Step 8 stays closed.

## Status


| | |
| --- | --- |
| Named DoD | **Upsert** and **Persist if dirty missing** pass |
| High logic open | — |
| Medium logic open | — |
| Low logic open | — |
| Performance open | — ([P5](#p5--cap-sized-flush-spills-at-default-work_mem) landed: `HUB_DB_WORK_MEM` per session). [P1](#p1--new-table-copies-bin) flush half measured — 4.2 ms of a cap flush, 54 µs typing; CTE not justified. [P2](#p2--one-savepoint-per-flush) won’t fix — the block **is** the persist fail-safe. [P3](#p3--row-fallback-on-cutover) won’t fix yet — cutover compatibility. |
| Auth / jobs | Out of scope. Observer / `jobs` is M3. |


## How to read severity

Same two-axis as the crate review: **today → later**. Today is one wiki, one persist task per room, one live owner. Later is a large snapshot, a lease-steal window, or a second writer inserting `crdt_update` while compact holds the advisory lock (flush does not take that lock).

## Logical errors


| ID | Sev | Effort | Issue | Where | Effect |
| --- | --- | --- | --- | --- | --- |
| [D1](#d1--dirtyclock-is-not-monotonic) | **done** | Tiny | `dirty.clock` is not monotonic | `venus_mark_dirty` | `GREATEST` + `WHERE … IS DISTINCT FROM`; test `compact_snapshot_must_not_rewind_dirty_clock` |
| [D2](#d2--missing-dirty-is-silent) | **done** | Tiny | Missing `dirty` is silent | `EXCEPTION` in `venus_mark_dirty` | `RAISE WARNING` with `SQLERRM`; persist still does not roll back |
| [D3](#d3--compact-re-inserts-a-processed-row) | **done** (P1) | Tiny | Compact re-inserts a processed row | snapshot `_ins` / `_upd` | Snapshot triggers **dropped**; compact cannot resurrect a GCed row. Observer still ignores `clock <= last_flushed` |
| [D4](#d4--trigger-search_path-follows-user) | **done** | Tiny | Trigger `search_path` follows `$user` | `CREATE FUNCTION venus_mark_dirty` | `SET search_path = public`; `INSERT INTO public.dirty` |


### D1 — dirty.clock is not monotonic

HA: flush appends `crdt_update`; `clock` is monotonic (`seq` or snapshot clock). The trigger does `SET clock = EXCLUDED.clock` with no `GREATEST`.

Compact writes `crdt_snapshot.clock = max_seq` of the merged trail, then `DELETE seq <= max_seq`. Flush does **not** take `venus_doc_lock_key`. After compact’s `now_max` check in `commit_compact`, a concurrent `INSERT` into `crdt_update` can set `dirty.clock` to a newer seq; the snapshot statement trigger then writes the **old** `max_seq`.

Today’s product path is one persist task per room (`flush` then `compact_if_needed`), so the happy path does not hit this. It is reachable on lease steal (old hub still flushing), a foreign SQL writer ([L7](./steps-1-3-review.md#l7--compact-from-ram-deletes-foreign-rows) already admits foreign trail rows), or tests that interleave `flush_updates` with compact.

**Proposal**

```sql
ON CONFLICT (workspace_id, doc_id) DO UPDATE
  SET clock = GREATEST(dirty.clock, EXCLUDED.clock)
  WHERE dirty.clock IS DISTINCT FROM GREATEST(dirty.clock, EXCLUDED.clock);
```

Same in the ROW branch. Test: after `now_max` matches, insert a newer `crdt_update` before the snapshot UPSERT (compact test hook already exists for trail-moved); `dirty.clock` must stay the newer seq.

Do not pause persist. Do not take the compact advisory lock on flush.

**Landed** (2026-09-13): `GREATEST(dirty.clock, EXCLUDED.clock)` with `WHERE dirty.clock IS DISTINCT FROM GREATEST(...)` on the STATEMENT branch and the ROW fallback. Test `compact_snapshot_must_not_rewind_dirty_clock` inserts a newer `crdt_update` after `now_max` matches (`compact_after_now_max`); `dirty.clock` stays the newer seq. P4 equal-clock WAL is the same `WHERE`.

The **snapshot** rewind path is now structurally gone — [P1](#p1--new-table-copies-bin) dropped that trigger, so `crdt_snapshot` never writes `dirty`. `GREATEST` stays for out-of-order **flush** statements (lease steal, foreign SQL writer).

### D2 — missing dirty is silent

The inner `BEGIN … EXCEPTION` swallows `undefined_table` and `undefined_column` with `NULL`. Persist must not roll back (plan). HA also said **log**. There is no `RAISE NOTICE` / `RAISE WARNING`. `undefined_column` also hides a `dirty` table whose shape drifted, not only a dropped table.

**Proposal**

Keep the fail-safe. `RAISE WARNING` with `SQLERRM` (and workspace/doc if `TG_LEVEL = 'ROW'` / from `ins`). Do not `RAISE EXCEPTION` — that would undo persist.

**Landed** (2026-09-13): `WHEN undefined_table OR undefined_column` then `RAISE WARNING`. Persist still commits (`flush_lands_when_dirty_table_is_dropped`).

### D3 — compact re-inserts a processed row

Snapshot replace always upserts `dirty`. That is correct when compact is the first persist of a page. After an M3 observer deletes the row (pin done), the next compact **INSERT**s it again at the same `clock`. Duplicate pin unless the observer keys on `clock` vs `last_flushed`.

**Proposal**

Leave the trigger as “SQL moved.” Document for M3: ignore `dirty` rows with `clock <= last_flushed`. Optional later: `WHERE dirty.clock IS DISTINCT FROM EXCLUDED.clock` so a no-op compact does not recreate a deleted row (that would skip D3 only when the row still exists with the same clock; a deleted row would still insert). Prefer the observer rule.

**Landed** (2026-09-13): **fixed by [P1](#p1--new-table-copies-bin)** — compact has no dirty trigger, so it cannot recreate a GCed row at all. `snapshot_write_alone_does_not_mark_dirty` pins that.

The observer rule stays as the durable contract ([LiveSnapshot HA](../LiveSnapshot/high-availability.md#dirty-list-edits-since-last-snapshot), `schema.sql` `dirty` comment): ignore `clock <= last_flushed`, because a **retried flush** can still re-mark a page at a clock the pin already covered.

### D4 — trigger search_path follows $user

`venus_mark_dirty` has no `SET search_path`. Default `"$user", public`. Compose hub role is `venus_hub`; there is no `venus_hub` schema today, so `dirty` is `public.dirty`.

**Proposal**

```sql
CREATE OR REPLACE FUNCTION venus_mark_dirty() RETURNS trigger
LANGUAGE plpgsql
SET search_path = public
AS $$
```

Qualify `INSERT INTO public.dirty`. Do not make the function `SECURITY DEFINER` unless the hub role is no longer table owner.

**Landed** (2026-09-13): `SET search_path = public`; `INSERT INTO public.dirty`. Test `venus_mark_dirty_pins_search_path_to_public`. Not `SECURITY DEFINER`.

## Performance


| ID | Sev | Effort | Issue | Where | Effect |
| --- | --- | --- | --- | --- | --- |
| [P1](#p1--new-table-copies-bin) | **snapshot half done; flush half measured** | Medium | `NEW TABLE` copies `bin` | `crdt_update_dirty` | Snapshot triggers **dropped** (whole page `bin` no longer copied). Flush copy measured by `p1_flush_transition_table_cost`: priced by **stored bytes** — 26–54 µs on a typing batch, 4.2 ms of a 21.7 ms cap-sized flush once [P5](#p5--cap-sized-flush-spills-at-default-work_mem)’s spill is out of the way. |
| [P2](#p2--one-savepoint-per-flush) | **won’t fix** (by design) | Tiny | One savepoint per flush | `EXCEPTION` block | P13 made this one subxid per **statement**, not per row, so the 64-subxid cache cannot overflow. The block **is** the “persist if `dirty` is missing” DoD. Keep it. |
| [P3](#p3--row-fallback-on-cutover) | **won’t fix yet** (cutover) | — | ROW fallback on cutover | `venus_mark_dirty` `TG_LEVEL = 'ROW'` | Deliberate compatibility for the one boot where a pre-P13 ROW trigger calls the new function body. Zero steady-state cost (one `IF`). Delete when no live volume can have a ROW `crdt_update_dirty`. |
| [P4](#p4--equal-clock-still-writes) | **done** (D1) | Tiny | Equal clock still writes | `ON CONFLICT DO UPDATE` | Folded into D1’s `WHERE … IS DISTINCT FROM GREATEST` |
| [P5](#p5--cap-sized-flush-spills-at-default-work_mem) | **done** | Small | Cap-sized flush spills at default `work_mem` | `flush_updates` `unnest($3::bytea[])` | Found while measuring P1: an 8 MiB batch writes a **16 MiB temp file** at the 4 MB default, trigger or not. Required `HUB_DB_WORK_MEM` (Compose `16MB`) set per session; `work_mem = 64MB` takes a cap-sized flush from 30.9 ms to 21.7 ms. Typing never spills. |


### P1 — NEW TABLE copies bin

`REFERENCING NEW TABLE AS ins` materializes every inserted/updated column, including `bin`. The function only reads `workspace_id`, `doc_id`, `seq`. Postgres cannot project columns onto a transition table — there is no “`NEW TABLE` without `bin`”.

**Landed** (2026-09-13): **only `crdt_update` marks dirty.** `crdt_snapshot_dirty_ins` / `_upd` are dropped (and the pre-P13 `crdt_snapshot_dirty` name).

Compact is not an edit. It merges trail rows that `crdt_update_dirty` **already** marked, and writes `crdt_snapshot.clock = max_seq` — the same clock. The second upsert was redundant, and it was the copy that scaled with page size (whole document `bin` in `ins` to rewrite one bigint).

```text
flush    INSERT crdt_update      → dirty.clock = max(seq)     ← the only mark
compact  UPSERT crdt_snapshot    → no trigger (same clock)
```

Removing it also deletes two failure modes instead of guarding them: a snapshot replace can no longer rewind the clock ([D1](#d1--dirtyclock-is-not-monotonic)) and can no longer resurrect a GCed row ([D3](#d3--compact-re-inserts-a-processed-row)). `GREATEST` stays for out-of-order **flush** statements (lease steal, foreign writer).

`venus_mark_dirty` now returns early unless `TG_TABLE_NAME = 'crdt_update'`, so a leftover snapshot trigger on an old volume is a no-op until `migrate` drops it. The `migrate` probe skips the trigger batch only when `crdt_update_dirty` is STATEMENT **and** no snapshot dirty trigger is left (it no longer counts to 3), so one boot on an existing volume performs the removal and later boots take no AccessExclusive (L20).

Tests: `snapshot_write_alone_does_not_mark_dirty` (snapshot INSERT + UPDATE leave `dirty` empty; the next `crdt_update` marks it), `migrate_drops_leftover_snapshot_dirty_triggers` (cutover), `migrate_serializes_two_hubs` (one STATEMENT trigger, zero snapshot ones), `compact_if_needed_sql_rebuild_merges_trail` (`dirty.clock` still equals the snapshot clock — the flush set it).

**The flush side stays.** `crdt_update_dirty` keeps `REFERENCING NEW TABLE AS ins` — it needs the transition table to know which page moved — so one flush copies every bin in that batch (ceiling = `PERSIST_BYTES`, **8 MiB**). Measured instead of argued:

```text
cargo test -p venus-hub --test store p1_flush_transition_table_cost -- --ignored --nocapture
```

`p1_flush_transition_table_cost` runs the shipped statement and the same payload into a trigger-less temp table of the same shape, **interleaved** A/B on one connection (laptop Docker drifts more than the trigger costs), min of 7 after warm-ups. Docker `postgres:16`, three runs, all figures reproducible to the spread shown:

| Shape | Rows × bin | Stored / bin | Trigger | Baseline | Whole dirty mark |
| --- | --- | --- | --- | --- | --- |
| Cap, runs of one byte | 128 × 64 KiB | 762 B (pglz) | 35–39 ms | 30–33 ms | 4.6–5.9 ms (14–18 %) |
| Cap, incompressible | 128 × 64 KiB | 64 KiB | 37–43 ms | 20–22 ms | **16.8–21.4 ms (84–97 %)** |
| Cap, sub-TOAST bins | 4096 × 2 KiB | 2 KiB | 75–81 ms | 57–59 ms | 15.5–23.7 ms (26–42 %) |
| Same rows, 16 B bins | 4096 × 16 B | 17 B | 16.1–16.4 ms | 14.6–14.9 ms | 1.50–1.56 ms (10 %) |
| Typing (~1 s batch) | 64 × 128 B | 132 B | 0.51–0.54 ms | 0.48–0.49 ms | 26–48 µs (5–10 %) |

A transition table is a tuplestore sized by `work_mem`, so the harness also sweeps it and reads `pg_stat_database.temp_bytes` for one triggered statement:

| Shape | `work_mem` | Trigger | Baseline | Whole dirty mark | Temp files |
| --- | --- | --- | --- | --- | --- |
| Cap, incompressible | 4 MB (default) | 30.9 ms | 19.5 ms | 11.4 ms (58 %) | 16 MiB |
| Cap, incompressible | 16 MB | 23.4 ms | 17.5 ms | 5.9 ms (34 %) | 0 |
| Cap, incompressible | 64 MB | 21.7 ms | 17.5 ms | **4.2 ms (24 %)** | 0 |
| Cap, sub-TOAST | 4 MB | 68.5 ms | 53.8 ms | 14.7 ms (27 %) | 16 MiB |
| Cap, sub-TOAST | 64 MB | 72.2 ms | 51.3 ms | 20.9 ms (41 %) | 0 |

**The spill is real but it is mostly not `ins`.** The compressible cap shape stores 762 B/bin — a ~95 KiB transition table — and still writes the same 16 MiB temp file while costing only 1.9 ms. So that file is dominated by materialising the 8 MiB `unnest($3::bytea[])` parameter array, which the trigger-less baseline pays too. Removing the spill still helps the 128-row shape a lot (11.4 ms → 4.2 ms), and it speeds the **whole** statement, trigger or not (30.9 ms → 21.7 ms) — see [P5](#p5--cap-sized-flush-spills-at-default-work_mem). It does nothing for the 4096-row shape, whose cost survives at 0 temp bytes.

**The copy is priced by stored bytes, not by rows.** The last two shapes hold row count fixed and cut the payload 128×: the cost falls 13×. The first two hold bytes and row count fixed and only change compressibility: pglz squashing 8 MiB to ~95 KiB total makes the trigger 4× cheaper. Row count is the minor term.

So at the ceiling with incompressible content, the trigger costs **4.2 ms of a 21.7 ms flush once the spill is gone** (24 %), and 11–21 ms at the default `work_mem` where it is competing for a temp file. The earlier “2–6 ms / 6–17 %” reading here was a best-case artefact: its payload was a run of one byte, which pglz squashes before the row — and the transition table — ever sees it.

What has **not** changed is the everyday path: a typing batch pays 26–54 µs. Nothing in the interactive path is at risk; this is a paste-sized-flush problem, and the cheap lever on it is [P5](#p5--cap-sized-flush-spills-at-default-work_mem), not the seam.

Do not drop statement-level `max(seq)` (P13) to shrink `ins`; `FOR EACH ROW` trades the copy for N trigger calls and N savepoints ([P2](#p2--one-savepoint-per-flush)), and the table above shows rows are not the expensive axis anyway. The only shape that removes the copy is the CTE — `INSERT crdt_update … RETURNING` feeding the `dirty` upsert in one statement, trigger dropped — which costs the “any writer marks dirty” property and needs [acceptance #9](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) (“SQL trigger preferred”) re-accepted. **Decision pending**, but 4.2 ms on a paste and 54 µs on a keystroke do not buy that re-accept; [P5](#p5--cap-sized-flush-spills-at-default-work_mem) is the better spend.

### P2 — one savepoint per flush

Plpgsql `BEGIN … EXCEPTION` is a subtransaction even when nothing is raised. One per flush is the P13 contract.

**Won’t fix** (2026-09-13). The Postgres failure mode here is the **64-subxid-per-transaction** cache: overflow it and the snapshot is suboverflowed, so every other backend pays `pg_subtrans` lookups. `flush_updates` is a single statement, hence one implicit transaction with exactly **one** subxid — it cannot grow with batch size or room count, so the cliff is structurally unreachable. What is left is savepoint bookkeeping plus a second XID per flush, inside the 0.11–0.14 ms the [P1](#p1--new-table-copies-bin) harness measures for the whole trigger.

Removing it costs the named DoD: the block is the only reason **persist survives a missing `dirty`** (`flush_lands_when_dirty_table_is_dropped`). “`dirty` is created in the same schema batch” only rules out *install*-time absence; the handler covers the live case (admin or partial restore drops/renames `dirty` under a running hub), where without it every `INSERT crdt_update` rolls back and typing stops persisting. A savepoint is the cheaper side of that trade. The handler stays narrow on purpose — only `undefined_table` / `undefined_column`; a constraint violation or disk error still rolls persist back.

### P3 — ROW fallback on cutover

`schema.sql` is two batches: tables + `CREATE OR REPLACE FUNCTION` commit first, then the trigger batch, and only when the probe finds stale state. On a pre-P13 volume that leaves a window inside the cutover boot where the **old** `FOR EACH ROW` trigger calls the **new** function body. The ROW branch keeps that hub persisting through the window; after the batch it is unreachable.

**Won’t fix yet** (2026-09-13). Steady-state cost is one `IF TG_LEVEL` per statement. Delete the branch (and the ROW arms of the `EXCEPTION` / `RETURN`) once no live volume can have `crdt_update_dirty` as `FOR EACH ROW` — `tgnewtable` set everywhere, which `migrate` converges on the first boot.

Deleting it early is not a hard failure, just a quiet one: the `ins` reference raises `undefined_table` under a ROW trigger, which [D2](#d2--missing-dirty-is-silent) absorbs into a `RAISE WARNING` with persist still committing. The gap is that those statements mark nothing, so a page edited in that window waits for its next edit to be seen. Not worth trading for dead code that costs nothing.

### P4 — equal clock still writes

Fold into D1’s `WHERE` clause. No separate patch.

**Landed** (2026-09-13) with D1.

### P5 — cap-sized flush spills at default work_mem

Found by the [P1](#p1--new-table-copies-bin) harness, and **not** a dirty problem: `flush_updates` passes the batch as one `bytea[]` and materialises it with `unnest($3::bytea[]) WITH ORDINALITY`. At `work_mem = 4MB` (the default, and what Compose runs) an 8 MiB batch writes a ~16 MiB temp file. The trigger-less baseline writes the same file, and the compressible cap shape — whose transition table is ~95 KiB — writes it too, which is how we know it is the parameter array and not `ins`.

Cost of the spill, incompressible cap shape: 30.9 ms → 21.7 ms with the trigger, 19.5 ms → 17.5 ms without, once `work_mem` is 64 MB. Typing-sized batches never spill (0 temp bytes), so this is a paste-path item only.

**Proposal**

Set `work_mem` for hub sessions rather than globally — `after_connect` on the pool, or `ALTER ROLE venus_hub SET work_mem`. Size it against `HUB_DB_MAX_CONNECTIONS` (32 in Compose): `work_mem` is per node per sort/hash, so 64 MB × 32 is not a budget to commit blindly; 16 MB already removes the spill for the 64 KiB shape. Follow the [P17](./steps-1-3-review.md#p17--pool-defaults-are-the-only-pool-config) convention if it becomes config — required env, named in the error.

Do not raise it by dropping the array parameter for a multi-values `INSERT`; that reopens [P4](./steps-1-3-review.md#p4--sequential-insert)’s single-statement shape and [L2](./steps-1-3-review.md#l2--flush-drops-bins)’s all-or-nothing put-back.

**Landed** (2026-09-13): required `HUB_DB_WORK_MEM` (Compose `16MB`), applied per session by `connect_with`’s `after_connect`. `config::parse_work_mem` normalizes the unit and **rejects a bare integer** — Postgres would read `16` as 16 kB — so a typo names the var at start instead of failing every pool connection. The value reaches Postgres through `set_config('work_mem', $1, false)`, a bind parameter, because `SET` takes none and this is environment input. `db::connect` (tests) leaves it `None`.

Fleet scale is **not** this file: `work_mem` is per backend per node, so wikis do not multiply it but replicas do (`replicas × HUB_DB_MAX_CONNECTIONS × HUB_DB_WORK_MEM`; Compose is already 2 × 32 × 16 MB against a 1g Postgres). Budget, the writer-pool and `SET LOCAL` alternatives, and the PgBouncer caveat live in [hub-fleet.md](../../devops/hub-fleet.md#memory-budget-as-replicas-grow).

Tests: `work_mem_normalizes_units_and_rejects_junk` (units, bare integer, `0MB`, a quote-injection token), `work_mem_is_set_per_session_and_stops_the_cap_flush_spilling` (`SHOW work_mem` on a pooled session, then an 8 MiB flush spilling >4 MiB at `4MB` and <1 MiB at `64MB` — both set explicitly, so the assertion does not depend on the server default), `connect_with_honors_pool_settings`, and the Compose env assertion in `apps/web/src/host/compose.test.ts`.

## What already matches the plan

- Dirty grain is `(workspace_id, doc_id)`. Lease grain is `workspace_id`.
- Statement-level `max(seq)` on `crdt_update` — **one** trigger. Compact writes no `dirty` (P1); it rewrites rows the flush already marked at that clock.
- Inner `EXCEPTION` so a missing `dirty` cannot roll back `crdt_update` (`RAISE WARNING`, D2). One subxid per **statement**, kept on purpose (P2).
- Hub SQL does not `INSERT` into `jobs`.
- `first_dirty_at` is insert-only (first seen). Hub never deletes `dirty` rows; GC is M3. Observer ignores `clock <= last_flushed` (D3).
- `dirty.clock` is the SQL `seq` of the persisted update, not a Yjs client clock. Global `BIGSERIAL` on `crdt_update`. `GREATEST` so an out-of-order flush cannot rewind (D1).
- `venus_mark_dirty` `SET search_path = public` (D4) and no-ops unless `TG_TABLE_NAME = 'crdt_update'`.

## Do not

- Poll export to mark dirty.
- Write dirty on apply/broadcast.
- Pause persist to make D1 easier (take advisory lock on flush).
- Re-add a `crdt_snapshot` dirty trigger “for safety” — compact is not an edit, and `migrate` drops it.
- Drop statement-level `max(seq)` for `FOR EACH ROW` to shrink `ins` (P13, P2).
- Move the `dirty` upsert into `flush_updates` SQL (CTE) without re-accepting #9 — P1 measured the whole trigger at 2–6 ms of a cap-sized flush.
- Drop the inner `EXCEPTION` to save a savepoint (P2) — that is the “persist if `dirty` is missing” DoD.
- Reopen steps 1–8 as board work.
