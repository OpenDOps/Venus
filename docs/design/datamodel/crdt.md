# CRDT storage

**Status:** design. Map: [README.md](./README.md). Git side: [git.md](./git.md). Prototype wire: [CRDT](../CRDT/README.md). Apply / hunks: [MDGate apply](../MDGate/apply.md). Product: [venus-design.md](../venus-design.md).

All live trees are BlockSuite `Store` / `Y.Doc` (`store.spaceDoc`). Venus does **not** add a markdown Y.Text. Persist is **Postgres** via keck (`DATABASE_URL`). SQLite is not the product store.

## Workspace vs spaces

Prototype binding ([implementation plan](../venus-implementation-plan.md#1-collection--octobase-workspace)):

```text
Venus workspace     TestWorkspace.id / keck room prefix
Published page      one OctoBase space = docId = catalog.node.docId
```

M1 collapsed this: keck room `venus-m0` **is** the one page `doc:home`. Later, one workspace contains many spaces. Create each space **once**, then sync.

Cloud: same space ids behind Hocuspocus / y-websocket + own Postgres. Do not encode OctoBase-specific block APIs into Venus records.

## Space kinds

| Kind | Space id (prototype) | Concurrent writes | Notes |
|---|---|---|---|
| **Published page** | `docId` (random OctoBase space id) | Yes, when **no** lease | Only accepted `affine:*` blocks. Freeze = readonly during lease. |
| **Catalog** | `venus:catalog` | Yes (folder moves) | Live wiki tree. Not a docs-framework TOC. |
| **Review session** | `venus:review:<docId>` | Comments yes; hunk **values** last-writer | Lease, threads, commit records. **Not** the proposal tree. |
| **Commit Before** | `venus:review:<docId>:c:<commitId>:before` | No | Readonly clone of parent clock. |
| **Commit After** | `venus:review:<docId>:c:<commitId>:after` | **Yes** until accept/reject | Proposal tree. Same **block ids** as published/`T0`. |

Blobs (images): keck `POST`/`GET /api/blobs/<workspace>` → same Postgres. Not a fourth kind of Y.Doc.

Do **not** put `proposedText`, `hunkStatus`, or `rationale` on the published page.

## Published page

One `affine:page` tree per `docId` ([venus-design — published document](../venus-design.md#published-document)). Identity:

- `docId` — space id; stable across rename/move.
- `blockId` — stable on that Y.Doc; git sidecar names spans, it does not own ids ([MDGate](../MDGate/README.md)).

Refresh: reconnect, wait `synced`, Postgres via keck is source of truth. IndexedDB is optional cache.

## Catalog

Live index of folders and pages. Git folders are the **share** layout; this CRDT is the **collaborative** tree.

```text
Catalog
  nodes: Map<nodeId, Node>

Node
  id
  kind            // folder | doc
  name            // "lease.md" or "crdt"
  parentId        // null = wiki root
  order           // fractional index among siblings
  docId?          // space id when kind=doc
  gitPath         // derived, cached: spec/crdt/lease.md
```

Moves = reparent + order. They do not rewrite page bodies. Next git commit that includes the move does `git mv` ([git.md](./git.md)). Empty folders may exist only in the catalog until a placeholder exists in git.

## Review session

Sibling of the published page. Holds:

```text
Lease
  docId
  holder
  snapshotClock     // T0
  snapshotTree?     // optional frozen Yjs blob (same clock as Before of first commit)
  acquiredAt
  heartbeatAt

Thread / Anchor / Commit / Hunk     // values — see venus-design comment-commits
```

Hunk `old` / `new` are **values**, not a second Y.Text of markdown.

`Commit` points at CRDT docs:

```text
Commit
  id
  parentCommitId?
  beforeDocId       // Before space
  afterDocId        // After space
  hunks[]           // immutable at submit
  message           // required why
  status            // draft | proposed | accepted | rejected | superseded
```

## Commit Before and After

Each review commit has two **persisted** BlockSuite documents in OctoBase (Postgres via keck). Not a scratch `Store` in one tab.

```text
Published (frozen during lease)    ids b1, b2, …     no proposal fields

Commit A
  before  = clone of T0             readonly space
  hunks   = markdown_T0 vs buffer   immutable at submit
  after   = clone of T0 + hunk ops  editable space; same block ids

User types on After_A (after submit)
  → Commit B, parentCommitId = A
       before = After_A at B’s submit clock
       after  = new space
```

**Sequence, not in-place.** Do not rewrite A’s hunks. Draft regenerate (`generation++`, same `commitId`) is only **before first submit**.

Materialize After as soon as hunks exist: clone T0 bytes → new space → apply hunk ops → keck persist. Crash must not lose a **submitted** After.

On **accept:** apply the sequence (or id-diff published `T0` vs tip After) onto **published**; `fromDoc` → git comment-commit; set Before/After **readonly** (archive). Rollback of a proposal does not move published.

This is a **block-tree** CRDT. Markdown stays a projection ([MDGate apply](../MDGate/apply.md)).

v1 **UI** may show one tip ([v1-concerns.md](../../drafts/pre-design/v1-concerns.md)). Persistence of Before/After is not optional.

## Pins vs spaces

A **pin** ([LiveSnapshot](../LiveSnapshot/README.md)) is a short-lived copy of Yjs bytes for flush or lease `T0`. It is **not** a space. First-commit Before is that pin loaded into the Before space (kept until the lease ends). Idle snapshot pins are dropped after git convert.

`GET /api/block/…/export` is the **current** tree in Postgres, not `T0`, not git.

## What Postgres holds

| Bytes | Via |
|---|---|
| Yjs update v1 for every space above | keck persist (~1s batch) |
| Blob octets | keck blob HTTP |

No `.md`, no sidecar JSON, no git SHA as the live document. Those are [git.md](./git.md).
