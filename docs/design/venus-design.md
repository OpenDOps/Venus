# Venus: a human-sharable LLM wiki

Venus is a **spec-driven development** tool. The wiki is the spec store (two honest representations of each published page):

1. **Live collaborative document** — a BlockSuite block tree, synced as a Yjs/y-octo CRDT through OctoBase.
2. **Git markdown tree** — ordinary folders and `.md` files that humans and LLMs can clone, read, and review without the editor.

Those two representations are **not two replicas**. The CRDT is the live published page. Markdown on disk is a **projection** of that page ([glossary](./glossary.md) — markdown projection vs git snapshot commit). Dataflow: [architecture.md](./architecture.md).

- **Casual WYSIWYG** (no lease): humans type in BlockSuite. Git gets a **snapshot commit** of the whole diff since the last commit, with an **autocomment** (stock message). Not a review comment-commit.
- **Markdown** (lease): a **comment-commit** is required — hunks plus a human/agent rationale, pinned like Google Docs / Jira. Apply updates the CRDT and git together.

See [v1-concerns.md](../drafts/pre-design/v1-concerns.md) and [CRDT views](#crdt-views-after-and-before).

This document is the product and data design. **Where bytes live:** [datamodel](./datamodel/README.md). Runtime dataflow: [architecture.md](./architecture.md). CRDT stack: [CRDT](./CRDT/README.md). Words: [glossary.md](./glossary.md).

Related:

- **v1 scope** (WYSIWYG vs git, one-commit review wedge): [v1-concerns.md](../drafts/pre-design/v1-concerns.md)
- **Why freeze on lease:** [lease-freeze-rationale.md](./lease-freeze-rationale.md)
- **Spec-driven plans** (yaml board; trackers are mirrors): [venus-plan.md](../drafts/pre-design/venus-plan.md)
- Binding and rollout: [venus-implementation-plan.md](./venus-implementation-plan.md)
- **Stores (CRDT spaces + git tree):** [datamodel](./datamodel/README.md)
- Installed symbols: [api-map.md](./api-map.md)
- Markdown adapter gate: [MDGate](./MDGate/README.md) (design). Code: [M2](./M2/README.md).
- Markdown cannot be a second CRDT: [cursor_crdt_limitations_with_markdown.md](../drafts/pre-design/cursor_crdt_limitations_with_markdown.md)
- Licensing (BlockSuite MPL vs OctoBase AGPL, prototype vs cloud): [licensing.md](../legal/licensing.md)
- Pitch (humans keep intention, agents do the work): [pitch.md](../marketing/pitch.md)

## Goal

**Humans keep heart, instinct, faith and intention; agents do the work.** Enforced as: one spec that cannot silently drift — **markdown and agent writes** publish only with a lease and a human, and a story is not done until the spec diff is accepted. Casual WYSIWYG is different: snapshot git + autocomment, no review comment. Full pitch: [pitch.md](../marketing/pitch.md).

A spec-driven workspace that is:

- **Collaborative** for humans in a Notion-style WYSIWYG editor (BlockSuite).
- **Sharable as files** for humans and agents (git folders + markdown).
- **Reviewable like Cursor** when someone (human or agent) edits source: one lease, frozen published page, commented diffs, accept / rollback / reply. Each review commit has **Before and After CRDTs in OctoBase**. Edits to After before apply are a **new** comment-commit in the sequence, not a silent rewrite. Alternative **picker** UI is post-v1 ([v1-concerns.md](../drafts/pre-design/v1-concerns.md)).
- **Runnable as plans:** spec → plan (planner agent) → DoD (**other** agent) → both human-accepted on the same lease/review path as spec → implement / autotests / PR review → human-accepted doc updates. The board is yaml + the plan page; GitHub/Plane are optional mirrors. See [venus-plan.md](../drafts/pre-design/venus-plan.md).
- **Autodocumenting:** aims of the software and generated API markdown live in the same spec tree; Hugo publishes only accepted git. See [venus-plan.md](../drafts/pre-design/venus-plan.md).

It is not a second AFFiNE. It is a thinner product on the same editor and CRDT engine, with git as the long-term, human-readable history.

## Non-goals (v1)

- Merging two markdown buffers, or treating `.md` as a live CRDT.
- Concurrent WYSIWYG on the **published** page while a markdown lease is held (After is a sibling OctoBase CRDT; [datamodel — commit Before/After](./datamodel/crdt.md#commit-before-and-after)).
- Storing pending hunks, rationales, or review threads on published blocks.
- Forking AFFiNE Cloud (NestJS, GraphQL, payments, copilot). OctoBase + y-octo are the backend.
- Full Notion databases, edgeless/whiteboard as a first-class surface.
- **UI** for picking among alternatives / deep stacks (v2). The **data** already stores Before/After CRDTs and `parentCommitId` ([v1-concerns.md](../drafts/pre-design/v1-concerns.md) is the wedge for one visible commit).

## Stores

**Source of truth for layout:** [datamodel](./datamodel/README.md) — [CRDT spaces](./datamodel/crdt.md), [git tree](./datamodel/git.md).

```text
Published page + catalog + review + Before/After     OctoBase → Postgres
        │  pin + fromDoc (not a second replica)
        ▼
wiki/*.md + .venus/ids + assets                      git
```

Published blocks never hold hunks or rationales. Review After/Before are real spaces. Git is share/history only.

## Commit Before / After

Decision and space ids: [datamodel — commit Before/After](./datamodel/crdt.md#commit-before-and-after). After-edits after submit are the next comment-commit (`parentCommitId`), not a rewrite. Persist as soon as hunks exist; not tab RAM.

## Roles of each AFFiNE tool

Venus is **based on** BlockSuite, OctoBase, and y-octo. It is **another tool** on top of them.

| Tool | Role in Venus |
|---|---|
| **BlockSuite** (`@blocksuite/affine`) | Primary editor: page editor, block schema, selection, outline, linked-doc embeds, markdown adapter. |
| **Yjs (browser)** | Client CRDT that BlockSuite already uses. Every doc is a Y.Doc. |
| **y-octo (Rust)** | Server-side Yjs-compatible engine: apply/merge updates, compact snapshots, binary ↔ structured doc, markdown conversion helpers. |
| **OctoBase** | Prototype WS + blob HTTP (keck). **Own Docker.** Persist is **Postgres in a second Docker**, not SQLite. |
| **Venus** | Lease, review commits, git snapshotter, folder catalog, freeze policy. None of this is BlockSuite’s job. |

OctoBase is pre-1.0 and AGPL. **Prototype only:** it is a convenient CRDT workspace so we do not write a sync server from scratch. **Cloud Venus replaces it** (Yjs + y-websocket/Hocuspocus, optional y-octo). We do not take AFFiNE’s full product shell. Details: [licensing.md](../legal/licensing.md).

## Published document

A page is one BlockSuite doc (one OctoBase space), with the usual `affine:*` tree:

```text
affine:page
  affine:surface          # present, unused in v1 page mode
  affine:note
    affine:paragraph | affine:heading | affine:list | affine:code | …
    affine:embed-linked-doc | affine:embed-synced-doc
```

Requirements on top of BlockSuite’s defaults:

1. **Stable `blockId`s** that survive markdown export → edit → parse. Export writes them into a Venus sidecar map (not as user-visible markdown syntax). Parse diffs by id, not by “paragraph 3.”
2. **Snapshot clock (`T0`)** — at lease acquire, Venus calls keck **doc export** (or a Yjs state vector) and **pins** that result. The live GET is not `T0`; keeping it is. [glossary](./glossary.md). [architecture.md](./architecture.md).
3. **Stable `docId`** — the OctoBase space id. The git path can change when the user moves the page; `docId` does not.

Markdown export is lossy for anything the adapter cannot name (colors, some embeds). Editable markdown is only the **round-trippable subset**. Unknown blocks stay opaque in export (HTML/html-comment or a raw block) and are not rewritten by a markdown commit unless the writer touches them.

## Folder tree (table of contents)

There are two different “TOCs.” Do not collapse them.

### In-page outline

BlockSuite’s **outline widget** (heading list for the open page). Use it as-is.

### Wiki folder tree

The wiki TOC is the **folder tree of pages**. BlockSuite’s `Workspace` / `DocCollection` is a flat bag of docs plus `meta.docMetas` (title, etc.). AFFiNE’s explorer is an application feature, not a BlockSuite primitive. Venus does **not** adopt AFFiNE’s full sidebar.

Canonical layout is the **git filesystem**:

```text
wiki/
  spec/
    protocol.md
    crdt/
      lease.md
  design/
    overview.md
  assets/
    …
```

Live collaboration on that tree is a small CRDT **catalog** in OctoBase ([datamodel — catalog](./datamodel/crdt.md#catalog)):

```text
Catalog
  nodes: Map<nodeId, Node>

Node
  id
  kind            // folder | doc
  name            // "lease.md" or "crdt"
  parentId        // null = wiki root
  order           // fractional index among siblings
  docId?          // OctoBase space id when kind=doc
  gitPath         // derived, cached: spec/crdt/lease.md
```

Folder moves are CRDT ops on the catalog (reparent + order). They do **not** rewrite page bodies. On the next git commit that includes a move, Venus does `git mv` so the filesystem matches the catalog.

Identity rule: **path is not identity**. Links and leases key by `docId`. Renames do not break `affine:embed-linked-doc`.

Why not “just git” as the live tree: two people moving folders offline need a merge. Git merge of directory moves is worse than a tiny CRDT catalog. Why not “just the CRDT”: LLMs and reviewers must see real folders. Git remains the share format; the catalog is the live index.

## Cross-document references

Use BlockSuite’s existing blocks, not a Venus hyperlink scheme:

- **`affine:embed-linked-doc`** — card / mention of another page (`pageId` = `docId`).
- **`affine:embed-synced-doc`** — transclude another page’s content.

Markdown export of a link uses a stable form:

```markdown
[Lease freeze](./crdt/lease.md)          <!-- human path -->
<!-- venus:doc:8f3a… -->
```

Import resolves `venus:doc:…` first, path second. Path-only links from an LLM still work if the catalog can resolve them at apply time.

The catalog + BlockSuite `docLinkBaseURLMiddleware` / `titleMiddleware` (already used by AFFiNE’s markdown adapter) are the binding for titles and URLs.

## Lease

Any **user or agent** may request a markdown lease on a page (later: on a folder, as a batch). One writer. Published WYSIWYG on that page **freezes** until release. Reviewers can still read, switch old / new / diff, comment, accept, reject, and attach alternative commits.

```text
idle ──acquire──► leased (writer editing markdown / others reviewing)
                    │
                    ├─ commit accepted proposals → apply CRDT → git commit → idle
                    ├─ reject session → drop proposals → idle
                    └─ timeout / steal (confirm) → drop or keep-pending policy
```

Lease record (on the review session, not on the block tree):

```text
Lease
  docId
  holder          // user or agent id
  snapshotClock   // T0
  snapshotTree    // optional frozen snapshot blob
  acquiredAt
  heartbeatAt
```

While leased:

- **Published** BlockSuite is read-only (freeze).
- Reviewers use [CRDT views](#crdt-views-after-and-before): **After** and **Before** are OctoBase docs ([datamodel](./datamodel/crdt.md#commit-before-and-after)).
- Read-only markdown may refresh from Before or After — it is not a replica.
- First hunks: holder’s private markdown vs `T0`. After submit, After is editable; those edits are the next comment-commit.
- Everyone else writes **comments** on the Before view (Google Docs / Jira rail) and workflow (accept / reject).

Acquire pins `T0` via keck **doc export** / y-octo (or a state vector) and serializes markdown **with the block-id map**. The HTTP GET is the current tree; the **kept** bytes (or clock) are `T0`.

## Comment-commits (markdown only)

Casual WYSIWYG does **not** create comment-commits. It creates [snapshot commits](#snapshots-revert-history) with autocomment.

A **markdown** change to published content is a **comment-commit**: a grouped set of diffs plus a **required** comment that says why. That is the only path that must have a review comment.

### Two entry points

**A. Markdown edit, then comment**

1. Acquire lease (flush-before-lease so `T0` = git HEAD).
2. Edit markdown (agent or human).
3. Venus diffs `T0` vs proposed tree/markdown **by block id**.
4. Writer submits: hunks are **one comment-commit**; a **comment is required**.
5. The comment is **pinned to a text selection** on the Before (or After) view, Google Docs / Jira style.

**B. Pin a comment first, diffs later**

1. Select text on the **Before** view (published, or frozen `T0`).
2. Pin a comment in the **right rail**. This creates a **thread**. In v1 it does not have to create a commit with no hunks.
3. Later, under a lease, the writer attaches hunks to that thread (one comment-commit).
4. Other users reply on the rail. Alternative/stacked commits are post-v1.

Entry B without hunks does **not** freeze the page. Freeze starts when someone **writes markdown / attach hunks** (lease). A comment-only pin uses relative positions on the live **Before** text until a lease freezes `T0`.

### Data

```text
Thread
  id
  docId
  anchor          // pin
  comments        // sequence; anyone may reply
  commits         // one or more proposals attached to this thread

Anchor
  // comment-only, no lease:
  { kind: live, blockId, start, end }          // Yjs relative positions
  // during / after a leased review:
  { kind: snapshot, clock, side: old|new, hunkId?, start, end }

Commit                         // a review proposal, not yet published git
  id
  threadId
  parentCommitId?              // sequence: this commit is **over** the parent, not a rewrite
  baseClock                    // Before clock (T0, or parent After)
  beforeDocId                  // OctoBase space — Before CRDT (readonly)
  afterDocId                   // OctoBase space — After CRDT (editable until accept)
  author                       // user or agent
  message                      // required rationale (the “why”)
  hunks[]                      // immutable at submit; later After edits → new Commit
  status                       // draft | proposed | accepted | rejected | superseded

Hunk
  id
  blockId?                     // null if insert
  afterBlockId?                // insert/move anchor in base order
  kind                         // modify | insert | delete | move
  old                          // markdown + tree slice (immutable)
  new                          // markdown + tree slice (immutable)
  diff                         // added / removed
```

`old` / `new` are **values**, not text CRDTs. Do not store proposal markdown as a second Y.Text. The **After** tree is a BlockSuite Y.Doc in OctoBase. The lease holder replacing `new` on **draft regenerate** (same commit id, bump `generation`) is only for an unsubmitted draft. After submit, After-edits are a **new** commit ([datamodel](./datamodel/crdt.md#commit-before-and-after)).

### Alternatives and stacks

Post-v1 **picker UI**. The fields `parentCommitId`, `beforeDocId`, `afterDocId` are in the model from the start. **Do not** ship a multi-PR chooser in v1; one visible tip is enough. See [v1-concerns.md](../drafts/pre-design/v1-concerns.md).

On one thread (later):

```text
T0
 ├── Commit A     alternative 1
 ├── Commit B     alternative 2 (same base T0)
 └── Commit A'
       └── Commit C   based on A   (stack)
```

- **Alternatives** share a base. Accepting one supersedes the others (or they remain as rejected history).
- **Stacked** commits apply in parent order. Accepting `C` implies accepting `A` first, or accepting the composed diff `T0 → C`.
- Replies are comments. A reply may introduce a new commit (`parentCommitId` set) instead of mutating the parent.

This is the Cursor loop, generalized to several humans and several proposals: regenerate replaces a commit’s hunks and increments a `generation`; anchors store `generation` so pins do not silently slide onto new text.

### Apply and git

Two git classes. Do not mix them.

**Snapshot (WYSIWYG, no lease)** — whole CRDT-vs-HEAD diff, **one** commit, **autocomment** (`snapshot: <title>` or similar). No review UI, no required why.

**Comment-commit (markdown lease)** — published CRDT and git move together on **accept**:

```text
accepted comment-commit + T0
  → BlockSuite ops on the published doc (by block id)
  → serialize markdown + write wiki/<gitPath>
  → git add / git mv as needed
  → git commit -m "<required comment>"
       author = holder
       body   = hunk summary + thread link
  → archive the review commit (immutable)
  → release lease if no remaining proposed commits require it
```

Rollback of a review commit is not `git revert` and not CRDT undo. It drops or rejects the proposal. `git revert` is how you undo an **already published** git commit (re-import that snapshot into the CRDT).

## Snapshots, revert, history

Every published markdown tree change Venus writes is a git commit: either a **snapshot** or a **comment-commit**.

| Class | When | Message | What |
|---|---|---|---|
| **Snapshot** | Idle / flush / flush-before-lease after WYSIWYG | **Autocomment** (stock: `snapshot: <title>`) | Full page (and catalog `git mv` if needed) vs last git |
| **Comment-commit** | Markdown lease accept | **Required** review comment (the why), optional pin | Hunks vs `T0` |

- **Who** — snapshot: last WYSIWYG editor (or `venus-snapshot`). Comment-commit: lease holder.
- **Why** — only comment-commits. Filter `snapshot:` in `git log` when you want rationale.
- **What** — `git show`.
- **BlockSuite snapshot** at that commit: `MarkdownAdapter.toDoc` plus block-id sidecar, or optional `.venus/snapshots/<docId>/<gitSha>.bin`.

Coalesce WYSIWYG: all typing since the last git commit is **one** snapshot commit, not one commit per keystroke. Details: [v1-concerns.md](../drafts/pre-design/v1-concerns.md).

Revert:

1. Choose a git commit (page or whole tree).
2. Acquire lease (the revert **is** a markdown/source write).
3. Show a comment-commit: old = current `T0`, new = historical snapshot.
4. Accept → apply to CRDT → git comment-commit “revert to \<sha\>”.

Do not replay raw Yjs history as the user-facing version log. Yjs history is for live merge. Git is for “what the wiki said on Tuesday.”

## CRDT views (After and Before)

The same page can be shown as BlockSuite in two **OctoBase** docs. After is not a second published replica and not a tab-local scratch Store.

**1. After** — how the document **will look** if this commit (plus ancestors) is accepted.

- **No lease:** After **is** the live published CRDT. Humans edit it (Notion-like). The entire diff since the last git commit is **one snapshot commit** (autocomment) when idle/flush runs.
- **Markdown lease:** After is the commit’s **After space** (clone of Before + hunks). It is a synced CRDT. Humans may edit it; those keystrokes are **not** published snapshots. Submitted After-edits are the **next** comment-commit in the sequence. v1 holder still **creates** the first hunks from markdown ([MDGate apply](./MDGate/apply.md)).

**2. Before** — the document **before** this comment-commit (lease `T0`, or parent After).

- Readonly CRDT of the parent (its own OctoBase space, or a readonly bind of that clone).
- **Right rail:** comments pinned to text, like [Google Docs](https://docs.google.com) or Jira — threads on a selection, not hunk cards stuffed into the block tree.
- Markdown comment-commits **must** use this rail for the required why (and replies). Snapshot autocomments do not need a rail.

Toggle After / Before (and Diff) while a lease is held. Diff overlays hunks on these CRDTs (Cursor-style). Without a lease, After is the editor; Before is “last snapshot” for compare if we show it.

## Views

| Mode | When | Who types |
|---|---|---|
| **After (WYSIWYG)** | No lease | Anyone; live published CRDT; snapshot+autocomment later |
| **After (proposal)** | Lease held | Anyone on the **After space** (not published). First hunks: holder markdown. Later After edits: next comment-commit |
| **Before** | Compare or lease | Nobody; parent CRDT + **comment rail** |
| **Read-only markdown** | Always | Nobody; `MarkdownAdapter.fromDoc` |
| **Leased markdown** | Holder only | Holder’s private buffer; parse → hunks, not a CRDT |
| **Diff** | Lease held | Nobody; hunk overlay + rail |

The wait state during a markdown lease is this review UI (After / Before / Diff + rail), not a spinner.

## Invariants

1. Markdown is never a second published replica.
2. The published block tree contains only accepted content.
3. At most one markdown lease per page.
4. While a lease is held, published WYSIWYG is frozen (see [lease-freeze-rationale.md](./lease-freeze-rationale.md)).
5. Every **markdown** (review) mutation has a comment-commit (git message + pin when the writer selected one). **Published** WYSIWYG mutations become snapshot commits with **autocomment** only. WYSIWYG on a commit **After** space is still review: a **new** comment-commit in the sequence, not a snapshot and not a rewrite of the parent commit.
6. Folder identity is `docId`; git path is a projection.
7. Links prefer `docId`, then path.
8. Git is the durable, human-readable history of published snapshots.
9. **Flow:** spec / plan / DoD / docs publish only on **human** accept of meaning (comment-commit). Agents draft; they do not apply. A story that moved spec is not done on PR merge. Skip the lease when spec did not move. Managers complete that path in Venus without Cursor. Details: [product-plan — Force these](../product/product-plan.md#force-these-or-it-is-not-the-flow).

## Trust boundary for agents

An agent is a lease holder. It does not write the published CRDT incrementally. It writes a private markdown buffer, emits hunks with rationales, and waits for **human** accept — the same path for spec, plan, DoD, aims, and generated API docs. Humans keep intention; agents do the draft. Humans can comment, request regenerate, or reject without unlocking WYSIWYG.

On plans, a second agent **should** author DoD; the implementer should not. v1 **warns**; hard refuse is later — [product-plan](../product/product-plan.md#product-goal). See [venus-plan.md](../drafts/pre-design/venus-plan.md).
