# High-load / high-availability snapshotter

**Status:** accepted (2026-08-30). **M3 code is gated on this file.** Pin rules: [README.md](./README.md). keck limits: [octobase.md](./octobase.md). Convert: [pin-convert.md](../MDGate/pin-convert.md). Git tree: [datamodel — git](../datamodel/git.md). Milestone: [implementation plan — M3](../venus-implementation-plan.md#m3--git-snapshotter-week).

M3 is **one** workspace and **one** page. This file is the scale contract so that slice does not paint “one replica + GET export in one process” into a corner. The invariant does not change: live collaboration never waits on markdown, git, or convert. Revising this file **re-opens the gate** — stop M3 implementation and re-accept.

## Load

| | Target |
|---|---|
| Product workspaces (wikis) | Thousands |
| Pages per wiki | Hundreds |
| Edits | Many pages, many wikis, in parallel |
| Snapshotter | A **fleet**, not one process |

A hidden AFFiNE client **per page** (or per wiki) does not survive this. Neither does one global `wiki/` git index, nor `fromDoc` of a live browser `Store`.

## What this is not

- Not inside OctoBase keck. Not a crate linked into keck. Not a Hyper/axum twin of keck. Sidecar **beside** persist; cloud **replaces** keck ([README — OctoBase](./README.md#octobase)).
- Not an exclusive `LOCK` of a workspace (or of dirty pages) for the duration of convert / `git commit`. That stalls the people typing.
- Not a log of every CRDT op. Git dirty unit is still the **page**.
- Not markdown in Postgres. Pins are short-lived copies. Durable Venus state is **clocks + dirty set + jobs**.

## Units

| Unit | Role |
|---|---|
| **Wiki** (`workspace_id`) | One git repository. One flush job at a time. Idle timer. |
| **Page** (`docId`) | One `.md`. Dirty / pin / convert grain. |
| **Blob** | Dirty with the page that references it (or its own clock). Copied only if dirty. |
| **Catalog** | Pinned in the same **cut** as the dirty pages so `gitPath` matches the files. |

At this load: **one git repo per wiki**. A single working tree cannot take thousands of parallel commits. M3 may still use one `wiki/` because there is one workspace.

## Two buffers

When a flush job hits a wiki, there are two buffers. They are not the same memory.

```text
live generation (always)          pin generation (flush only)
browsers → keck apply/broadcast   copy of dirty docs at cut clock T
         → persist (never paused) held in the worker until commit
         → new writes after T     convert + git run on this copy
           stay on the dirty list
```

**Live keeps living** for the whole pin + convert. Clients do not freeze. keck apply/broadcast does not stop. Persist of **other** pages never stops. Persist of **dirty** pages must also continue: new bytes after T are the **next** snapshot, not a blocked writer.

The pin buffer is the worker’s `Map<docId, { bytes, clock }>`. Crash it and the next job retries. Git HEAD is the last successful commit. Do not durable-write the pin except lease `T0` ([README — where the pin lives](./README.md#where-the-pin-lives)).

## Dirty list (edits since last snapshot)

Per wiki, the snapshotter stores **which pages moved** since `last_flushed`, not the text of the edits.

```text
last_flushed[workspace_id, docId] = { clock, gitSha }
dirty[workspace_id, docId]        = { clock, firstDirtyAt }
```

`dirty` is a **set** keyed by `docId`. A second keystroke on the same page **upserts the clock**. It does not enqueue a second job. Coalesce: all WYSIWYG since last git SHA is one snapshot ([README — git snapshotter](./README.md#git-snapshotter)).

Also dirty: catalog nodes whose `gitPath` changed; blob ids whose bytes changed.

**Who writes `dirty`:** the persist seam, when Yjs (or blob) bytes land. keck **does not** notify `docId`s ([octobase.md](./octobase.md#what-keck-cannot-do)). Do not poll every space. Do not open a replica socket per page.

| Era | How dirty is observed |
|---|---|
| M3 (one page) | One replica or idle `GET …/export`; compare clock to `last_flushed` in process memory |
| Scale / cloud | Persist hook (Hocuspocus `onStore` / own Postgres upsert). Same pin interface: `{ docId, bytes, clock }[]` |

Do not tail keck’s SQL WAL as the product path. That couples Venus to keck tables that cloud deletes.

## Queue

One **pending job per wiki**. Upsert, do not stack.

| Field | Rule |
|---|---|
| Key | `workspace_id` |
| `reason` | `idle` \| `flush` \| `lease` (flush-before-lease / `T0`) |
| `not_before` | Idle: now + 30–120s from `firstDirtyAt`. Flush / lease: now |
| Inflight | **1** per wiki (`FOR UPDATE SKIP LOCKED` on the job row) |
| Coalesce | New dirty while pending: update clocks on `dirty`; do **not** reset `not_before` for `idle`. `flush` / `lease` may pull `not_before` forward |

```text
persist upserts dirty[docId]
        │
        ▼
if no job for wiki → insert job (idle, not_before = firstDirtyAt + debounce)
if job pending     → leave job; dirty set grows
if job inflight    → dirty set grows; after commit, leftover dirty enqueues again
        │
        ▼
workers:  SELECT … WHERE not_before <= now()
          FOR UPDATE SKIP LOCKED
```

Priority: `lease` > `flush` > `idle`. A lease job is allowed to cut in front of idle for **that** wiki only. It does not steal another wiki’s inflight worker mid-convert.

Fairness: N workers, N wikis in parallel. One wiki with hundreds of dirty pages occupies **one** worker until that flush finishes (or hits a time/memory bound — then commit what was pinned and leave the rest dirty). Do not convert the whole fleet on one core.

Durable store: **Venus tables** (same Postgres instance is fine, **not** keck’s `jwst` doc blob schema). Workers are stateless. No leader. No Redis required for correctness.

## Pin cut (lock only dirty files, then copy)

“Lock the DB before we pin” means a **consistent cut of dirty rows only**, then copy those bytes into the pin buffer. It does **not** mean `LOCK TABLE`, `FOR UPDATE` on those rows, or pausing persist until git is done.

**Why not `FOR UPDATE` / `FOR SHARE` on dirty pages:** those lock modes conflict with writers. The pages in the dirty set are exactly the ones people are editing. An exclusive or share-row lock for the copy would stall live CRDT on the hot path. Forbidden by the [invariant](./README.md#invariant).

**What to implement:**

```text
1. Claim job (wiki flush lease — this is the only exclusive lock, and it is Venus’s job row, not CRDT rows)
2. Read dirty set S (docIds ∪ catalog ∪ dirty blobs)
3. Open pin buffer (empty)
4. CUT — consistent read of S at clock T
      live writers continue (new generation / MVCC next row version)
      other wikis untouched
      pages not in S untouched
5. Copy S into the pin buffer (I/O only)
6. End cut — drop the snapshot; persist was never paused
7. Convert + git on the buffer (no DB row locks, no keck involvement)
8. last_flushed[docId] = pin clock (not “now”)
      if dirty[docId].clock > pin clock → page stays dirty → enqueue again
9. Drop pin buffer (keep it if this flush is lease T0)
```

Cut implementation by persist:

| Persist | Cut |
|---|---|
| **Postgres MVCC** (cloud / Venus-owned docs table) | `REPEATABLE READ` (or a snapshot) `SELECT` of dirty doc/blob rows. Plain `SELECT`, not `FOR UPDATE`. Writers append new row versions. Copy bytes out. `COMMIT` ends the cut. |
| **Copy-on-write persist** (preferred later) | Freeze generation **G** for ids in S. New writes allocate **G+1**. Live uses G+1. Worker copies G. Drop G after the pin is in RAM. |
| **Stock keck** | No cut API ([octobase.md](./octobase.md)). M3 idle: wait ≥ persist batch, then `GET …/export` **per dirty space** (export is already a Postgres read). That is a **best-effort** cut, not a multi-space transaction. Acceptable for one page; not the scale mechanism. |

Collect **every** pin in S **before** any `fromDoc` ([README — pin then convert](./README.md#pin-then-convert)). Convert is the slow part; it must run **after** the cut is released so a 200-page `fromDoc` cannot hold a DB snapshot (or generation G) for seconds.

New updates during copy and during convert are **not** in this git commit. They remain on `dirty`. Live buffer keeps them.

## Convert and git

Same Path B as M3: `fromPinnedBytes` / `pinThenFromDoc` on the buffer. Not the spectator pane. Not keck export-as-markdown.

```text
pin buffer (worker RAM, maybe spill to worker disk)
        │  fromDoc + sidecar   (CPU, JS adapter — scale out workers)
        ▼
wiki repo for this workspace
        │  git add / git mv / one commit
        ▼
last_flushed clocks
```

- **Git lock** = the job row (one writer per wiki). Do not share a working tree across workers without that.
- Workers are interchangeable if each wiki has a **remote** (push after commit). Local-disk-only trees need sticky `workspace_id → worker` or a shared volume plus the same job lock.
- Autocomment `snapshot: <title>` for idle/flush; lease accept is the other class (required why) and reuses this convert path.
- Do not re-export clean pages. Catalog-only moves are `git mv` without `fromDoc` if the body clock is unchanged.

`fromDoc` is BlockSuite JS. Scale-out is **more convert workers**, not a Rust reimplementation of the adapter. y-octo + git2 in `crates/venus-sidecar` may own pin-decode + commit in one Venus process; it still consumes the pin interface and still must call the same exporter (Node or embedded JS). It still does not live in keck.

## High availability

| Failure | What happens |
|---|---|
| Worker dies during cut / copy | Pin buffer gone. Job row lease expires. Dirty set unchanged. Another worker retries. |
| Worker dies during convert | Git HEAD unchanged (commit is atomic). Same retry. |
| Worker dies after commit, before `last_flushed` | Next attempt sees same pin clocks; empty diff or identical commit — make step 8 **idempotent** (compare clocks, skip commit if HEAD already has them). |
| Queue DB down | Live CRDT unaffected (keck/Postgres persist still runs). Snapshot lag grows. Do not block editors. |
| One wiki is huge / hot | Occupies one worker; other wikis proceed. Bound pin memory (spill). Bound docs per flush if needed; leftover stays dirty. |
| Convert poison page | Fail that `docId`, keep others in the same pin set if already copied, surface the error. Do not fail the fleet. |

Pins stay non-durable (except `T0`). **HA state** is `dirty` + `jobs` + `last_flushed` + git remotes. Replacing a worker is cheap.

Live CRDT HA (multi-keck, failover) is **not** this file. Prototype keck is one process. Cloud persist replacement owns that. This fleet only has to keep snapshotting without stalling whatever persist is in front.

## M3 must keep this shape

M3 may degenerate every box. It must not invert them.

| Box | M3 (allowed thin) | Must not |
|---|---|---|
| Dirty list | RAM, one `docId` | `fromDoc` every keystroke into git |
| Queue | In-process idle timer + Flush | Commit inside the editor tab |
| Cut | Replica encode **or** idle GET export | `fromDoc` the live `Store`; pause persist for convert |
| Pin buffer | Process `Map` | Write markdown to Postgres |
| Workers | One process | Link into keck |
| Git | One `wiki/` | Per-block commits |

Exit of M3 stays: clone `wiki/` and read markdown; typing during flush still syncs ([implementation plan — M3](../venus-implementation-plan.md#m3--git-snapshotter-week)). This file is the checklist when that process grows a queue.

## Acceptance (gate for M3)

Accepted 2026-08-30. No `wiki/` writer, snapshotter process, or M3 implementation unless this section is accepted. Implementation still waits for an M3 step plan; accepting this file is not a license to start coding in the same change.

[LiveSnapshot README](./README.md) is pin-then-convert for one wiki. This file is the fleet. M3 is the **thin column** of the table above, not a second architecture.

| # | Locked |
|---|---|
| 1 | Live CRDT never waits on markdown, git, or convert. Typing during flush still syncs. |
| 2 | **Wiki** = one git repo and **one** flush job at a time. **Page** = dirty / pin / convert grain. Blobs ride with the page (or their clock). Catalog is in the same **cut** as the dirty pages. |
| 3 | Two buffers: live generation always; pin generation only for the flush. Drop the pin after commit (keep it if this flush is lease `T0`). |
| 4 | Dirty is `{ clock }` on `docId` (plus catalog path / blob id), not keystrokes and not markdown in Postgres. Second edit **upserts** the clock. |
| 5 | Queue: one pending job per wiki; upsert, do not stack; inflight **1**; `lease` > `flush` > `idle`. Workers claim with `FOR UPDATE SKIP LOCKED` on the **job row** (Venus tables), never on CRDT rows. |
| 6 | Cut = consistent read of dirty set **S** at clock **T**, copy bytes, **release the cut**, then `fromDoc`. No `FOR UPDATE` / exclusive lock of CRDT rows. Do not hold the cut across convert or `git commit`. Stock keck has **no** cut API: M3 idle may use replica encode or `GET …/export` (best-effort). That is not the scale mechanism. |
| 7 | Convert is Path B: `pinThenFromDoc` / `fromPinnedBytes` on the pin buffer, same `from-doc.js`. Not the spectator pane. Not keck export-as-markdown. Scale-out is **more convert workers**, not a Rust adapter and not a crate inside keck. |
| 8 | Durable HA state is `dirty` + `jobs` + `last_flushed` + git remotes. Pins stay non-durable except `T0`. Worker death retries from dirty; git HEAD is the last successful commit. Step 8 (`last_flushed`) is **idempotent**. |
| 9 | Snapshotter sits **beside** persist. Cloud **replaces** keck. Do not embed the snapshotter in keck, poll every space, or open one replica per page at thousands of wikis. |

**Left to the M3 plan** (not this gate): idle debounce inside 30–120s; first pin source (replica vs idle GET); Node `simple-git` vs `crates/venus-sidecar`; whether process crash recovers `last_flushed` by reading the git sidecar clock.

## Do not

- Embed the snapshotter in keck, or export a Venus crate for keck to start.
- Open one Y.Doc replica per page at thousands of wikis.
- `FOR UPDATE` / exclusive lock CRDT rows while copying or converting.
- Hold the cut open across `fromDoc` / `git commit`.
- Pause keck `save_update` until pin finishes ([octobase.md](./octobase.md)).
- Use the spectator splice map as a git pin.
- Store the list of keystrokes as the dirty list.
- One global git index for all wikis at this load.

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | Pin then convert; one-wiki prototype |
| [octobase.md](./octobase.md) | Why keck cannot be the queue or the cut |
| [high-availability.md](./high-availability.md) | This scale / HA contract |
| [MDGate pin-convert](../MDGate/pin-convert.md) | Convert helper (no git write) |
