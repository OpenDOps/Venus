# M3 — Git snapshotter


|               |                                                                                          |
| ------------- | ---------------------------------------------------------------------------------------- |
| **planId**    | `m3-git-snapshotter`                                                                     |
| **Milestone** | [M3 in the implementation plan](../venus-implementation-plan.md#m3--git-snapshotter-week) |
| **Duration**  | About a week                                                                             |
| **Encoding**  | Headings + tables ([venus-plan.md](../../drafts/pre-design/venus-plan.md) option B)      |
| **Board**     | [M3.state.yaml](./M3.state.yaml)                                                         |


Parent design: [venus-design.md](../venus-design.md). Pin then convert: [LiveSnapshot](../LiveSnapshot/README.md). Scale / thin-column checklist: [LiveSnapshot HA](../LiveSnapshot/high-availability.md) ([M3 must keep this shape](../LiveSnapshot/high-availability.md#m3-must-keep-this-shape), [Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3)). Convert helper: [pin-convert.md](../MDGate/pin-convert.md). Git tree: [datamodel git](../datamodel/git.md). Hub (live CRDT): [M3.0](../M3.0/README.md), [M3.0 HA](../M3.0/high-availability.md). Exporter: [M2](../M2/README.md), [MDGate](../MDGate/README.md). Dataflow: [architecture.md](../architecture.md). Words: [glossary.md](../glossary.md). Tool choices: [venus-implementation-plan.md](../venus-implementation-plan.md). Installed symbols: [api-map.md](../api-map.md).

This is a **design-folder plan**. The spec-wiki lease/DoD runner is not built yet. DoD scenarios below are the accept rules for the code; they are not a leased wiki page.

**Gate (do not skip):** do not start step work in the product repo until [M3.0](../M3.0/README.md) is **closed** (board steps 1–9 `done`) **and** [LiveSnapshot HA Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) is **accepted**. This folder existing is not permission to write `wiki/` or a snapshotter process.

## Story

As an implementer I need casual WYSIWYG to become **cloneable markdown** without a review ceremony: a Venus **snapshotter beside the hub** pins dirty Yjs bytes, runs the same `fromDoc` as M2 on that pin, and writes **one** git commit with autocomment `snapshot: <title>`. Two tabs still sync while that flush runs. Venus still has no lease freeze, catalog tree, or comment-commit why.

If we `fromDoc` the live Store, git is a moving photograph. If convert sits in the hub, live collab waits on markdown. If we commit every keystroke, git is not a snapshot.

## Exit

All of these must be true at once:

1. **`wiki/`** is a real git repository (nested, not the product remote). It contains the M0 page as markdown at the catalog-v0 path (`spec/home.md` unless api-map Actual says otherwise) plus `.venus/ids/<docId>.json`.
2. Dirty is **clocks** (M3.0 `dirty` row vs last flushed), not keystrokes. One page. One pending flush at a time.
3. Idle (30–120s, Actual default **60s**) and/or **Flush** pin Yjs update v1 (**idle GET export after ≥ persist batch**, or replica encode — Actual in recon), **then** `fromPinnedBytes` / `pinThenFromDoc` on that pin, **then** one `git commit` with autocomment `snapshot: <title>`. Convert the pin, not the live `Store`.
4. Cut is released **before** `fromDoc`. Hub apply / broadcast / persist is **not** paused. Typing during flush still appears in the other tab without reload (M1 two-tabs still green while a flush is in flight).
5. Clone `wiki/` **elsewhere** and read the page as ordinary markdown (seed headings survive; a typed unique word after Flush is in the file).
6. Casual WYSIWYG did **not** require a review comment. Commit message is autocomment, not a why.
7. Host UI: `git log` for that file shows the autocomment (later comment-commits will sit on the same log; none in M3).
8. Snapshotter is **`crates/venus-sidecar`** (Rust + y-octo hydrate + M2 `from-doc.js` via Node or embedded JS + **git2**). It is **not** in `crates/venus-hub`. Browser tab does not `git commit`.
9. No markdown in Postgres. No `jobs` from the hub. No Akka/Kafka/Redis queue. No per-block commits.
10. [api-map.md](../api-map.md) Actual column is filled for every snapshotter name the code uses.
11. Existing **M1 Playwright** (`pnpm test:e2e:m1`) and M2 export / pane tests stay green.

## Non-goals (do not start)


| Later                                                        | Why not M3                                                                                    |
| ------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Folder tree, catalog CRDT, `git mv`, product header          | M4                                                                                            |
| Lease freeze, CodeMirror, flush-before-lease as `T0` product | M5 (same pin helper; this milestone does not keep pins for a lease)                           |
| Comment-commit, After/Before/Diff, required why              | M6                                                                                            |
| Apply / hunks / `ap-*` fixtures                              | M6 — [apply.md](../MDGate/apply.md)                                                           |
| `jobs` table, `dirty_wiki`, `SKIP LOCKED` fleet, many wikis  | [LiveSnapshot HA](../LiveSnapshot/high-availability.md) scale; M3 is the thin column          |
| Hub HPA / gateway / many hub replicas                        | [hub-fleet.md](../../devops/hub-fleet.md)                                                     |
| LifeIndexing gists / graph (AB1)                             | Parallel after this exit; must not delay `last_flushed`                                       |
| Bound chat (AB2), chat-edit (AB3), history/why pack (AB4)    | After later milestones                                                                        |
| `.venus/snapshots/*.bin`                                     | M8 optional                                                                                   |
| Markdown as Y.Text                                           | Forbidden ([datamodel](../datamodel/README.md))                                               |
| Convert / `fromDoc` inside the hub                           | Hub stays apply + broadcast + persist + export/blob                                           |


Do not commit from the editor tab. Do not `fromDoc` every keystroke. Do not write sidecar JSON to Postgres.

## Constraints

1. **Thin host.** Same Vite + React app. Flush + git log are host chrome (like the markdown pane), not BlockSuite widgets. `mount-editor.js` does not import git or the sidecar.
2. **Thin column of HA.** Allowed degeneration is the table in [M3 must keep this shape](../LiveSnapshot/high-availability.md#m3-must-keep-this-shape). Must not invert it. If this plan disagrees with [LiveSnapshot HA](../LiveSnapshot/high-availability.md), **the HA file wins**.
3. **Path B convert.** Git uses `fromPinnedBytes` / `pinThenFromDoc` ([pin-convert.md](../MDGate/pin-convert.md)). Same `from-doc.js` as the pane. Do not pass `incrementalFromDoc` as the commit. Do not `fromDoc` the live published Store.
4. **Pin source (M3 thin).** Idle: wait ≥ persist batch (**≥2s**), then `GET` hub **Export command** (api-map). Replica encode is allowed if recon records it as Actual; a hidden AFFiNE client per page is not required to close M3. Flush uses the same pin source (wait ≥2s then GET, or replica). This is a **best-effort** cut, not MVCC `REPEATABLE READ` of many spaces.
5. **Dirty.** Product mark is already the M3.0 SQL trigger on `crdt_update`. Snapshotter **reads** `dirty(workspace_id, doc_id, clock)`. It does not poll every workspace with export. Hub does **not** write `jobs`. M3 does **not** add `dirty_wiki` or a `jobs` table (in-process idle + Flush is the queue).
6. **One collection, one page.** Workspace UUID `77e4a2b1-8b40-5979-a73c-fd4477216d00` / BlockSuite `doc:home` (SQL `PAGE_DOC_ID`). Catalog v0 is a **constant** `gitPath`, not a catalog Y.Doc.
7. **One process, one `wiki/`.** Observer + worker may share `venus-sidecar`. Do not link convert into the hub binary. Do not share a working tree across processes.
8. **Idle debounce** is **60s** from first dirty (inside the 30–120s window). Flush / tests may set `not_before = now` (env Actual, e.g. `SNAPSHOT_IDLE_MS`). Do not reset idle `not_before` on a second keystroke.
9. **`last_flushed`.** RAM in-process is the M3 thin store. Crash recovery reads the git sidecar `clock` (and git HEAD). Do not require a new Venus table to close M3. After commit, if `dirty.clock` > pin clock **T**, the page stays dirty (next idle/Flush).
10. **Pin `yjs` 13.6.32** and BlockSuite **0.22.4**. Worker hydrate is **y-octo**. Node `Y.applyUpdate` is recon/tests only for convert proof, not Compose `sidecar` apply of the live room.
11. **Memory default.** Vitest and `pnpm test:e2e` stay green without Docker. M3 e2e / Flush require Compose **hub** (+ sidecar). Unset sync env: no git write.
12. **Docker is the runtime** for the snapshotter DoD (Compose `sidecar` or documented `pnpm` spawn against hub). Host `cargo run` is recon only.
13. **No `@affine/core`.** No nbstore. No convert in the hub image.

## Target tree

Only create what M3 needs. Do **not** add catalog packages, review types, or apply tests. Do **not** put git2 in `crates/venus-hub`.

```text
Venus/
  docker-compose.yml                 # postgres + hub + web + sidecar
  deploy/
    sidecar/Dockerfile               # venus-sidecar: y-octo + Node from-doc + git2
  crates/venus-sidecar/              # product convert worker (not the hub)
  wiki/                              # nested git repo (product .gitignore)
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
Tab A / Tab B  (BlockSuite Store → Y.Doc)
        │  y-protocols/sync   (unchanged; never waits)
        ▼
Venus hub  (Compose `hub`)  apply + broadcast + persist ~1s
        │  AFTER persist UPSERT dirty     (already M3.0)
        ▼
Postgres  crdt_* + blob + dirty

venus-sidecar  (Compose `sidecar`, beside the hub)
        │  read dirty vs last_flushed (RAM / sidecar clock)
        │  idle 60s or Flush
        │  CUT: wait ≥2s → GET …/export  (or replica encode)
        │  pin Map { bytes, clock }      ← frozen copy only
        │  release cut
        │  y-octo hydrate + fromPinnedBytes (from-doc.js)
        ▼
wiki/  spec/home.md + .venus/ids/<docId>.json
        │  git2 one commit  message: snapshot: <title>
        ▼
last_flushed = T   (RAM; sidecar clock is crash recovery)
```

Ids stay:

```text
TestWorkspace.id  =  77e4a2b1-8b40-5979-a73c-fd4477216d00
doc:home          =  store.spaceDoc; SQL PAGE_DOC_ID; dirty.grain
gitPath           =  spec/home.md     (catalog v0 constant)
```

## Chosen stack

Locked in [step-recon-snapshot](#1-step-recon-snapshot). If this section disagrees with [api-map.md](../api-map.md), **the map wins**.


| Piece       | Intent (Actual in recon)                                                                                          |
| ----------- | ----------------------------------------------------------------------------------------------------------------- |
| Live CRDT   | Unchanged: hub + `OctoBaseKeckProvider` (`kind: 'octobase'` alias)                                                |
| Snapshotter | **`crates/venus-sidecar`**, Compose `sidecar`. Rust + **y-octo** hydrate + **git2**. Not in the hub.              |
| Convert JS  | Same `from-doc.js` / `fromPinnedBytes`. Host: Node CLI or embed (Actual). Pane `splice.js` is **not** on this path |
| Pin source  | Idle/Flush: hub `GET …/export` after ≥2s **or** replica encode. Not the tab’s `Store`.                            |
| Dirty       | Postgres `dirty` (M3.0 trigger). Observer in sidecar. No `jobs` table.                                            |
| Queue       | In-process idle timer + Flush. One inflight per wiki.                                                             |
| Git         | Nested `wiki/`, **git2**, autocomment `snapshot: <title>`. Node `simple-git` is recon-only.                       |
| Catalog v0  | Constant: `doc:home` → `spec/home.md`. No catalog CRDT.                                                           |
| Out of scope | Lease UI, `jobs` fleet, convert in hub, markdown in SQL, AB1 LLM                                                   |


### Pin and git


|             |                                                                                                                                      |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| **Idle**    | 60s from `dirty.first_dirty_at` (env override for tests). Second edit upserts `dirty.clock`; does not restart the timer.             |
| **Flush**   | Immediate (after ≥2s persist wait if pin source is GET). Host chrome.                                                                |
| **Cut**     | Best-effort: persist has flushed, then export/replica. Not `FOR UPDATE` on `crdt_*`.                                                 |
| **Commit**  | One commit per flush. Message `snapshot: <title>` (page title, seed `Venus`). Coalesce all WYSIWYG since last SHA.                   |
| **Blobs**   | Copy dirty blobs into `wiki/assets/` only if the pin’s page references them (M2 opaque image form).                                  |
| **Crash**   | Pin Map gone. Retry from `dirty` vs sidecar clock / missing `last_flushed`. Git HEAD is last successful commit.                      |


## Steps summary

What each step **adds** to the product (not how to test it — that is under each step).


| #   | id                                                    | Adds                                                                                          |
| --- | ----------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| 1   | [`step-recon-snapshot`](#1-step-recon-snapshot)       | Gate + map: pin source, worker, git tree, idle, JS embed; spike export → `fromPinnedBytes`.   |
| 2   | [`step-wiki-init`](#2-step-wiki-init)                 | Nested `wiki/` repo; catalog v0 path; product git ignores it.                                 |
| 3   | [`step-worker`](#3-step-worker)                       | `crates/venus-sidecar`: y-octo hydrate + same exporter; **no** git commit yet.                |
| 4   | [`step-pin`](#4-step-pin)                             | Pin Map from idle GET (or replica). Not the live Store. Cut released before convert.          |
| 5   | [`step-dirty-idle`](#5-step-dirty-idle)               | Read `dirty`; RAM `last_flushed`; in-process idle 60s + Flush; one job.                       |
| 6   | [`step-flush`](#6-step-flush)                         | Write markdown + sidecar (+ dirty blobs); one autocomment commit; clocks.                     |
| 7   | [`step-live-during-flush`](#7-step-live-during-flush) | Typing during convert still syncs A→B; hub persist not paused.                                |
| 8   | [`step-git-log`](#8-step-git-log)                     | Host chrome: `git log` for `spec/home.md` (autocomment visible).                              |
| 9   | [`step-verify`](#9-step-verify)                       | Close-out: clone elsewhere; person + Playwright; board `done`.                                |


---

## Steps

Do them in order (1–9). A step is not started until its `dependsOn` steps are done. Test scenarios under each step are the accept rules (Given / When / Then). Encode them as tests where the How column names a command; do not invent extra scenarios.

### 1. step-recon-snapshot

[Back to overall summary](#steps-summary). Steps: **1** · [2](#2-step-wiki-init) · [3](#3-step-worker) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                                 |
| ------------- | --------------------------------------------------------------- |
| **n**         | 1                                                               |
| **id**        | `step-recon-snapshot`                                           |
| **title**     | Map pin source, worker, git tree, idle, JS embed                |
| **dependsOn** | M3.0 closed; [LiveSnapshot HA Acceptance](../LiveSnapshot/high-availability.md#acceptance-gate-for-m3) accepted |
| **kind**      | implement                                                       |


**Adds:** a decision and a map, not `wiki/` commits. You know which process writes git, how it gets pin bytes, how it runs `from-doc.js`, and that the hub is not that process.

M3 dies if convert is bolted onto `handle_socket`, or if git photographs the live tab.

#### Work

1. Confirm the **gate** on the board: M3.0 steps 1–9 `done`; HA Acceptance table accepted (date + who on [M3.state.yaml](./M3.state.yaml) evidence or the HA file). **Stop** if either is open.
2. Spike (throwaway ok): hub up, seed `doc:home`, wait ≥2s, api-map **Export command**, `fromPinnedBytes` (or `hydrateM0FromUpdate` + `fromDoc`). Log markdown contains seed H1. **No** `git commit`. Prove this does not import `splice.js`.
3. Decide **pin source** (GET after ≥2s vs replica encode). Write it in [api-map.md](../api-map.md).
4. Decide **how Rust hosts JS** (`from-doc.js`): Node CLI vs embed. Record the command/binary. Do not reimplement `MarkdownAdapter`.
5. Decide **gitPath**, sidecar filename (`doc:home` vs `PAGE_DOC_ID`), idle env name, Compose service name (`sidecar`), Flush HTTP or stdio.
6. Fill [api-map.md](../api-map.md) **Names — git snapshotter** Actuals. Record: one inflight per wiki; crash recovery = sidecar clock; no `jobs` in M3.

#### Do not

- Write `wiki/` as product output (step 2).
- Import git into `mount-editor.js` or `crates/venus-hub`.
- Start the Flush UI.
- Add `jobs` / `dirty_wiki` tables.
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
   - **Then** these are concrete: pin source, export/replica command, sidecar crate / Compose service, convert JS command, `gitPath`, sidecar path, idle default, Flush trigger, `last_flushed` recovery.
   - **How:** read the file. Fail if cells are empty or `recon:`.
3. **Spike convert**
   - **Given** hub + Postgres, seeded `doc:home` (Compose or testcontainers).
   - **When** you export (wait ≥2s) and run Path B convert with **no** git.
   - **Then** markdown contains `Why Venus`; the convert module graph does **not** import `splice.js` / `mount-md-pane.js`.
   - **How:** Vitest or `cargo test -p venus-sidecar` recon. Fail if convert used the live Store in a browser tab.

---

### 2. step-wiki-init

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · **2** · [3](#3-step-worker) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                    |
| ------------- | ---------------------------------- |
| **n**         | 2                                  |
| **id**        | `step-wiki-init`                   |
| **title**     | Nested wiki/ repo, catalog v0 path |
| **dependsOn** | `step-recon-snapshot`              |
| **kind**      | implement                          |


**Adds:** a git working tree Venus can commit into. One folder, one doc, fixed path. Product git does not absorb wiki history.

#### Work

1. `git init` in `wiki/` (nested). Default branch Actual (e.g. `main`).
2. Catalog v0: directory `spec/`, future file `home.md` (or api-map path). Empty tree or a `.gitkeep` is enough; do not hand-write a fake export as the published page.
3. Venus root `.gitignore`: `/wiki/` (keep the nested repo off the product remote). Document in runbook: wiki is a **separate** git.
4. Record `gitPath` = `spec/home.md` in api-map if not already.

#### Do not

- Submodule / two remotes as a product feature (after M4).
- Catalog Y.Doc, tree UI, `git mv`.
- Commit markdown from a host script that `fromDoc`s the live Store.

#### Test scenarios

1. **Nested repo**
   - **Given** this repo after the step.
   - **When** you `git -C wiki rev-parse --is-inside-work-tree` and `git check-ignore -v wiki`.
   - **Then** `wiki/` is a git work tree, and the **product** git ignores it.
   - **How:** a small Vitest/shell in sidecar or `apps/web`. Fail if `wiki/` is tracked on the Venus remote.
2. **Catalog v0 path**
   - **Given** api-map **gitPath**.
   - **When** you read it.
   - **Then** it is one folder + one file (`spec/home.md` unless Actual differs), mapped to `doc:home` only.
   - **How:** grep api-map. Fail if a second page path exists “for later.”

---

### 3. step-worker

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · **3** · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                          |
| ------------- | -------------------------------------------------------- |
| **n**         | 3                                                        |
| **id**        | `step-worker`                                            |
| **title**     | venus-sidecar: y-octo hydrate + from-doc.js (no git yet) |
| **dependsOn** | `step-wiki-init`                                         |
| **kind**      | implement                                                |


**Adds:** the convert process. It hydrates pin bytes with y-octo and produces `{ markdown, sidecar }` with the M2 exporter. It does not commit.

#### Work

1. Workspace member `crates/venus-sidecar`. **git2** may be a dep already; do not call commit yet if that keeps the crate smaller — adding git2 without using it is fine. Hydrate: y-octo `apply_update_from_binary_v1` / `try_from_binary_v1`.
2. Invoke Path B: the recon **convert JS command** (Node CLI importing `fromPinnedBytes`). Goldens: seed H1/H2 match M2 `fromDoc` (modulo subset whitespace).
3. `deploy/sidecar/Dockerfile` **or** a documented host binary for later Compose. Health may be “process boots.” Hub Dockerfile must **not** COPY this crate as the hub entrypoint.
4. Static check: `crates/venus-hub` does not depend on `venus-sidecar` / `git2` / `from-doc.js`.

#### Do not

- `fromDoc` inside the hub HTTP handler.
- A second MarkdownAdapter in Rust.
- `simple-git` as the product writer.

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

### 4. step-pin

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · [3](#3-step-worker) · **4** · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                              |
| ------------- | -------------------------------------------- |
| **n**         | 4                                            |
| **id**        | `step-pin`                                   |
| **title**     | Pin Map from export or replica, not the Store |
| **dependsOn** | `step-worker`                                |
| **kind**      | implement                                    |


**Adds:** `{ bytes, clock }` in sidecar RAM for `doc:home`. Convert runs on that copy after the GET/replica returns (cut released). Live hub RAM is not frozen.

#### Work

1. Implement pin collect: wait ≥2s after a known persist **or** replica encode (Actual). `GET` api-map Export command. Store `Map<docId, { bytes, clock }>`. Clock = sidecar encoding (lib0 state vector / documented Actual), not wall time.
2. Convert **only** from the Map (`fromPinnedBytes`). Drop the HTTP client / replica handle before convert if that is the cut; do not hold a DB transaction (M3 GET has none).
3. Unit test: mutate a **live** Y.Doc after pin copy; convert still matches the **pin**, not the later mutation.

#### Do not

- `fromDoc(session.store)` in the browser for git.
- Pause hub persist while copying.
- `FOR UPDATE` on `crdt_update`.
- Durable-write the pin (except later M5 `T0`).

#### Test scenarios

1. **Pin from export**
   - **Given** hub up, seed present, wait ≥2s.
   - **When** sidecar pins via Actual pin source.
   - **Then** `bytes.length > 2` and `clock` is non-empty.
   - **How:** sidecar test against hub or recorded fixture. Fail if bytes were `fromDoc` markdown.
2. **Not live Store**
   - **Given** a pin of markdown containing `alpha` only (fixture Yjs or export).
   - **When** you apply a later update `beta` to a **different** live doc, then convert the pin.
   - **Then** markdown has `alpha` and does **not** have `beta`.
   - **How:** unit test. Fail if convert read the live Store.
3. **Cut released**
   - **Given** pin collect uses GET.
   - **When** convert runs.
   - **Then** convert does not hold an open HTTP stream / DB txn as a lock across `fromDoc` (GET has completed; bytes only in the Map).
   - **How:** code review + test that convert works with the hub **stopped** after bytes are in the Map. Fail if `fromDoc` requires the hub to stay up.

---

### 5. step-dirty-idle

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · [3](#3-step-worker) · [4](#4-step-pin) · **5** · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                     |
| ------------- | --------------------------------------------------- |
| **n**         | 5                                                   |
| **id**        | `step-dirty-idle`                                   |
| **title**     | Dirty clocks, idle timer, Flush, one inflight       |
| **dependsOn** | `step-pin`                                          |
| **kind**      | implement                                           |


**Adds:** the thin queue. Sidecar watches Postgres `dirty` for the M0 workspace / `PAGE_DOC_ID`, compares to `last_flushed` (RAM, missing = dirty), waits 60s or Flush. Does not commit yet — may call pin+convert into memory to prove scheduling. Prefer: schedule only; write git in step 6. If easier, idle may no-op until step 6 as long as tests assert **when** a flush **would** start.

#### Work

1. Read `dirty` for workspace UUID + `PAGE_DOC_ID`. Ignore rows with `clock <= last_flushed`.
2. In-process: if dirty and no inflight, start idle timer (`SNAPSHOT_IDLE_MS` default 60000). Flush host control sets “due now.”
3. Second persist upserts `dirty.clock`; **do not** restart idle `not_before`.
4. Inflight **1**: ignore overlapping Flush/idle until the current run finishes (or queue one follow-up — do not stack N commits).
5. Compose: sidecar `depends_on` postgres (and hub if pin is GET). Sidecar DSN may be a read role; must not be omitted if it reads `dirty`.

#### Do not

- Poll `GET …/export` on a timer as the dirty **observer**.
- Hub inserting `jobs`.
- One job per keystroke.
- `FOR UPDATE` on `dirty` across convert.

#### Test scenarios

1. **Dirty clock not keystroke**
   - **Given** empty `last_flushed`, hub persist of one WS write, wait ≥2s.
   - **When** sidecar reads `dirty`.
   - **Then** one row for M0 UUID / `PAGE_DOC_ID` with a clock; a second write **upserts** (still one row, clock moved).
   - **How:** reuse hub `persist_after_ws_upserts_dirty_clock_not_jobs` shape + sidecar select. Fail if sidecar marked dirty by counting editor events.
2. **Idle coalesce**
   - **Given** `SNAPSHOT_IDLE_MS=500` (or fake time), dirty at t=0, another persist at t=100ms.
   - **When** you wait until due.
   - **Then** **one** flush run is scheduled, not two. Timer did not reset to +500ms from the second write.
   - **How:** sidecar unit test. Fail if each upsert starts a new 500ms window that stacks jobs.
3. **Flush now**
   - **Given** dirty row, idle not yet due.
   - **When** Flush is triggered (HTTP/stdio Actual).
   - **Then** a run starts without waiting the idle remainder.
   - **How:** sidecar test. Fail if Flush is a no-op until 60s.

---

### 6. step-flush

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · [3](#3-step-worker) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · **6** · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                              |
| ------------- | ------------------------------------------------------------ |
| **n**         | 6                                                            |
| **id**        | `step-flush`                                                 |
| **title**     | Pin then convert: one autocomment commit                     |
| **dependsOn** | `step-dirty-idle`                                            |
| **kind**      | implement                                                    |


**Adds:** `wiki/spec/home.md` + sidecar on disk + **one** `git commit` `snapshot: <title>`. `last_flushed = T`. Dirty blobs in `wiki/assets/` if the pin has an image.

#### Work

1. On due job: pin (step 4) → convert (step 3) → write files → `git add` → `git2` commit. Message `snapshot: Venus` (or Actual title). Author may be a Venus identity (config); not a human why.
2. Sidecar JSON `{ docId, clock, blocks }` at api-map path. Clock is **pin** clock.
3. Set in-process `last_flushed`. If `dirty.clock > T`, leave dirty (next run). If equal, RAM last_flushed matches; do not `DELETE` hub `dirty` from persist path (observer ignores `clock <= last_flushed`).
4. Idempotent: second flush with same pin clocks → empty diff, skip commit **or** identical tree (HA step 8).
5. Playwright or script: type unique word, Flush (wait persist), file contains the word.
6. Host **Flush** control (`data-testid="venus-flush"`) when sidecar/env is on. `mount-editor` still ignorant.

#### Do not

- Required review message.
- Per-block commits.
- Markdown rows in Postgres.
- `fromDoc` the tab Store.

#### Test scenarios

1. **Autocomment commit**
   - **Given** hub + sidecar + nested `wiki/`, seeded page.
   - **When** you Flush (after ≥2s if GET).
   - **Then** `git -C wiki log -1 --format=%s` matches `snapshot: Venus` (or Actual title). `spec/home.md` contains `Why Venus`.
   - **How:** sidecar integration / `e2e/m3-flush.spec.ts`. Fail if message is empty or a typed why. Fail if commit ran in the browser.
2. **Sidecar on disk**
   - **Given** that commit.
   - **When** you read `.venus/ids/<docId>.json`.
   - **Then** JSON has `docId`, `clock`, `blocks[]` with `id`/`start`/`end`; markdown body has no per-block id comments.
   - **How:** same test. Fail if sidecar was RAM-only (M2).
3. **Typed word lands**
   - **Given** Compose hub + Vite/web with sync.
   - **When** you type a unique string in the note, wait ≥2s, Flush.
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
   - **Then** git does not add a second distinct snapshot of the same tree (no-op or identical; history does not spam).
   - **How:** `git -C wiki rev-list --count HEAD` does not increase, **or** the second commit is documented as skipped. Fail if two autocomments for zero clock movement.

---

### 7. step-live-during-flush

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · [3](#3-step-worker) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · **7** · [8](#8-step-git-log) · [9](#9-step-verify)


|               |                                                            |
| ------------- | ---------------------------------------------------------- |
| **n**         | 7                                                          |
| **id**        | `step-live-during-flush`                                   |
| **title**     | Typing during convert still syncs; persist not paused      |
| **dependsOn** | `step-flush`                                               |
| **kind**      | implement                                                  |


**Adds:** proof of the LiveSnapshot invariant. Convert may be slow; tabs must not freeze.

#### Work

1. Test hook: delay convert (e.g. `SNAPSHOT_CONVERT_SLEEP_MS=3000`) **after** pin Map is filled.
2. Playwright: two tabs; start Flush; during sleep, A types `during-flush`; B sees it **without** reload (10s). After convert, first commit may omit `during-flush` (step 6 scenario 4); a later Flush has it.
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

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · [3](#3-step-worker) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · **8** · [9](#9-step-verify)


|               |                                              |
| ------------- | -------------------------------------------- |
| **n**         | 8                                            |
| **id**        | `step-git-log`                               |
| **title**     | Host chrome: git log for the file            |
| **dependsOn** | `step-flush`                                 |
| **kind**      | implement                                    |


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

[Back to overall summary](#steps-summary). Steps: [1](#1-step-recon-snapshot) · [2](#2-step-wiki-init) · [3](#3-step-worker) · [4](#4-step-pin) · [5](#5-step-dirty-idle) · [6](#6-step-flush) · [7](#7-step-live-during-flush) · [8](#8-step-git-log) · **9**


|               |                                              |
| ------------- | -------------------------------------------- |
| **n**         | 9                                            |
| **id**        | `step-verify`                                |
| **title**     | Close-out: clone wiki/ and read markdown     |
| **dependsOn** | `step-live-during-flush`, `step-git-log`     |
| **kind**      | implement                                    |


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

[M4 — Folder tree + links + product header](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks): catalog CRDT, tree UI, `git mv`, header undo/redo. M3 still one page and a constant `gitPath`.

Lease [M5](../venus-implementation-plan.md#m5--lease--freeze-week) reuses this pin + convert (`T0`). Do not keep pins after commit in M3 except as git files.

Parallel (**AB1**, not this exit): [LifeIndexing](../Agents/LifeIndexing.md) may gist/tag/graph dirty pages at the SHA **after** `last_flushed`. Do not put an LLM on convert.

## Order of work (calendar)


| When  | Steps                                                                 |
| ----- | --------------------------------------------------------------------- |
| Day 1 | 1 `step-recon-snapshot` → 2 `step-wiki-init`                          |
| Day 2 | 3 `step-worker` → 4 `step-pin`                                        |
| Day 3 | 5 `step-dirty-idle`                                                   |
| Day 4 | 6 `step-flush` → 7 `step-live-during-flush`                           |
| Day 5 | 8 `step-git-log` → 9 `step-verify`                                    |


If M3.0 is still open, **stop**. Do not thin the hub into a git writer to save a crate.

## Risks


| Risk                                      | What to do in M3                                                                                          |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Gate skipped                              | Step 1 **Gate held** fails closed                                                                         |
| Convert in the hub                        | Step 3 **Not in hub**; HA invariant                                                                       |
| `fromDoc` live Store                      | Step 4 **Not live Store**; Path B only                                                                    |
| GET misses unflushed RAM                  | Wait ≥2s; Flush uses the same wait. Replica is the recon escape, not a tab Store                          |
| Idle reset every keystroke                | Step 5 coalesce                                                                                           |
| Tabs freeze during `fromDoc`              | Step 7 delay convert; two-tabs must pass                                                                  |
| Nested wiki committed to product git      | `.gitignore` / **Nested repo**                                                                            |
| Second adapter dialect                    | Same `from-doc.js`; goldens                                                                               |
| `jobs` / Kafka “for HA”                   | Forbidden in M3; thin queue is in-process                                                                 |
| AB1 on the cut                            | Not this exit; must not delay commit                                                                      |


## Handoff to M4

M4 may assume:

- `wiki/` exists; snapshot autocomment works for `doc:home`.
- Path B convert + disk sidecar; pane is still Path A.
- Catalog is a **constant path**, not a CRDT. Tree UI and `git mv` are new.
- Lease / review / apply are **not** done.
- Hub still has no convert.

M4 exit is two pages, a link, a folder move, git tree matches. M3 exit is “clone `wiki/` and read markdown; typing during flush still syncs.”

## Invariants (M3 only)

1. One workspace, one page, page mode. Catalog v0 = one `gitPath`.
2. Live CRDT never waits on markdown, git, or convert.
3. Dirty is clocks, not keystrokes. One inflight flush per wiki.
4. Pin then convert. Not the live Store. Not the spectator splice.
5. Snapshotter beside the hub. Not in the hub. Not in the editor tab.
6. No markdown in Postgres. No per-block commits. No review why.
7. `mount-editor` stays unaware of git and mdgate convert.
8. Outline remains in-page headings (not a wiki tree).
