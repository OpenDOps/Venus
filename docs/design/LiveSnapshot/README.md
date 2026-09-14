# Live snapshot (pin + git snapshotter)

**Status:** design. **Implemented in [M3](../M3/README.md)** (closed 2026-09-14). Live CRDT is the **Venus hub** (`crates/venus-hub`, Compose `hub`) after [M3.0](../M3.0/README.md) (**closed** 2026-09-13). Snapshotter sits **beside** that hub (`crates/venus-sidecar`). [high-availability.md](./high-availability.md) **Acceptance** accepted 2026-09-13. Lease `T0` reuses the same pin ([M5](../venus-implementation-plan.md#m5--lease--freeze-week)). Live CRDT HA: [M3.0/high-availability.md](../M3.0/high-availability.md). Scale for git/jobs is this folder’s HA file — M3 is a **thin instance** and must not invert it ([M3 must keep this shape](./high-availability.md#m3-must-keep-this-shape)).

Product rules (two git classes, markdown is a projection): [venus-design.md](../venus-design.md). **Git tree:** [datamodel — git](../datamodel/git.md). Idle / flush-before-lease: [v1-concerns.md](../../drafts/pre-design/v1-concerns.md). Adapter: [MDGate](../MDGate/README.md). Exporter landed in [M2](../M2/README.md); this folder converts a **pin**, not the live Store.

A **pin** is a frozen copy of Yjs bytes (plus catalog) at time T. **Git snapshotting** converts that pin to markdown and commits. Pinning must not stall live CRDT apply or fan-out.

## This folder vs M3

| File | Owns |
|---|---|
| This README | Pin-then-convert invariant (one wiki). Hub is the collab front. |
| [high-availability.md](./high-availability.md) | Scale: `jobs`, `dirty_wiki`, `SKIP LOCKED` fleet. Gate for M3 code. |
| [M3/plan.md](../M3/plan.md) | Implementation: one page **load**, HA mechanism (`jobs` + MVCC), `venus-sidecar`, one nested `wiki/`. |

keck proved the wire in [M1](../M1/README.md). It is not the product collab front and not a pin source.

## Invariant

Live collaboration never waits on markdown, git, or pin conversion.

```text
browsers  ──Yjs update v1──►  hub memory (Rust y-octo apply + broadcast)
                                  │
                                  ├── broadcast to other sockets     (always)
                                  ├── persist batch → Postgres       (always, ~1s)
                                  │     AFTER persist UPSERT dirty
                                  └── pin copy → sidecar Map         (flush only)
                                              │
                                              ▼
                                       venus-sidecar: y-octo hydrate + fromDoc + sidecar
                                              │
                                              ▼
                                       wiki/*.md + git commit
```

Do **not** freeze the published page for a WYSIWYG snapshot. Freeze is lease-only ([lease-freeze-rationale.md](../lease-freeze-rationale.md)).

## Pin then convert

Flush is two phases. Collect every dirty pin **before** any `fromDoc`.

```text
1. Dirty set     docIds with a new Yjs clock
                 ∪ catalog nodes whose gitPath changed
                 ∪ new/changed blobs
2. Pin           copy Yjs update v1 (+ catalog) into Venus memory
3. Convert       MarkdownAdapter.fromDoc on the pin only
                 rebuild sidecar ranges; write wiki/<gitPath>
                 git mv if path changed
4. Commit        one git commit (autocomment, or required why on lease accept)
5. Release       store last-flushed clocks; drop pins
                 (keep the pin if this flush is T0 for a lease)
6. Index         enqueue LifeIndexing on this SHA + dirty docIds
                 (async; must not delay 5)
```

Step 6 is a **parallel track**, not M3 exit. Direct (link) parse may run after the `.md` is written and the cut is released. LLM gists / logical graph never sit on the cut, `fromDoc`, or `last_flushed`.

Dirty unit is the **page** (one `.md` file), not a block. Intra-doc hunks are the comment-commit path (M6), not the snapshotter. Catalog moves are `git mv` with no `fromDoc` if the body clock is unchanged.

What you pin is **CRDT export bytes**, not markdown files. Files are the output of convert. MDGate: [pin-convert.md](../MDGate/pin-convert.md) (`pinThenFromDoc`). Do not `fromDoc` the live published Store. Do not pass `incrementalFromDoc` output as T0.

## Where the pin lives

| When | Store | Why |
|---|---|---|
| Idle / Flush (M3) | Process memory `Map<docId, { bytes, clock }>` | Short; crash → next idle retries; git HEAD is last successful commit |
| Flush-before-lease / `T0` (M5) | Keep the same bytes until the lease ends (review space or a pin row) | Review Before must equal the git snapshot just committed |
| After git SHA (M8, optional) | `.venus/snapshots/<docId>/<sha>.bin` | Rare byte-identical CRDT restore. Not the working pin |

Do not invent a second document store. Postgres remains the live persist for Y.Docs (`crdt_*`). Pins are short-lived copies keyed by `flushId` / `docId` / clock.

## Live work during a pin

Goal the product needs: **copy does not stall CRDT**.

While Venus is pinning and converting:

- Clients keep sending updates.
- The **hub** **applies** them to the in-memory doc and **broadcasts** them.
- Persist to Postgres **keeps running** (hub batches ~1s; [M3.0 HA](../M3.0/high-availability.md)).
- New updates after the pin clock are **not** in this git commit. They wait for the next idle/flush (already the lag rule).

The Venus pin is that in-memory copy. It is **not** a pause of hub persist.

Do **not**:

- Stop apply or WebSocket fan-out for the duration of `fromDoc` / `git commit`.
- Pause Postgres writes for the whole convert (seconds). That only increases crash-loss and is not a hub API.
- `fromDoc` the live `Store` in a browser tab.
- Re-export the whole wiki; only dirty `docId`s.

A best-effort cut: sequential pins of dirty docs, then convert. Not a multi-space SQL transaction. Catalog pin in the same collect phase so `gitPath` matches the files you write. M3 has one page and a constant `gitPath` (`spec/home.md` unless api-map Actual differs). Catalog CRDT + `git mv`: [M4](../M4/README.md).

## Pin source

Against the **hub** ([M3.0 HA — pin](../M3.0/high-availability.md#pin-against-this-hub)):

| When | Source | Notes |
|---|---|---|
| **M3 idle / Flush** ([plan](../M3/plan.md) constraint 5) | After **claim**: MVCC `REPEATABLE READ` plain `SELECT` of dirty set S (`crdt_*` + blobs). | Persist lag is inherent (SQL as of T). Not GET export. Not a tab `Store`. Hidden AFFiNE client per page is **not** required. |
| **M5 `T0` / flush-before-lease** | Same MVCC cut, or replica encode if Before must include RAM not yet in SQL | So Before is not missing in-memory-not-yet-SQL updates. |
| **Scale** | Same MVCC `SELECT` after claim | [HA pin cut](./high-availability.md#pin-cut-lock-only-dirty-files-then-copy). M3 is this mechanism at one-wiki load. |

Convert from the pin Map (`fromPinnedBytes`). The hub never sees the pin and does not convert.

## Git snapshotter

Venus is the only writer of published commits in v1.

| Class | Trigger | Message | Input |
|---|---|---|---|
| Snapshot | Idle 30–120s (M3 default **60s**), Flush, flush-before-lease | Autocomment `snapshot: <title>` | Pin of dirty pages + catalog `git mv` |
| Comment-commit | Lease accept | Required why | Same convert path on the `T0` pin after BlockSuite ops |

Coalesce: all WYSIWYG since last git SHA is **one** snapshot commit, not one commit per keystroke.

Working tree:

```text
wiki/                         ← nested git (M3, created on first snapshot); product git ignores it
  spec/home.md                ← M3 catalog v0 (`doc:home`); later: catalog gitPath
  .venus/ids/<docId>.json     ← sidecar; clock = pin clock
  assets/                     ← dirty blobs only
```

LifeIndexing ([Agents](../Agents/LifeIndexing.md)) reads this tree at the commit SHA. It does not convert the pin. Logical gists/edges are a side index keyed by that SHA (Venus tables or a follow-up under `.venus/`); they do not change snapshot autocomment.

Clone of `wiki/` is ordinary folders + markdown. Folder **nesting** is directories. Sibling **order** stays on the catalog CRDT (M4). Empty catalog folders are not in git unless a placeholder is added later.

## Collab front (hub)

Product collab is Compose **`postgres` + `hub` (+ `web`)**. The hub **only** applies, broadcasts, persists `crdt_*` / blobs, and serves export. It does **not** offer “hold persist until pin copy finishes.” After persist, Postgres **upserts** `dirty` and `dirty_wiki` (SQL trigger). The hub does not convert and does not write `jobs`.

M3 snapshotter (`venus-sidecar`) **observes** `dirty_wiki`, `INSERT…SELECT`s `jobs`, workers **claim** (`SKIP LOCKED`), **MVCC**-pin, convert, commit. Convert is Path B: y-octo hydrate + Rust `fromDoc` (JS CLI oracle) + **git2**. Not in `crates/venus-hub`. Not from the editor tab.

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | This design (hub + pin then convert) |
| [M3.0/high-availability.md](../M3.0/high-availability.md) | Live CRDT HA (sticky owner, persist, dirty, drain) |
| [high-availability.md](./high-availability.md) | Queue, dirty list, pin cut, worker fleet. **Accepted 2026-09-13.** |
| [M3 plan](../M3/plan.md) | Implementation steps (`jobs` + MVCC; recon + convert done). |
| [MDGate pin-convert](../MDGate/pin-convert.md) | Host convert helper (no git write) |
| [LifeIndexing](../Agents/LifeIndexing.md) | After commit: gists, tags, direct + logical graphs. Not convert. Plan: [agentic-binding](../Agents/agentic-binding.md). |

Words: [glossary.md](../glossary.md). Dataflow: [architecture.md](../architecture.md). Hub process: [components/backend/hub](../components/backend/hub/).
