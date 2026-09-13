# Live snapshot (pin + git snapshotter)

**Status:** design (one-wiki pin). Implement in [M3](../M3/README.md) ([plan](../M3/plan.md)) **only after** [M3.0](../M3.0/README.md) **is closed** (Venus hub replaces keck) **and** [high-availability.md](./high-availability.md) **Acceptance** (snapshotter beside the hub). 2026-08-31 “OctoBase stays” is superseded. Lease `T0` reuses the same pin ([M5](../venus-implementation-plan.md#m5--lease--freeze-week)). Live CRDT HA: [M3.0/high-availability.md](../M3.0/high-availability.md). M1 keck recon: [octobase.md](./octobase.md). Scale for git/jobs is this HA file — M3 is a thin instance and must not invert it.

Product rules (two git classes, markdown is a projection): [venus-design.md](../venus-design.md). **Git tree:** [datamodel — git](../datamodel/git.md). Idle / flush-before-lease: [v1-concerns.md](../../drafts/pre-design/v1-concerns.md). Adapter: [MDGate](../MDGate/README.md). Exporter lands in [M2](../M2/README.md); this folder converts a **pin**, not the live Store.

A **pin** is a frozen copy of Yjs bytes (plus catalog) at time T. **Git snapshotting** converts that pin to markdown and commits. Pinning must not stall live CRDT apply or fan-out.

## Invariant

Live collaboration never waits on markdown, git, or pin conversion.

```text
browsers  ──Yjs update v1──►  hub memory (Rust y-octo apply + broadcast)
                                  │
                                  ├── broadcast to other sockets     (always)
                                  ├── persist batch → Postgres       (always, ~1s)
                                  └── pin copy → Venus buffer        (flush only)
                                              │
                                              ▼
                                       Rust worker: y-octo hydrate + fromDoc + sidecar
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

Do not invent a second document store. Postgres remains the live persist for Y.Docs. Pins are short-lived copies keyed by `flushId` / `docId` / clock.

## Live work during a pin

Goal the product needs: **copy does not stuck CRDT**.

While Venus is pinning and converting:

- Clients keep sending updates.
- The **hub** **applies** them to the in-memory doc and **broadcasts** them.
- Persist to Postgres **keeps running** (hub batches ~1s; [M3.0 HA](../M3.0/high-availability.md). M1 keck did the same: [octobase.md](./octobase.md)).
- New updates after the pin clock are **not** in this git commit. They wait for the next idle/flush (already the lag rule).

The Venus pin is that in-memory copy. It is **not** a pause of hub persist.

Do **not**:

- Stop apply or WebSocket fan-out for the duration of `fromDoc` / `git commit`.
- Pause Postgres writes for the whole convert (seconds). That only increases crash-loss and is not a hub API.
- `fromDoc` the live `Store` in a browser tab.
- Re-export the whole wiki; only dirty `docId`s.

A best-effort cut: sequential pins of dirty docs, then convert. Not a multi-space SQL transaction. Catalog pin in the same collect phase so `gitPath` matches the files you write.

## Pin source (prototype)

Preferred: the snapshotter holds its **own Y.Doc replica** (same `SyncProvider` / AFFiNE socket as a hidden client, or a Node `Y.Doc` on that room). Pin = `Y.encodeStateAsUpdate` into a **new** `Y.Doc` (or keep the bytes). Convert from that clone (`fromPinnedBytes`). The hub never sees the pin.

Acceptable for M3 idle (30–120s): `GET /api/block/:workspace/export` after persist has had a chance to flush. That GET reads **Postgres**, not the hub’s live RAM ([M3.0 HA — pin](../M3.0/high-availability.md#pin-against-this-hub)). Idle flush is already longer than the ~1s persist batch.

Flush-before-lease: pin from the replica (or wait ≥2s then GET) so `T0` is not missing in-memory-not-yet-SQL updates.

## Git snapshotter

Venus is the only writer of published commits in v1.

| Class | Trigger | Message | Input |
|---|---|---|---|
| Snapshot | Idle 30–120s, Flush, flush-before-lease | Autocomment `snapshot: <title>` | Pin of dirty pages + catalog `git mv` |
| Comment-commit | Lease accept | Required why | Same convert path on the `T0` pin after BlockSuite ops |

Coalesce: all WYSIWYG since last git SHA is **one** snapshot commit, not one commit per keystroke.

Working tree:

```text
wiki/
  <gitPath>.md              ← projection of a pinned page
  .venus/ids/<docId>.json   ← sidecar; clock = pin clock
  assets/                   ← dirty blobs only
```

LifeIndexing ([Agents](../Agents/LifeIndexing.md)) reads this tree at the commit SHA. It does not convert the pin. Logical gists/edges are a side index keyed by that SHA (Venus tables or a follow-up under `.venus/`); they do not change snapshot autocomment.

Clone of `wiki/` is ordinary folders + markdown. Folder **nesting** is directories. Sibling **order** stays on the catalog CRDT. Empty catalog folders are not in git unless a placeholder is added later.

## Collab front (hub)

The hub **does not** offer “hold persist until pin copy finishes.” It already separates live apply/broadcast from a ~1s persist buffer ([M3.0 HA](../M3.0/high-availability.md)). Venus puts the pin **beside** the hub. After persist, Postgres **upserts** `dirty`; the hub does not convert and does not write `jobs`. M1 keck recon (legacy): [octobase.md](./octobase.md).

## Files

| File | Role |
|---|---|
| [README.md](./README.md) | This design |
| [octobase.md](./octobase.md) | **M1 keck recon** (export, persist, Format overlay). Not the product hub. |
| [M3.0/high-availability.md](../M3.0/high-availability.md) | Live CRDT HA (sticky owner, persist, dirty, drain) |
| [high-availability.md](./high-availability.md) | Queue, dirty list, pin cut, worker fleet. **Re-accept;** M3 code is gated on it **and** M3.0 closed. |
| [M3 plan](../M3/plan.md) | Thin-column implementation steps (not started). |
| [MDGate pin-convert](../MDGate/pin-convert.md) | Host convert helper (no git write) |
| [LifeIndexing](../Agents/LifeIndexing.md) | After commit: gists, tags, direct + logical graphs. Not convert. Plan: [agentic-binding](../Agents/agentic-binding.md). |

Words: [glossary.md](../glossary.md). Dataflow: [architecture.md](../architecture.md).
