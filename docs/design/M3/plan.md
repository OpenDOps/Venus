# M3 — Git snapshotter


|               |                                                                                           |
| ------------- | ----------------------------------------------------------------------------------------- |
| **planId**    | `m3-git-snapshotter`                                                                      |
| **Milestone** | [M3 in the implementation plan](../venus-implementation-plan.md#m3--git-snapshotter-week) |
| **Duration**  | About 1–1.5 weeks                                                                         |
| **Encoding**  | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B)       |
| **Board**     | [M3.state.yaml](./M3.state.yaml) — steps 1–9 `done` |


This plan **implements** [LiveSnapshot](../LiveSnapshot/README.md) (pin then convert) and the [HA snapshotter](../LiveSnapshot/high-availability.md): `dirty` / `dirty_wiki` → `jobs` → SKIP LOCKED consumers → MVCC cut. Load in M3 is **one wiki / one page**; the **mechanism is not a throwaway**. Do not ship idle GET / in-process-timer as the product queue. Gate: [Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) (items 1–9 unchanged; M3 does not take the GET degeneration in item 6). Convert helper: [pin-convert.md](../MDGate/pin-convert.md). Git tree: [datamodel git](../datamodel/git.md). Hub (live CRDT): [M3.0](../M3.0/README.md), [M3.0 HA](../M3.0/high-availability.md). Exporter: [M2](../M2/README.md), [MDGate](../MDGate/README.md). Dataflow: [architecture.md](../architecture.md). Words: [glossary.md](../glossary.md). Tool choices: [venus-implementation-plan.md](../venus-implementation-plan.md). Installed symbols: [api-map.md](../api-map.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

**Gate (do not skip):** do not start step work in the product repo until [M3.0](../M3.0/README.md) is **closed** (board steps 1–9 `done`) **and** [LiveSnapshot HA Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) is **accepted**. This folder existing is not permission to write `wiki/` or a snapshotter process.

## Story

As an implementer I need casual WYSIWYG to become **cloneable markdown** without a review ceremony: a Venus **snapshotter beside the hub** pins dirty Yjs bytes (pin generation), runs the same `fromDoc` as M2 on that pin, and writes **one** git commit with autocomment `snapshot: <title>`. Live generation (hub apply / broadcast / persist) never waits. Two tabs still sync while that flush runs. Venus still has no lease freeze, catalog tree, or comment-commit why.

If we `fromDoc` the live Store, git is a moving photograph. If convert sits in the hub, live collab waits on markdown. If we commit every keystroke, git is not a snapshot. If we freeze the published page for a snapshot, we have stolen the lease path.

## Exit

All of these must be true at once:

1. `wiki/` is a real git repository (nested, not the product remote), **created by the sidecar on the first snapshot** (git2 init + catalog dirs if missing). It contains the M0 page as markdown at the catalog-v0 path (`spec/home.md` unless api-map Actual says otherwise) plus `.venus/ids/<docId>.json`. No origin in M3. Product git ignores `wiki/`.
2. Dirty is **clocks** (M3.0 `dirty` vs `last_flushed`), not keystrokes. Enqueue grain is **wiki** (`dirty_wiki` → one `jobs` row). Cut grain is the page. One pending job per wiki. Load: one page (`doc:home`).
3. Observer writes `jobs` (`not_before` = idle 60s or Flush now). A worker **claims** (`SKIP LOCKED` + TTL lease, then **COMMIT**), then **MVCC** `REPEATABLE READ` plain `SELECT` of dirty set **S** (`crdt_*` + dirty blobs) into the pin Map. Collect S **before** any `fromDoc`. Convert that pin (Rust engine; JS CLI oracle), then one `git commit` with autocomment `snapshot: <title>`. Convert the pin, not the live `Store`, not `GET …/export`. Drop the pin Map after commit (M5 may keep it as `T0`).
4. Cut is released **before** `fromDoc`. Hub apply / broadcast / persist is **not** paused. Published WYSIWYG is **not** `readonly` (freeze is lease-only). Typing during flush still appears in the other tab without reload (M1 two-tabs still green while a flush is in flight).
5. Clone `wiki/` **elsewhere** and read the page as ordinary markdown (seed headings survive; a typed unique word after Flush is in the file).
6. Casual WYSIWYG did **not** require a review comment. Commit message is autocomment, not a why.
7. Host UI: `git log` for that file shows the autocomment (later comment-commits will sit on the same log; none in M3).
8. Snapshotter is `crates/venus-sidecar` (Rust + y-octo hydrate + convert + **git2**). Convert is Path B on the pin: JS `from-doc.js` (Node CLI, step 2) **and** a Rust adapter (step 3) that is **byte-identical** to that JS. Product Flush uses the api-map **Convert engine** Actual (Rust if goldens match). It is **not** in `crates/venus-hub`. Browser tab does not `git commit`.
9. No markdown in Postgres. Hub does **not** write `jobs` (trigger does **not** write `jobs`). Queue is Venus `jobs` in Postgres, not Akka/Kafka/Redis. No per-block commits.
10. [api-map.md](../api-map.md) Actual column is filled for every snapshotter name the code uses.
11. Existing **M1 Playwright** (`pnpm test:e2e:m1`) and M2 export / pane tests stay green.

## Non-goals (do not start)


| Later                                                        | Why not M3                                                                                                       |
| ------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| Folder tree, catalog CRDT, `git mv`, product header          | [M4](../M4/README.md)                                                                 |
| Lease freeze, CodeMirror, flush-before-lease as `T0` product | M5 (same pin helper; this milestone does not keep pins for a lease)                                              |
| Comment-commit, After/Before/Diff, required why              | M6                                                                                                               |
| Apply / hunks / `ap-*` fixtures                              | M6 — [apply.md](../MDGate/apply.md)                                                                              |
| Hub HPA / gateway / many hub replicas                        | [hub-fleet.md](../../devops/hub-fleet.md)                                                                        |
| k8s snapshotter HPA / many wiki working trees                | Same `jobs` / MVCC; replicaCount follows queue depth later. M3: one Compose `sidecar`, **N workers** in process. |
| LifeIndexing gists / graph (AB1)                             | Parallel after this exit; must not delay `last_flushed`                                                          |
| Bound chat (AB2), chat-edit (AB3), history/why pack (AB4)    | After later milestones                                                                                           |
| `.venus/snapshots/*.bin`                                     | M8 optional                                                                                                      |
| Markdown as Y.Text                                           | Forbidden ([datamodel](../datamodel/README.md))                                                                  |
| Convert / `fromDoc` inside the hub                           | Hub stays apply + broadcast + persist + export/blob                                                              |


Do not commit from the editor tab. Do not `fromDoc` every keystroke. Do not write sidecar JSON to Postgres.

## Constraints

1. **Thin host.** Same Vite + React app. Flush + git log are host chrome (like the markdown pane), not BlockSuite widgets. `mount-editor.js` does not import git or the sidecar.
2. **HA snapshotter, one-wiki load.** Pin-then-convert is [LiveSnapshot README](../LiveSnapshot/README.md). Queue / cut / `last_flushed` are [LiveSnapshot HA](../LiveSnapshot/high-availability.md) (jobs, SKIP LOCKED, MVCC). If this plan disagrees with HA Acceptance items 1–9, **they win**. [api-map.md](../api-map.md) wins for Actual names. 2026-09-13 recon GET-after-2s / no-`jobs` Actuals are **superseded** (2026-09-14).
3. **Two buffers.** Live generation = hub apply / broadcast / persist (never paused, never frozen for a snapshot). Pin generation = sidecar `Map<docId, { bytes, clock }>` **only for the flush**. The hub never sees the pin. Drop the Map after commit (M5 may keep those bytes as `T0`).
4. **Path B convert.** Git uses a pin, then convert — not the live Store, not `incrementalFromDoc`. The **dialect oracle** is M2 `from-doc.js` / `fromPinnedBytes` ([pin-convert.md](../MDGate/pin-convert.md)). Product convert is the Rust `fromDoc` from [step-rust-adapter](#3-step-rust-adapter) (goldens matched). Do not a third dialect.
5. **Pin source = MVCC cut of S.** After **claim**, `BEGIN` + `SET TRANSACTION ISOLATION LEVEL REPEATABLE READ`; plain `SELECT` of dirty set **S** (`crdt_snapshot` + `crdt_update` trail + dirty blobs). **No** `FOR UPDATE` / `FOR SHARE` on those rows. **No** hub `get_doc` (it takes `FOR SHARE`). **No** `GET …/export` as the product pin. Persist lag is inherent: the cut is SQL as of T, not hub RAM. New `crdt_update` rows after T stay on `dirty` for the next job. [HA pin cut](../LiveSnapshot/high-availability.md#pin-cut-lock-only-dirty-files-then-copy).
6. **Pin at claim, not at dirty.** Trigger upserts `dirty` / `dirty_wiki` only. Observer `INSERT…SELECT`s `jobs` (`not_before`). Pin copy starts when a worker **claims** that row. Do not pin on persist. Do not pin when starting idle.
7. **Collect then convert.** Fill the pin Map (page bytes ∪ dirty blobs) **before** any `fromDoc`. **COMMIT** the cut txn before convert (no open snapshot across `fromDoc`). Catalog v0 is a constant `gitPath` in that same collect (no `git mv` until M4).
8. **Dirty.** Product mark is the M3.0 SQL trigger on `crdt_update`, extended to UPSERT `dirty_wiki`. Sidecar **reads** `dirty` / `dirty_wiki` vs `last_flushed`. Do not poll every workspace with export. Hub does **not** write `jobs`. Trigger does **not** write `jobs`.
9. **One collection, one page (load).** Workspace UUID `77e4a2b1-8b40-5979-a73c-fd4477216d00` / BlockSuite `doc:home` (SQL `PAGE_DOC_ID`). Catalog v0 is a **constant** `gitPath`, not a catalog Y.Doc. Schema and consumers are wiki-grained (`jobs.workspace_id`) so a second wiki does not need a rewrite.
10. **Sidecar beside hub, one `wiki/` for this load.** Observer + **N workers** share `venus-sidecar` (Compose one service; `SNAPSHOT_WORKERS` ≥ 2). Sidecar **autoinits** `wiki/` on the first snapshot. No `origin` in M3. Product `.gitignore` `/wiki/`. Do not link convert into the hub binary. Do not share a working tree across workers without the job lease. Browser tab does not `git commit`.
11. **Idle debounce** is `jobs.not_before` = `dirty_wiki.first_dirty_at` + **60s** (`SNAPSHOT_IDLE_MS`). Flush sets `reason=flush`, `not_before=now`. Do not reset idle `not_before` on a second persist (coalesce).
12. **`last_flushed`.** Venus table `{ workspace_id, doc_id, clock, git_sha }`. After commit, if `dirty.clock` > pin clock **T**, the page stays dirty (observer enqueues again once the job row is gone). Idempotent. Crash: table + git HEAD; pin Map is gone.
13. **No WYSIWYG freeze** for a snapshot. `store.readonly` is lease / M5.
14. **Pin `yjs` 13.6.32** and BlockSuite **0.22.4**. Worker hydrate is **y-octo**. Node `Y.applyUpdate` is recon/tests only for convert proof, not Compose `sidecar` apply of the live room.
15. **Memory default.** Vitest and `pnpm test:e2e` stay green without Docker. M3 e2e / Flush require Compose **hub** (+ sidecar). Unset sync env: no git write.
16. **Docker is the runtime** for the snapshotter DoD (Compose `sidecar` or documented `pnpm` spawn against hub). Host `cargo run` is recon only.
17. **No `@affine/core`.** No nbstore. No convert in the hub image. No LifeIndexing on the cut.

## LiveSnapshot mapping

This plan **fills** the HA boxes. Load is one wiki; do not invert them.


| Design ([README](../LiveSnapshot/README.md) / [HA](../LiveSnapshot/high-availability.md)) | This plan                                                                                    |
| ----------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Live CRDT never waits; freeze is lease-only                                               | [step-live-during-flush](#7-step-live-during-flush); Flush does **not** set `store.readonly` |
| Two buffers: live always; pin only for flush; drop pin after commit                       | Hub = live; sidecar Map = pin; drop Map in [step-flush](#6-step-flush)                       |
| Dirty = `{ clock }` on `(workspace_id, docId)`; enqueue = wiki                            | Trigger → `dirty` + `dirty_wiki`; cut reads page `dirty`                                     |
| Queue = `jobs`; pin at **claim**                                                          | Observer `INSERT…SELECT`; workers `SKIP LOCKED` in [step-pin-queue](#42-step-pin-queue)      |
| Cut = MVCC `SELECT` of S; release before `fromDoc`                                        | [step-pin-cut](#43-step-pin-cut): `REPEATABLE READ` plain `SELECT` of `crdt_*` + blobs       |
| Collect every pin before any `fromDoc`                                                    | Claim fills Map (page + dirty blobs) then Path B                                             |
| Convert = Path B on the pin; Rust worker beside hub                                       | JS CLI oracle; Rust `fromDoc` product engine; not hub; not splice                            |
| `last_flushed` after commit; idempotent; pins non-durable                                 | Venus `last_flushed` table; drop Map in [step-flush](#6-step-flush)                          |
| One `wiki/` per wiki; snapshot autocomment; not per-block                                 | Nested `wiki/`; sidecar **autoinit** on first Flush; `snapshot: <title>`                     |
| Blobs ride with the page; catalog in the same cut                                         | `wiki/assets/` if dirty; catalog v0 constant `spec/home.md`                                  |
| Not M3                                                                                    | `git mv`, keep pin as `T0`, AB1 on the cut, hub HPA                                          |


## Target tree

Only create what M3 needs. Do **not** add catalog packages, review types, or apply tests. Do **not** put git2 in `crates/venus-hub`.

```text
Venus/
  docker-compose.yml                 # postgres + hub + web + sidecar
  deploy/
    sidecar/Dockerfile               # venus-sidecar: y-octo + Node from-doc + git2
  crates/venus-sidecar/              # y-octo + JS CLI + Rust fromDoc (step 3) + git2
  wiki/                              # created on first snapshot (product .gitignore)
    spec/home.md                     # catalog v0 path for doc:home
    .venus/ids/<docId>.json
    assets/                          # dirty blobs only
  apps/web/
    src/host/
      mdgate/from-doc.js             # unchanged exporter
      mdgate/pin-from-doc.js         # Path B (sidecar must call this, not splice)
      snapshot/                      # Flush button + git-log chrome (not mount-editor)
    e2e/
      m3-flush.spec.ts
      m3-git-log.spec.ts
      m3-live-during-flush.spec.ts
      m1-*.spec.ts                   # still pass
      m2-*.spec.ts
  docs/design/api-map.md             # snapshotter Actuals in step 1
  docs/design/M3/
    …
```

## Binding (what you are proving)

```text
live generation (always)                    pin generation (after claim only)
Tab A / Tab B  (BlockSuite Store → Y.Doc)
        │  y-protocols/sync   (never waits; not readonly)
        ▼
Venus hub  (Compose `hub`)  apply + broadcast + persist ~1s
        │  AFTER persist UPSERT dirty + dirty_wiki   (SQL trigger; no jobs)
        ▼
Postgres  crdt_* + blob + dirty + dirty_wiki + jobs + last_flushed
        │
        │  observer INSERT…SELECT jobs FROM dirty_wiki
        │  not_before = first_dirty_at + 60s  (Flush: now)
        │  ON CONFLICT DO NOTHING
        ▼
venus-sidecar  (Compose `sidecar`, N workers)
        │  CLAIM: SKIP LOCKED + TTL lease + COMMIT
        │  CUT: REPEATABLE READ SELECT S (crdt_* + dirty blobs)
        │  pin Map { bytes, clock }     ← frozen copy only
        │  COMMIT cut                   ← convert must not hold this
        │  y-octo hydrate + Rust fromDoc
        │  if no .git: git2 init + mkdir gitPath parents   ← first snapshot
        ▼
wiki/  spec/home.md + .venus/ids/<docId>.json  (+ assets/ if dirty)
        │  git2 one commit  message: snapshot: <title>
        ▼
last_flushed = { T, gitSha }   (Postgres)
drop pin Map
```

Ids stay:

```text
TestWorkspace.id  =  77e4a2b1-8b40-5979-a73c-fd4477216d00
doc:home          =  store.spaceDoc; SQL PAGE_DOC_ID; dirty.grain
gitPath           =  spec/home.md     (catalog v0 constant)
```

## Chosen stack

Locked in [step-recon-snapshot](#1-step-recon-snapshot), **except pin / queue / last_flushed** (superseded 2026-09-14: HA jobs + MVCC). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.


| Piece        | Intent                                                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| Live CRDT    | Unchanged: hub + `OctoBaseKeckProvider` (`kind: 'octobase'` alias)                                                                   |
| Snapshotter  | `crates/venus-sidecar`, Compose `sidecar`. Observer + **N workers**. Rust + **y-octo** hydrate + **git2**. Not in the hub.       |
| Convert JS   | Dialect **oracle**: `from-doc.js` / `fromPinnedBytes` via Node CLI (`from-pinned-cli.js`). Pane `splice.js` is **not** on this path. |
| Convert Rust | In-process `fromDoc` on y-octo ([step-rust-adapter](#3-step-rust-adapter); goldens matched). Product Flush engine.                   |
| Pin source   | After claim: MVCC `REPEATABLE READ` plain `SELECT` of S. Not GET export. Not replica encode. Not the tab `Store`.                    |
| Dirty        | Postgres trigger on `crdt_update` → `dirty` + `dirty_wiki`. No second keystroke list.                                                |
| Queue        | Venus `jobs`. Observer `INSERT…SELECT`. Workers `SKIP LOCKED` + TTL lease, then COMMIT. Pin at **claim**.                        |
| Git          | Nested `wiki/`, **git2**. Sidecar **init on first snapshot** if missing. No origin in M3. Autocomment `snapshot: <title>`.           |
| Catalog v0   | Constant: `doc:home` → `spec/home.md`. No catalog CRDT.                                                                              |
| last_flushed | Venus table `{ clock, git_sha }`. Crash = that row + git HEAD. Idempotent.                                                           |
| Out of scope | Lease UI, convert in hub, markdown in SQL, AB1 LLM, keep pin as `T0`, k8s worker HPA                                                 |


### Pin and git


|              |                                                                                                                                                                                                          |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Idle**     | `jobs.not_before` = `dirty_wiki.first_dirty_at` + 60s (`SNAPSHOT_IDLE_MS`). Second persist upserts `dirty.clock`; does **not** reset `not_before` or `first_dirty_at`. **No pin** until a worker claims. |
| **Flush**    | Host chrome → sidecar `POST /flush`: `reason=flush`, `not_before=now` (insert or update the wiki’s job). Pin starts at **claim**, not at the click.                                                      |
| **Cut**      | `REPEATABLE READ` plain `SELECT` of S. Not `FOR UPDATE` on `crdt_*`. COMMIT before `fromDoc`.                                                                                                             |
| **Commit**   | One commit per claimed job. Message `snapshot: <title>` (page title, seed `Venus`). Coalesce all WYSIWYG since last SHA.                                                                                 |
| **Autoinit** | After convert, before write: if `wiki/` has no `.git`, `git2` init + mkdir catalog dirs. Idempotent. No origin. Not on dirty upsert.                                                                     |
| **Blobs**    | Collect dirty blob bytes into the pin **in the same cut**; write `wiki/assets/` only if the pin’s page references them.                                                                                  |
| **Crash**    | Pin Map gone. Job lease expires → another worker claims. Retry from `dirty` vs `last_flushed`. Git HEAD is last successful commit.                                                                       |


## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).


| #   | id                                                    | Adds                                                                                                                   |
| --- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| 1   | `[step-recon-snapshot](#1-step-recon-snapshot)`       | ✅ **done.** Gate + map: pin source, worker, gitPath, autoinit-on-flush, idle, JS embed; spike convert.                 |
| 2   | `[step-worker](#2-step-worker)`                       | ✅ **done.** `crates/venus-sidecar`: y-octo hydrate + JS `from-doc.js` CLI; **no** git; **no** Rust adapter yet.        |
| 3   | `[step-rust-adapter](#3-step-rust-adapter)`           | ✅ **done.** Pure-Rust `fromDoc`; byte-identical to JS; large-file wall-time vs JS-from-Rust.                           |
| 4   | [`step-pin`](#4-step-pin)                             | ✅ **done.** HA pin (sub-steps 4.1–4.3). No GET. No git.                                                                |
| 4.1 | [`step-pin-schema`](#41-step-pin-schema)              | ✅ **done.** `dirty_wiki` + `jobs` + `last_flushed`; trigger upserts `dirty_wiki` only.                                 |
| 4.2 | [`step-pin-queue`](#42-step-pin-queue)                | ✅ **done.** Observer writes `jobs`; N consumers `SKIP LOCKED` + TTL lease. No Yjs copy yet.                            |
| 4.3 | [`step-pin-cut`](#43-step-pin-cut)                    | ✅ **done.** After claim: MVCC `SELECT` of S → pin Map; COMMIT; convert from Map.                                       |
| 5   | `[step-dirty-idle](#5-step-dirty-idle)`               | ✅ **done.** `not_before` idle 60s; Flush due now; coalesce; no pin at enqueue.                                         |
| 6   | `[step-flush](#6-step-flush)`                         | ✅ **done.** Autoinits `wiki/` if missing; convert pin; one autocomment commit; write `last_flushed`; drop Map.         |
| 7   | `[step-live-during-flush](#7-step-live-during-flush)` | ✅ **done.** Typing during convert still syncs A→B; hub persist not paused.                                     |
| 8   | `[step-git-log](#8-step-git-log)`                     | ✅ **done.** Host chrome: `git log` for `spec/home.md` (autocomment visible).                                           |
| 9   | `[step-verify](#9-step-verify)`                       | ✅ **done.** Close-out: clone elsewhere; person + Playwright; board `done`.                                             |


---

## Steps

Do them in order (1–9). Step 4 is three sub-steps (4.1 → 4.2 → 4.3); a sub-step is not started until its `dependsOn` is done. Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

### 1. step-recon-snapshot

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-worker) · [3](#3-step-rust-adapter) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                                                                                 |
| ------------- | --------------------------------------------------------------------------------------------------------------- |
| **n**         | 1                                                                                                               |
| **id**        | `step-recon-snapshot`                                                                                           |
| **title**     | Map pin source, worker, gitPath, autoinit-on-flush, JS embed                                                    |
| **dependsOn** | M3.0 closed; [LiveSnapshot HA Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) accepted |
| **kind**      | implement                                                                                                       |


**Adds:** a decision and a map, not `wiki/` commits. You know which process writes git, how it gets pin bytes, how it runs `from-doc.js`, that the hub is not that process, and that **`wiki/` is created on first snapshot** (not a pre-inited folder).

**2026-09-14:** pin source / queue Actuals from this step (GET after ≥2s, no `jobs`) are **superseded**. Product cut is MVCC after `jobs` claim ([step-pin-cut](#43-step-pin-cut)). Leave this step `done`; do not re-run recon GET as the pin design.

M3 dies if convert is bolted onto `handle_socket`, or if git photographs the live tab.

#### Work

1. Confirm the **gate** on the board: M3.0 steps 1–9 `done`; HA Acceptance table accepted (date + who on [M3.state.yaml](./M3.state.yaml) evidence or the HA file). **Stop** if either is open.
2. Spike (throwaway ok): hub up, seed `doc:home`, wait ≥2s, api-map **Export command**, `fromPinnedBytes` (or `hydrateM0FromUpdate` + `fromDoc`). Log markdown contains seed H1. **No** `git commit`. Prove this does not import `splice.js`.
3. Decide **pin source** (GET after ≥2s vs replica encode). Write it in [api-map.md](../api-map.md).
4. Decide **how Rust hosts JS** (`from-doc.js`): Node CLI vs embed. Record the command/binary. Do not reimplement `MarkdownAdapter` **in this step** (that is [step-rust-adapter](#3-step-rust-adapter)).
5. Decide **gitPath** (`spec/home.md`), sidecar filename (`doc:home` vs `PAGE_DOC_ID`), wiki dir env, default branch (e.g. `main`), idle env name, Compose service name (`sidecar`), Flush HTTP or stdio. Product root `.gitignore`: `/wiki/` (rule only — do **not** `git init`).
6. Fill [api-map.md](../api-map.md) **Names — git snapshotter** Actuals. Record: sidecar **autoinit on first Flush**; no origin in M3; one inflight per wiki; pin at **run** not dirty upsert; collect page+blobs then `fromDoc`; drop pin after commit; crash recovery = sidecar clock; no `jobs` in M3.

#### Do not

- `git init wiki/` or check in an empty nested repo (that is [step-flush](#6-step-flush) autoinit).
- Import git into `mount-editor.js` or `crates/venus-hub`.
- Start the Flush UI.
- Add `jobs` / `dirty_wiki` tables.
- Set `origin` / pick a git host.
- Call M3 started because this folder exists.

#### Test scenarios

1. **Gate held**
  - **Given** this repo.
  - **When** you read [M3.0.state.yaml](../M3.0/M3.0.state.yaml) and [LiveSnapshot HA Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3).
  - **Then** M3.0 steps 1–9 are `done`, and the HA table is accepted (not “re-accept before M3 code” with no date).
  - **How:** grep / review. Fail if M3.0 `step-verify` is still `pending`. Fail if HA still says un-accepted.
2. **Map complete**
  - **Given** the repo after this step.
  - **When** a reviewer opens [api-map.md](../api-map.md) **Names — git snapshotter**.
  - **Then** these are concrete: pin source, export/replica command, sidecar crate / Compose service, convert JS command, `gitPath`, sidecar path, wiki dir, autoinit-on-flush, idle default, Flush trigger, `last_flushed` recovery. Catalog v0 is one path (`spec/home.md` unless Actual differs).
  - **How:** read the file. Fail if cells are empty or `recon:`. Fail if a second page path exists “for later.”
3. **Product ignores wiki/**
  - **Given** Venus root `.gitignore`.
  - **When** you `git check-ignore -v wiki`.
  - **Then** the product git ignores that path (even if `wiki/` does not exist yet).
  - **How:** shell / Vitest. Fail if `/wiki/` is missing from `.gitignore`.
4. **Spike convert**
  - **Given** hub + Postgres, seeded `doc:home` (Compose or testcontainers).
  - **When** you export (wait ≥2s) and run Path B convert with **no** git.
  - **Then** markdown contains `Why Venus`; the convert module graph does **not** import `splice.js` / `mount-md-pane.js`.
  - **How:** Vitest or `cargo test -p venus-sidecar` recon. Fail if convert used the live Store in a browser tab.

---

### 2. step-worker

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · **2** · [3](#3-step-rust-adapter) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                          |
| ------------- | -------------------------------------------------------- |
| **n**         | 2                                                        |
| **id**        | `step-worker`                                            |
| **title**     | venus-sidecar: y-octo hydrate + from-doc.js (no git yet) |
| **dependsOn** | `step-recon-snapshot`                                    |
| **kind**      | implement                                                |


**Adds:** the convert process. It hydrates pin bytes with y-octo and produces `{ markdown, sidecar }` with the M2 exporter. It does not init `wiki/` or commit.

#### Work

1. Workspace member `crates/venus-sidecar`. **git2** may be a dep already; do not call **init or commit** yet if that keeps the crate smaller — adding git2 without using it is fine. Hydrate: y-octo `apply_update_from_binary_v1` / `try_from_binary_v1`.
2. Invoke Path B: the recon **convert JS command** (Node CLI importing `fromPinnedBytes`). Goldens: seed H1/H2 match M2 `fromDoc` (modulo subset whitespace).
3. `deploy/sidecar/Dockerfile` **or** a documented host binary for later Compose. Health may be “process boots.” Hub Dockerfile must **not** COPY this crate as the hub entrypoint.
4. Static check: `crates/venus-hub` does not depend on `venus-sidecar` / `git2` / `from-doc.js`.

#### Do not

- `fromDoc` inside the hub HTTP handler.
- A Rust `MarkdownAdapter` here (that is [step-rust-adapter](#3-step-rust-adapter)).
- `simple-git` as the product writer.
- `git init wiki/` here (autoinit is [step-flush](#6-step-flush)).

#### Test scenarios

1. **Hydrate pin**
  - **Given** export bytes of seeded `doc:home` (fixture or live hub).
  - **When** sidecar hydrates with y-octo.
  - **Then** apply does not throw; re-encode is >2 bytes.
  - **How:** `cargo test -p venus-sidecar`. Fail if hydrate is Node `Y.applyUpdate` as the **product** path (tests may still decode with yjs).
2. **Same exporter**
  - **Given** those pin bytes.
  - **When** sidecar runs Path B convert.
  - **Then** markdown contains `Why Venus` and `Empty host`; sidecar has block ids; body has no `<!-- id:… -->`.
  - **How:** same crate test or Vitest calling the CLI. Fail if output came from `incrementalFromDoc`.
3. **Not in hub**
  - **Given** `crates/venus-hub/Cargo.toml` and hub sources.
  - **When** you search for `venus-sidecar`, `git2`, `from-doc`, `MarkdownAdapter`.
  - **Then** none are product deps/imports of the hub.
  - **How:** grep / `cargo tree -p venus-hub`. Fail if convert linked into `venus-hub`.

---

### 3. step-rust-adapter

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · **3** · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| **n**         | 3                                                                  |
| **id**        | `step-rust-adapter`                                                |
| **title**     | Rust fromDoc: byte-identical to JS; large-file wall-time vs JS CLI |
| **dependsOn** | `step-worker`                                                      |
| **kind**      | implement                                                          |


**Adds:** an in-process Rust Path B convert on the same pin bytes as the JS CLI. It is allowed to become the product Flush engine **only** if markdown (and sidecar ranges) match the JS oracle exactly. A bench records how the two variants differ on a large page. It does not init `wiki/` or commit.

This is the HA bar “second adapter **until goldens match**.” Matching is the step. A different markdown dialect fails closed.

#### Work

1. In `crates/venus-sidecar` (or a workspace crate it owns): Rust `fromDoc` over a y-octo-hydrated pin. Cover the M2 [round-trippable subset](../MDGate/subset.md) (page title, paragraphs, `h1`–`h6`, lists, code, links, marks, linked-doc `venus:doc` comment, opaque image form). Same UTF-16 sidecar `start`/`end` rules as `from-doc.js`. Do **not** put this in `crates/venus-hub`.
2. One API both tests call: `{ markdown, sidecar }` from pin bytes. **Oracle** = step 2 JS CLI (`from-pinned-cli.js` / `fromPinnedBytes`). **Candidate** = Rust, in-process (no Node).
3. If markdown + sidecar `blocks[]` (`id`, `start`, `end`) + `docId` are byte/field equal on M2 goldens **and** the large fixture: record Convert engine Actual = **Rust** in [api-map.md](../api-map.md). JS CLI stays the test oracle. If they differ, this step is **not** done (do not “close enough”).
4. Bench both variants on the **same** large pin (below). Write numbers to [convert-bench.md](./convert-bench.md) (or yaml evidence on the board). Do **not** fail the step because Rust is slower or faster — fail only if bytes differ or the bench was not run.
5. Flush (step 6) will call the api-map Convert engine. Until this step is `done`, product convert stays the JS CLI.

#### Do not

- A third dialect (post-process JS output, squeeze blanks, different list markers).
- Convert inside the hub.
- `toDoc` / apply in this step (M6).
- Skip large-file timing because seed convert is fast.
- Declare Rust the product engine while goldens differ.

#### Test scenarios

1. **Seed identical**
  - **Given** pin bytes of seeded `doc:home` (same fixture as step 2).
  - **When** JS CLI convert and Rust convert both run on those bytes.
  - **Then** `markdown` bytes are identical (including EOF `\n`). Sidecar `docId` matches. `blocks[]` matches (`id`, `start`, `end` UTF-16). Body has no `<!-- id:… -->`.
  - **How:** `cargo test -p venus-sidecar` (spawns the JS CLI). Fail on the first differing byte / first mismatched range. Fail if Rust called `fromDoc` on a live Store.
2. **M2 goldens identical**
  - **Given** pin (or Store→pin) fixtures for [subset](../MDGate/subset.md) goldens used by M2 `fromDoc` (`rt-paragraph`, `rt-headings`, `rt-list`, `rt-code`, `rt-link`, `rt-marks`, opaque image, seed). Linked-doc export golden if the pin can hold `affine:embed-linked-doc`.
  - **When** both engines convert each pin.
  - **Then** markdown bytes match per fixture. Opaque / loss cases match the JS form (do not invent a nicer dump).
  - **How:** same crate test, fixtures under `crates/venus-sidecar/tests/` or shared `apps/web` goldens. Fail if Rust skips a golden that JS emits.
3. **Large file identical**
  - **Given** one generated pin: at least **2 000** `affine:paragraph` blocks (unique text so the file is not a tiny repeat), markdown **≥ 512 KiB** (raise Actual in api-map if the generator lands bigger). Same pin file for both engines.
  - **When** JS CLI and Rust convert that pin.
  - **Then** markdown bytes are identical; sidecar `blocks.len()` equals the paragraph count (+ title/note wrappers Actual).
  - **How:** generator in the sidecar test (BlockSuite JS to build the pin **once**, or a recorded `.yjs` fixture committed if < GitHub limits; otherwise generate in CI). Fail if only a truncated prefix was compared.
4. **Large file wall time (both variants)**
  - **Given** the same large pin as scenario 3.
  - **When** you measure:
    - **JS-from-Rust:** sidecar spawns the Node CLI (cold spawn counted; that is the step-2 product path). Warmup **3** runs, then **10** timed runs.
    - **Pure Rust:** in-process convert. Same warmup + 10 runs.
  - **Then** [convert-bench.md](./convert-bench.md) (or board evidence) records for **each** variant: pin bytes, markdown bytes, block count, median ms, p95 ms, max ms, machine OS/CPU. Optional: JS convert-only if the CLI process is reused (second table). **No pass/fail on which is faster.**
  - **How:** `cargo test -p venus-sidecar -- --ignored` or `cargo bench -p venus-sidecar` plus a checked-in markdown table. Fail if either variant is missing, if N < 10, or if the file was smaller than scenario 3.
5. **Still not in hub**
  - **Given** hub sources.
  - **When** you search for the Rust `fromDoc` module / `venus-sidecar` convert.
  - **Then** `crates/venus-hub` does not depend on it.
  - **How:** `cargo tree -p venus-hub`. Fail if the adapter linked into the hub.

---

### 4. step-pin

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · [3](#3-step-rust-adapter) · **4** ([4.1](#41-step-pin-schema) · [4.2](#42-step-pin-queue) · [4.3](#43-step-pin-cut)) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                     |
| ------------- | ----------------------------------- |
| **n**         | 4                                   |
| **id**        | `step-pin`                          |
| **title**     | Jobs queue + MVCC pin Map (not GET) |
| **dependsOn** | `step-rust-adapter`                 |
| **kind**      | implement                           |


**Adds:** the HA snapshotter cut, in three sub-steps. Board `step-pin` is **done** only when 4.1–4.3 are done. Live hub RAM is not frozen. Pin starts at **claim**, not at `dirty` upsert. No git in this step.

This is [HA pin cut](../LiveSnapshot/high-availability.md#pin-cut-lock-only-dirty-files-then-copy) and [queue](../LiveSnapshot/high-availability.md#queue). Do not implement idle GET / `SNAPSHOT_PERSIST_WAIT_MS` as a product path. **Work** is under the sub-steps, not this heading: [4.1](#41-step-pin-schema) → [4.2](#42-step-pin-queue) → [4.3](#43-step-pin-cut).

#### 4.1 step-pin-schema

[Back to 4](#4-step-pin). Sub-steps: **4.1** · [4.2](#42-step-pin-queue) · [4.3](#43-step-pin-cut)


|               |                                              |
| ------------- | -------------------------------------------- |
| **n**         | 4.1                                          |
| **id**        | `step-pin-schema`                            |
| **title**     | dirty_wiki + jobs + last_flushed tables      |
| **dependsOn** | `step-rust-adapter`                          |
| **kind**      | implement                                    |


**Adds:** Venus tables in the same Postgres as `crdt_*`. Persist trigger upserts `dirty_wiki`. Hub still does not write `jobs`. No observer, no workers, no pin Map.

##### Work

1. Install from hub `schema.sql` (the persist trigger must see the tables):
   - `dirty_wiki(workspace_id PK, first_dirty_at)`
   - `jobs(workspace_id PK, reason TEXT, not_before TIMESTAMPTZ, owner TEXT, lease_until TIMESTAMPTZ)` — `reason` in `idle` \| `flush` \| `lease`
   - `last_flushed(workspace_id, doc_id, clock, git_sha)` PK `(workspace_id, doc_id)`
2. Extend `venus_mark_dirty`: on `crdt_update` also `INSERT … ON CONFLICT DO NOTHING` into `dirty_wiki` (do **not** reset `first_dirty_at` when `dirty.clock` moves). Trigger still must **not** write `jobs`. Keep the fail-safe `EXCEPTION` / `WARNING` so a missing table cannot roll back persist.

##### Do not

- `INSERT jobs` in `venus_mark_dirty` or in `crates/venus-hub`.
- Observer / workers / pin collect.
- `GET /api/block/…/export` as a pin.
- `git init` / commit.

##### Test scenarios

1. **Trigger marks dirty_wiki, not jobs**
   - **Given** hub persist of one WS write (wait one persist tick).
   - **When** you `SELECT` `dirty`, `dirty_wiki`, `jobs`.
   - **Then** one `dirty` row and one `dirty_wiki` row for the M0 workspace; `jobs` is **empty**.
   - **How:** hub SQL test (`cargo test -p venus-hub`). Fail if the trigger inserted `jobs`. Fail if hub sources `INSERT INTO jobs`.
2. **Tables exist**
   - **Given** a migrated hub DB.
   - **When** you `\dt` / `information_schema` for `dirty_wiki`, `jobs`, `last_flushed`.
   - **Then** all three exist; `jobs` has a unique `workspace_id`.
   - **How:** same crate migrate test. Fail if sidecar was required to create them (trigger would miss `dirty_wiki` on persist).

#### 4.2 step-pin-queue

[Back to 4](#4-step-pin). Sub-steps: [4.1](#41-step-pin-schema) · **4.2** · [4.3](#43-step-pin-cut)


|               |                                              |
| ------------- | -------------------------------------------- |
| **n**         | 4.2                                          |
| **id**        | `step-pin-queue`                             |
| **title**     | Observer writes jobs; SKIP LOCKED consumers  |
| **dependsOn** | `step-pin-schema`                            |
| **kind**      | implement                                    |


**Adds:** sidecar **observer** + **N workers** that claim. Claim leases the `jobs` row and **COMMIT**s. Pin Map stays **empty** (no Yjs copy yet — that is [4.3](#43-step-pin-cut)). Tests may set `SNAPSHOT_IDLE_MS=0` so `not_before` is due immediately. Debounce / Flush HTTP are [step-dirty-idle](#5-step-dirty-idle).

##### Work

1. **Observer** in `venus-sidecar` (timer `SNAPSHOT_OBSERVE_MS`, or one `NOTIFY` per persist batch — not `NOTIFY` per wiki). Ignore `dirty.clock <= last_flushed.clock`.
   ```sql
   INSERT INTO jobs (workspace_id, reason, not_before)
   SELECT w.workspace_id, 'idle', w.first_dirty_at + ($idle_ms || ' milliseconds')::interval
   FROM dirty_wiki w
   WHERE NOT EXISTS (
     SELECT 1 FROM jobs j WHERE j.workspace_id = w.workspace_id
   )
   ON CONFLICT (workspace_id) DO NOTHING;
   ```
   Two observer loops must be safe (`ON CONFLICT DO NOTHING`).
2. **Workers** (`SNAPSHOT_WORKERS` default **≥ 2**):
   ```text
   BEGIN
     SELECT … FROM jobs
     WHERE not_before <= now()
       AND (lease_until IS NULL OR lease_until < now())
     FOR UPDATE SKIP LOCKED
     LIMIT 1
     UPDATE jobs SET owner = $worker, lease_until = now() + interval '2 minutes'
   COMMIT
   -- later: 4.3 pin / fromDoc; git is step-flush (no row lock, no open txn on jobs)
   heartbeat lease_until until done
   ```
   One inflight per wiki = that lease. In this sub-step the worker body is a no-op (or log + release in tests). Never hold the claim txn across later pin or convert.

##### Do not

- Copy Yjs bytes / fill the pin Map (enqueue ≠ claim ≠ cut).
- Poll `GET …/export` as the dirty observer.
- `INSERT jobs` in the hub or in the trigger.
- One job per keystroke (still `UNIQUE (workspace_id)`).
- `FOR UPDATE` on `crdt_*` / `dirty`.
- `git init` / commit.

##### Test scenarios

1. **Observer enqueues one job**
   - **Given** a `dirty_wiki` row, no `last_flushed`, observer tick, `SNAPSHOT_IDLE_MS=0`.
   - **When** observer runs twice (two loops or two ticks).
   - **Then** exactly **one** `jobs` row (`reason=idle`, `not_before <= now()`). Second insert is `ON CONFLICT DO NOTHING`.
   - **How:** `cargo test -p venus-sidecar`. Fail if two rows or if observer `GET`s export to decide dirty.
2. **One wiki, two consumers, one claim**
   - **Given** that due job, two worker tasks.
   - **When** both try to claim.
   - **Then** exactly one owner; the other `SKIP LOCKED` sees nothing. Inflight stays 1. Pin Map empty.
   - **How:** sidecar test. Fail if both claim. Fail if claim copied Yjs bytes.
3. **Two wikis, two consumers**
   - **Given** two workspace ids with due jobs (fixture `dirty_wiki` + `jobs`).
   - **When** two workers claim.
   - **Then** both jobs are leased (parallel wikis).
   - **How:** sidecar test. Fail if a single global mutex serializes all wikis.

#### 4.3 step-pin-cut

[Back to 4](#4-step-pin). Sub-steps: [4.1](#41-step-pin-schema) · [4.2](#42-step-pin-queue) · **4.3**


|               |                                              |
| ------------- | -------------------------------------------- |
| **n**         | 4.3                                          |
| **id**        | `step-pin-cut`                               |
| **title**     | MVCC SELECT of S into pin Map                |
| **dependsOn** | `step-pin-queue`                             |
| **kind**      | implement                                    |


**Adds:** after [4.2](#42-step-pin-queue) **claim**, copy dirty set **S** under `REPEATABLE READ` into `Map<docId, { bytes, clock }>`. Convert on that Map after the cut **COMMIT**. Hub export HTTP is not the collect.

##### Work

1. **Cut** after claim (this wiki’s dirty pages, ignore `clock <= last_flushed`):
   ```text
   BEGIN
     SET TRANSACTION ISOLATION LEVEL REPEATABLE READ
     -- plain SELECT, no FOR SHARE / FOR UPDATE / hub get_doc
     SELECT bin FROM crdt_snapshot WHERE workspace_id AND doc_id
     SELECT seq, bin FROM crdt_update WHERE workspace_id AND doc_id ORDER BY seq
     SELECT … FROM blob for dirty blob ids
   COMMIT
   hydrate snapshot+trail with y-octo; encode update v1 → Map { bytes, clock }
   ```
   `clock` = persist seq (`dirty.clock` at cut), documented Actual. Catalog v0 `gitPath` is a constant in the same collect. Fill **all** of S before any `fromDoc`.
2. Convert **only** from the Map (Rust `from_doc::from_pinned_bytes`). Drop the cut txn before convert.

##### Do not

- `GET /api/block/…/export` (or replica encode) as the product pin.
- `fromDoc(session.store)` in the browser for git.
- Pause hub persist while copying.
- `FOR UPDATE` / `FOR SHARE` on `crdt_*` / `dirty`.
- Start `fromDoc` before S is in the Map, or while the RR txn is open.
- Durable-write the pin (except later M5 `T0`).
- `git init` / commit (that is [step-flush](#6-step-flush)).

##### Test scenarios

1. **MVCC pin, not live RAM**
   - **Given** persist of text `alpha` (dirty clock T), worker claims and copies S into the Map.
   - **When** hub persist of `beta` lands **after** the cut `COMMIT` (or during convert), then you convert the Map.
   - **Then** markdown has `alpha` and does **not** have `beta`. `dirty.clock` > T (next job).
   - **How:** sidecar + hub testcontainers / Compose. Fail if convert used `GET …/export` or the live `Store`. Fail if bytes were markdown.
2. **Cut released**
   - **Given** the Map is filled and the RR txn committed.
   - **When** convert runs.
   - **Then** no open DB snapshot / `jobs` row lock is held across `fromDoc`. Persist of new updates still `INSERT`s `crdt_update` during convert.
   - **How:** convert after `COMMIT`; optional `SNAPSHOT_CONVERT_SLEEP_MS`. Fail if `fromDoc` requires `FOR SHARE` or an open HTTP export stream.
3. **Not GET**
   - **Given** sidecar pin/collect sources.
   - **When** you search for product collect calling hub `/export`.
   - **Then** that is not the claim path (debug/recon curl may remain in docs).
   - **How:** grep. Fail if step-4.3 collect is `reqwest` to `:3000/api/block/…/export`.

---

### 5. step-dirty-idle

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · [3](#3-step-rust-adapter) · [4](#4-step-pin) · **5** · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                       |
| ------------- | ------------------------------------- |
| **n**         | 5                                     |
| **id**        | `step-dirty-idle`                     |
| **title**     | jobs.not_before idle, Flush, coalesce |
| **dependsOn** | `step-pin-cut`                        |
| **kind**      | implement                             |


**Adds:** debounce and Flush on the `jobs` row ([HA queue](../LiveSnapshot/high-availability.md#queue)). Observer already inserts `idle` jobs in [step-pin-queue](#42-step-pin-queue). This step sets **when** they become due and proves enqueue does **not** fill the pin Map.

#### Work

1. Default `SNAPSHOT_IDLE_MS=60000`. Observer `not_before = first_dirty_at + idle`. `Flush` (`POST /flush`): upsert that wiki’s job to `reason=flush`, `not_before=now()` (may pull an existing idle row forward; do not insert a second row).
2. Second persist upserts `dirty.clock`; **do not** update `dirty_wiki.first_dirty_at`; **do not** reset idle `jobs.not_before`.
3. Inflight **1** is the TTL lease from [step-pin-queue](#42-step-pin-queue). A second Flush while leased does not start a second convert; leftover `dirty.clock` after commit enqueues again.
4. Compose: sidecar `depends_on` postgres. Sidecar DSN is required (reads `dirty` / `dirty_wiki` / `jobs`, writes `jobs` lease + later `last_flushed`). Hub DSN stays hub-only.

#### Do not

- Fill the pin Map on `dirty` upsert, observer insert, or Flush HTTP (pin at **claim**).
- Poll `GET …/export` as the dirty observer.
- Hub inserting `jobs`.
- One job per keystroke (still `UNIQUE (workspace_id)`).
- `FOR UPDATE` on `dirty` across convert.
- In-process `sleep(60s)` as the product queue (the column is `not_before`).

#### Test scenarios

1. **Dirty clock not keystroke**
  - **Given** empty `last_flushed`, hub persist of one WS write.
  - **When** you read `dirty`.
  - **Then** one row for M0 UUID / `PAGE_DOC_ID` with a clock; a second write **upserts** (still one row, clock moved). `dirty_wiki.first_dirty_at` is unchanged on the second write.
  - **How:** reuse hub persist-dirty shape + sidecar select. Fail if sidecar marked dirty by counting editor events.
2. **Idle coalesce**
  - **Given** `SNAPSHOT_IDLE_MS=500`, dirty at t=0 (observer inserts job `not_before≈t+500ms`), another persist at t=100ms.
  - **When** you inspect `jobs`.
  - **Then** **one** row; `not_before` did **not** become t+100+500ms. Still one claim when due.
  - **How:** sidecar test. Fail if each upsert stacks jobs or restarts the window.
3. **Flush now**
  - **Given** an idle job with `not_before` in the future.
  - **When** `POST /flush`.
  - **Then** that row is `reason=flush` and `not_before <= now()`. Pin Map stays empty until a worker **claims**.
  - **How:** sidecar HTTP test. Fail if Flush is a no-op until 60s. Fail if the HTTP handler copied Yjs bytes.
4. **No pin at enqueue**
  - **Given** a `dirty` upsert and a job that is **not** yet due (or due but **unclaimed**).
  - **When** you inspect the sidecar pin Map.
  - **Then** the Map is empty.
  - **How:** sidecar unit test. Fail if observer insert or Flush copied Yjs bytes.

---

### 6. step-flush

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · [3](#3-step-rust-adapter) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · **6** · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                           |
| ------------- | --------------------------------------------------------- |
| **n**         | 6                                                         |
| **id**        | `step-flush`                                              |
| **title**     | Autoinits wiki/; pin then convert; one autocomment commit |
| **dependsOn** | `step-dirty-idle`                                         |
| **kind**      | implement                                                 |


**Adds:** sidecar **creates** `wiki/` (git2 init + catalog dirs) if missing, then `spec/home.md` + sidecar on disk + **one** `git commit` `snapshot: <title>`. Writes Venus `last_flushed`. Dirty blobs in `wiki/assets/` if the pin has an image. Pin Map **dropped** after commit (M5 may keep those bytes as `T0`). Job row released / deleted so leftover dirty can enqueue again. No origin.

#### Work

Follow [LiveSnapshot — pin then convert](../LiveSnapshot/README.md#pin-then-convert) (no LifeIndexing on this cut):

1. **Claim** a due job ([step-pin-cut](#43-step-pin-cut)): cut S into the Map, **COMMIT** the RR txn. Hub apply / broadcast / persist is not paused. Published WYSIWYG is not `readonly`.
2. **Then** convert on the Map only (Rust `fromDoc`). Not `GET …/export`. Not the live Store.
3. **Ensure repo** (after convert, before write): if the wiki dir has no `.git`, `git2` init (default branch Actual) and create parent dirs for `gitPath` / `.venus/ids/` / dirty `assets/`. Idempotent if the repo already exists. **No `origin`.** Not hub workspace create. Not on dirty upsert.
4. Write files → `git add` → `git2` commit. Message `snapshot: Venus` (or Actual title). Author may be a Venus identity (config); not a human why. Snapshot class only.
5. Sidecar JSON `{ docId, clock, blocks }` at api-map path. Clock is **pin** clock (`dirty.clock` at cut).
6. `INSERT/UPDATE last_flushed` `{ clock: T, git_sha }`. If `dirty.clock > T`, leave dirty (next observer tick inserts a new job once this job row is gone). Do not `DELETE` hub `dirty` from persist path (observer ignores `clock <= last_flushed`). Delete or complete the `jobs` row. **Drop the pin Map.**
7. Idempotent: second claim with same pin clocks → empty diff, skip commit **or** identical tree (HA Acceptance #8).
8. Playwright or script: type unique word, Flush (persist then claim), file contains the word.
9. Host **Flush** control (`data-testid="venus-flush"`) when sidecar/env is on. `mount-editor` still ignorant. Flush does **not** set `store.readonly`.

#### Do not

- Required review message.
- Per-block commits.
- Markdown rows in Postgres.
- `fromDoc` the tab Store.
- `fromDoc` before the pin Map holds the page (and dirty blobs).
- `store.readonly` / freeze the published page (lease / M5).
- Keep the pin Map after commit (except later M5 `T0`).
- Enqueue LifeIndexing / LLM on this cut (must not delay `last_flushed`).
- Require a human `git init` before Flush.
- `git init` in the hub or on CRDT workspace create.
- `git remote add origin` / pick a git host (after M4).
- Submodule / two remotes as a product feature (after M4).

#### Test scenarios

1. **Autoinit on first snapshot**
  - **Given** no `wiki/.git` (product `.gitignore` already has `/wiki/`), hub + sidecar, seeded page.
  - **When** you Flush (persist has landed; worker claims).
  - **Then** `wiki/` is a git work tree; `git -C wiki log -1 --format=%s` matches `snapshot: Venus` (or Actual title); `spec/home.md` contains `Why Venus`. Product git still ignores `wiki/`. No `origin` required.
  - **How:** sidecar integration / `e2e/m3-flush.spec.ts` with an empty wiki dir. Fail if Flush required a pre-inited repo. Fail if message is empty or a typed why. Fail if commit ran in the browser. Fail if `wiki/` is tracked on the Venus remote.
2. **Sidecar on disk**
  - **Given** that commit.
  - **When** you read `.venus/ids/<docId>.json`.
  - **Then** JSON has `docId`, `clock`, `blocks[]` with `id`/`start`/`end`; markdown body has no per-block id comments.
  - **How:** same test. Fail if sidecar was RAM-only (M2).
3. **Typed word lands**
  - **Given** Compose hub + Vite/web with sync.
  - **When** you type a unique string in the note, wait for persist, Flush.
  - **Then** `wiki/spec/home.md` contains that string.
  - **How:** Playwright `m3-flush.spec.ts`. Fail if the file is still the previous SHA’s text.
4. **Writes after T stay dirty**
  - **Given** a pin at clock T already collected (or Flush with a delayed convert).
  - **When** you type more **after** pin bytes are taken, then convert/commit that pin.
  - **Then** the commit does **not** include the later string; `dirty.clock > T` so a later Flush does.
  - **How:** sidecar test with delayed convert, or two Flushes. Fail if the first commit includes post-pin typing.
5. **Idempotent**
  - **Given** a successful Flush, no new persist.
  - **When** you Flush again.
  - **Then** git does not add a second distinct snapshot of the same tree (no-op or identical; history does not spam). Same `.git` (did not init a second history).
  - **How:** `git -C wiki rev-list --count HEAD` does not increase, **or** the second commit is documented as skipped. Fail if two autocomments for zero clock movement. Fail if `git init` ran again as a new repo.
6. **Drop pin after commit**
  - **Given** a successful Flush (`last_flushed` set).
  - **When** you inspect the sidecar pin Map.
  - **Then** it has no entry for `doc:home` (dropped; not kept as lease `T0`).
  - **How:** sidecar test. Fail if pin bytes remain after commit ([README — pin then convert](../LiveSnapshot/README.md#pin-then-convert) step 5).

---

### 7. step-live-during-flush

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · [3](#3-step-rust-adapter) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · **7** · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                       |
| ------------- | ----------------------------------------------------- |
| **n**         | 7                                                     |
| **id**        | `step-live-during-flush`                              |
| **title**     | Typing during convert still syncs; persist not paused |
| **dependsOn** | `step-flush`                                          |
| **kind**      | implement                                             |


**Adds:** proof of the LiveSnapshot invariant. Convert may be slow; tabs must not freeze.

#### Work

1. Test hook: delay convert (e.g. `SNAPSHOT_CONVERT_SLEEP_MS=3000`) **after** pin Map is filled.
2. Playwright: two tabs; start Flush; during sleep, A types `during-flush`; B sees it **without** reload (10s). After convert, first commit may omit `during-flush` ([step-flush](#6-step-flush) scenario 4); a later Flush has it.
3. Confirm hub persist still inserts `crdt_update` during the sleep (dirty clock moves).

#### Do not

- `store.readonly` on Flush (that is lease / M5).
- Stop WS apply in the hub for the sleep.
- Skip this step because “convert is fast on seed.”

#### Test scenarios

1. **A→B during convert**
  - **Given** two tabs, sidecar convert delayed ≥2s after pin.
  - **When** Flush starts and A types `during-flush` while convert sleeps; B does not reload.
  - **Then** B’s note contains `during-flush` within 10s.
  - **How:** `e2e/m3-live-during-flush.spec.ts` (or extend two-tabs). Fail if B waits for git. Fail if editors were set readonly.
2. **Persist not paused**
  - **Given** the same delayed Flush.
  - **When** A’s extra typing happens during convert.
  - **Then** after ≥2s, `dirty.clock` is greater than the pin clock T (or a new `crdt_update` landed).
  - **How:** SQL in the spec or sidecar log. Fail if persist was queued behind convert.

---

### 8. step-git-log

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · [3](#3-step-rust-adapter) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · **8** · [9](#9-step-verify)


|               |                                   |
| ------------- | --------------------------------- |
| **n**         | 8                                 |
| **id**        | `step-git-log`                    |
| **title**     | Host chrome: git log for the file |
| **dependsOn** | `step-flush`                      |
| **kind**      | implement                         |


**Adds:** a read-only log of `wiki/spec/home.md` so a person sees autocomment vs (later) comment-commits. Not a Yjs undo timeline. Not `@affine/core`.

#### Work

1. Sidecar (or git in the snapshotter) exposes log for the catalog path (HTTP Actual, e.g. `GET /git/log?path=spec/home.md` → `{ subject, sha }[]`).
2. Host chrome `data-testid="venus-git-log"` lists subjects. After Flush, `snapshot: Venus` is visible. `mount-editor` does not import git.
3. Memory mode: log hidden or empty; no crash.

#### Do not

- History panel of `store.history`.
- Product header undo/redo (M4).
- Fetch log by scraping `git` in the **browser** against the developer’s disk.

#### Test scenarios

1. **Log shows autocomment**
  - **Given** a Flush commit exists, app with sidecar env.
  - **When** you load `/`.
  - **Then** `[data-testid="venus-git-log"]` contains `snapshot: Venus` (or Actual).
  - **How:** `e2e/m3-git-log.spec.ts`. Fail if the list is Yjs undo labels.
2. **Seam holds**
  - **Given** `mount-editor.js` / `editor-container.js` / `boot.js`.
  - **When** you search for git / sidecar / `from-doc` / `pin-from-doc` imports.
  - **Then** none of those files import them.
  - **How:** extend `sync-provider.test.ts` or a sibling. Fail if editor mount talks to git.

---

### 9. step-verify

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-worker) · [3](#3-step-rust-adapter) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · **9**


|               |                                          |
| ------------- | ---------------------------------------- |
| **n**         | 9                                        |
| **id**        | `step-verify`                            |
| **title**     | Close-out: clone wiki/ and read markdown |
| **dependsOn** | `step-live-during-flush`, `step-git-log` |
| **kind**      | implement                                |


**Adds:** nothing new in the product. Marks the board `done`. Person + clone + smoke.

#### Work

1. Runbook: Compose `postgres` + `hub` + `web` + `sidecar`; Flush; clone `wiki/` to `/tmp/venus-wiki-clone`.
2. Person: type, Flush, read clone markdown, second tab during Flush, git log chrome, no review prompt.
3. `pnpm test:e2e:m1`, M2 pane, new `m3-*.spec.ts`.
4. Mark [M3.state.yaml](./M3.state.yaml) steps 1–9 `done` with evidence.

#### Do not

- Start M4 catalog CRDT in this step.
- Close M3 if clone still needs the live editor to “see” the page.
- Treat AB1 LifeIndexing as required.

#### Test scenarios

1. **Clone elsewhere**
  - **Given** a Flush after typing a unique word.
  - **When** you `git clone wiki /tmp/venus-wiki-clone` (or `git clone` the nested repo path) and open `spec/home.md`.
  - **Then** the file is readable markdown containing that word and seed headings. No Postgres, no hub required to read it.
  - **How:** shell in verify / documented script. Fail if the clone is Yjs binaries.
2. **Smoke**
  - **Then** `m3-flush`, `m3-live-during-flush`, `m3-git-log` pass; `pnpm test:e2e:m1` and M2 pane still pass.
  - **How:** `pnpm test:e2e:m3` (or Actual) + existing suites.
3. **Manual path**
  - **Then** runbook M3 close-out holds (type, Flush, clone, two tabs during Flush, log shows autocomment, no review why).
  - **How:** person in Chrome or Firefox. Yaml: date + browser.

---

## After M3

[M4 — Folder tree + links + product header](../M4/README.md) ([plan](../M4/plan.md)): catalog CRDT, tree UI, `git mv`, header undo/redo. M3 still one page and a constant `gitPath`. **Gate:** this plan **closed**.

Lease [M5](../venus-implementation-plan.md#m5--lease--freeze-week) reuses this pin + convert (`T0`). Do not keep pins after commit in M3 except as git files.

Parallel (**AB1**, not this exit): [LifeIndexing](../Agents/LifeIndexing.md) may gist/tag/graph dirty pages at the SHA **after** `last_flushed`. Do not put an LLM on convert.

## Order of work (calendar)


| When  | Steps                                                                                 |
| ----- | ------------------------------------------------------------------------------------- |
| Day 1 | 1 `step-recon-snapshot`                                                               |
| Day 2 | 2 `step-worker`                                                                       |
| Day 3 | 3 `step-rust-adapter` (goldens + large-file bench)                                    |
| Day 4 | 4.1 `step-pin-schema` → 4.2 `step-pin-queue` → 4.3 `step-pin-cut` → 5 `step-dirty-idle` |
| Day 5 | 6 `step-flush` (autoinit) → 7 `step-live-during-flush`                                |
| Day 6 | 8 `step-git-log` → 9 `step-verify`                                                    |


If M3.0 is still open, **stop**. Do not thin the hub into a git writer to save a crate.

## Risks


| Risk                                    | What to do in M3                                                |
| --------------------------------------- | --------------------------------------------------------------- |
| Gate skipped                            | Step 1 **Gate held** fails closed                               |
| Convert in the hub                      | Step 2 **Not in hub**; HA invariant                             |
| `fromDoc` live Store                    | Step 4.3 **MVCC pin**; Path B only                              |
| Pin at dirty upsert / enqueue           | Step 5 **No pin at enqueue**; pin at **claim**                  |
| Pin from GET / hub RAM                  | Step 4.3 **Not GET**; cut is SQL `REPEATABLE READ` of S         |
| Persist lag (RAM ahead of SQL)          | Inherent. Next job if `dirty.clock > T`. Do not pause persist.  |
| Idle reset every keystroke              | Step 5 coalesce (`first_dirty_at` / `not_before`)               |
| Tabs freeze / `store.readonly` on Flush | Step 7 delay convert; two-tabs must pass; Flush does not freeze |
| Nested wiki committed to product git    | Recon `.gitignore`; Flush **Autoinit** must still ignore        |
| Flush needs a pre-inited `wiki/`        | Step 6 **Autoinit on first snapshot**                           |
| Second adapter dialect                  | Step 3: Rust **byte-identical** to JS oracle or the step fails  |
| Convert bench skipped                   | Step 3 **Large file wall time** must record both variants       |
| Kafka / Redis “for HA”                  | Forbidden; queue is Venus `jobs`                                |
| Trigger writes `jobs`                   | Step 4.1 **Trigger marks dirty_wiki, not jobs**                 |
| Two consumers, one wiki, two converts   | Step 4.2 **SKIP LOCKED** + TTL lease                            |
| Keep pin after commit as `T0`           | Drop Map in step 6; M5 keeps the pin                            |
| Origin / git host in M3                 | Local clone only; remotes after M4                              |
| AB1 on the cut                          | Not this exit; must not delay commit                            |


## Handoff to M4

M4 may assume:

- `wiki/` exists **after the first snapshot** (sidecar autoinit); snapshot autocomment works for `doc:home`. No origin required.
- `dirty_wiki` + `jobs` + `last_flushed` exist. Cut is MVCC. Convert engine is Rust `fromDoc` (JS CLI oracle).
- Catalog is a **constant path**, not a CRDT. Tree UI and `git mv` are new. A second page is a second `dirty` row on the same `jobs.workspace_id` (one wiki job still).
- Lease / review / apply are **not** done.
- Hub still has no convert and does not write `jobs`.

M4 exit is two pages, a link, a folder move, git tree matches ([M4/plan.md](../M4/plan.md)). M3 exit is “clone `wiki/` and read markdown; typing during flush still syncs.”

## Invariants (M3 only)

1. One workspace, one page **as load**. Catalog v0 = one `gitPath`. Schema is wiki-grained (`jobs.workspace_id`).
2. Live CRDT never waits on markdown, git, or convert. Freeze (`store.readonly`) is lease-only.
3. **Two buffers.** Live generation (hub apply / broadcast / persist) always. Pin generation (sidecar Map) only after claim. Drop the pin after commit.
4. Dirty is `{ clock }` on `(workspace_id, docId)`, not keystrokes. Enqueue is `dirty_wiki` → `jobs`. Inflight **1** = TTL lease. Trigger never writes `jobs`.
5. Pin starts when a worker **claims**, not at dirty upsert / observer insert / Flush HTTP.
6. Collect every pin (page + dirty blobs) **before** any `fromDoc`. COMMIT the MVCC cut first. Path B on the pin, not the live Store, not GET export, not the spectator splice.
7. Snapshotter beside the hub. Not in the hub. Not in the editor tab. Sidecar autoinits `wiki/` on first snapshot; no origin in M3.
8. No markdown in Postgres. No per-block commits. No review why. Queue is Venus `jobs`, not Kafka.
9. `mount-editor` stays unaware of git and mdgate convert.
10. Outline remains in-page headings (not a wiki tree).

