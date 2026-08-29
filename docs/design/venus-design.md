# Venus: a human-sharable LLM wiki

Venus is a **spec-driven development** tool. The wiki is the spec store (two honest representations of each published page):

1. **Live collaborative document** — a BlockSuite block tree, synced as a Yjs/y-octo CRDT through OctoBase.
2. **Git markdown tree** — ordinary folders and `.md` files that humans and LLMs can clone, read, and review without the editor.

Those two representations are **not two replicas**. The CRDT is the live published page. Markdown on disk is a **snapshot** of that page.

- **Casual WYSIWYG** (no lease): humans type in BlockSuite. Git gets a **snapshot commit** of the whole diff since the last commit, with an **autocomment** (stock message). Not a review comment-commit.
- **Markdown** (lease): a **comment-commit** is required — hunks plus a human/agent rationale, pinned like Google Docs / Jira. Apply updates the CRDT and git together.

See [v1-concerns.md](../drafts/pre-design/v1-concerns.md) and [CRDT views](#crdt-views-after-and-before).

This document is the product and data design. Related:

- **v1 scope** (WYSIWYG vs git, one-commit review wedge): [v1-concerns.md](../drafts/pre-design/v1-concerns.md)
- **Why freeze on lease:** [lease-freeze-rationale.md](./lease-freeze-rationale.md)
- **Spec-driven plans** (yaml board; trackers are mirrors): [venus-plan.md](../drafts/pre-design/venus-plan.md)
- Binding and rollout: [venus-implementation-plan.md](./venus-implementation-plan.md)
- Markdown cannot be a second CRDT: [cursor_crdt_limitations_with_markdown.md](../drafts/pre-design/cursor_crdt_limitations_with_markdown.md)
- Licensing (BlockSuite MPL vs OctoBase AGPL, prototype vs cloud): [licensing.md](../legal/licensing.md)
- Pitch (humans keep intention, agents do the work): [pitch.md](../marketing/pitch.md)

## Goal

**Humans keep heart, instinct, faith and intention; agents do the work.** Enforced as: one spec that cannot silently drift — **markdown and agent writes** publish only with a lease and a human, and a story is not done until the spec diff is accepted. Casual WYSIWYG is different: snapshot git + autocomment, no review comment. Full pitch: [pitch.md](../marketing/pitch.md).

A spec-driven workspace that is:

- **Collaborative** for humans in a Notion-style WYSIWYG editor (BlockSuite).
- **Sharable as files** for humans and agents (git folders + markdown).
- **Reviewable like Cursor** when someone (human or agent) edits source: one lease, frozen published page, commented diffs, accept / rollback / reply. Alternative and stacked proposals are post-v1 ([v1-concerns.md](../drafts/pre-design/v1-concerns.md)).
- **Runnable as plans:** spec → plan (planner agent) → DoD (**other** agent) → both human-accepted on the same lease/review path as spec → implement / autotests / PR review → human-accepted doc updates. The board is yaml + the plan page; GitHub/Plane are optional mirrors. See [venus-plan.md](../drafts/pre-design/venus-plan.md).
- **Autodocumenting:** aims of the software and generated API markdown live in the same spec tree; Hugo publishes only accepted git. See [venus-plan.md](../drafts/pre-design/venus-plan.md).

It is not a second AFFiNE. It is a thinner product on the same editor and CRDT engine, with git as the long-term, human-readable history.

## Non-goals (v1)

- Merging two markdown buffers, or treating `.md` as a live CRDT.
- Concurrent WYSIWYG typing while a markdown lease is held.
- Storing pending hunks, rationales, or review threads on published blocks.
- Forking AFFiNE Cloud (NestJS, GraphQL, payments, copilot). OctoBase + y-octo are the backend.
- Full Notion databases, edgeless/whiteboard as a first-class surface.
- Alternative commits, stacked commits, or a comment-commit DAG (v2). See [v1-concerns.md](../drafts/pre-design/v1-concerns.md).

## Three stores

```text
┌─────────────────────────────────────────────────────────────┐
│  Published page                                             │
│  BlockSuite block tree  ← Yjs binary →  OctoBase / y-octo   │
│  (live CRDT, only accepted content)                         │
└──────────────────────────┬──────────────────────────────────┘
                           │ snapshot on commit
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  Git filesystem                                             │
│  folders + *.md + assets                                    │
│  (durable history, revert, LLM/human share)                 │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│  Review session (sibling, not in the published tree)        │
│  Lease + threads + commits + hunks + comment DAG            │
│  OctoBase space (or equivalent synced doc)                  │
└─────────────────────────────────────────────────────────────┘
```

| Store | Holds | Concurrent writes | Lands in git |
|---|---|---|---|
| Published BlockSuite doc | Accepted blocks + rich text | Yes, when no lease | Markdown projection: **snapshot** (WYSIWYG, autocomment) or **comment-commit** (markdown accept). Binary CRDT is not the share format. |
| Workspace catalog | Folder tree + doc identity | Yes (folder moves) | Yes, as directory layout on snapshot or comment-commit |
| Review session | Lease, threads, comment-commits, hunks | Comments yes; markdown body no | Only on markdown **accept** |
| Git repo | Markdown snapshots + assets + commit message | No (one commit at a time, from Venus) | Yes |

The published tree does **not** grow `proposedText`, `hunkStatus`, or `rationale`. Review is a sibling document. See the earlier CRDT-structure conclusion in the transcript.

## Roles of each AFFiNE tool

Venus is **based on** BlockSuite, OctoBase, and y-octo. It is **another tool** on top of them.

| Tool | Role in Venus |
|---|---|
| **BlockSuite** (`@blocksuite/affine`) | Primary editor: page editor, block schema, selection, outline, linked-doc embeds, markdown adapter. |
| **Yjs (browser)** | Client CRDT that BlockSuite already uses. Every doc is a Y.Doc. |
| **y-octo (Rust)** | Server-side Yjs-compatible engine: apply/merge updates, compact snapshots, binary ↔ structured doc, markdown conversion helpers. |
| **OctoBase** | Local-first workspace store + sync (WebSocket / later WebRTC). Workspace → spaces (docs) → blocks. Blob sync for images. |
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
2. **Snapshot clock** — Yjs state vector / OctoBase snapshot id of the published doc at lease acquire. That is `T0`.
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

Live collaboration on that tree is a small CRDT **catalog** in OctoBase (one well-known space, e.g. `venus:catalog`):

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
- Reviewers use [CRDT views](#crdt-views-after-and-before): **After** (how it will look if the comment-commit lands) and **Before** (parent, comments pinned to the right).
- Read-only markdown may refresh from `T0` or the proposal — it is not a replica.
- Only the holder writes proposed markdown.
- Everyone else writes **comments** on the Before view (Google Docs / Jira rail) and workflow (accept / reject).

Acquire snapshots `T0` via OctoBase/y-octo (`get_doc_snapshot` / state vector) and serializes markdown **with the block-id map**.

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

Commit                         // a review proposal, not yet git
  id
  threadId
  parentCommitId?              // stacked: based on another proposal
  baseClock                    // T0, or the parent commit’s result
  author                       // user or agent
  message                      // required rationale (the “why”)
  hunks[]
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

`old` / `new` are **values**, not text CRDTs. The lease holder replaces `new` on regenerate. Storing proposal text as a second Y.Text would reintroduce markdown merge.

### Alternatives and stacks

Post-v1. The data model may keep `parentCommitId` and multiple commits per thread; **do not ship the UI or accept/supersede rules in v1.** Wedge: one lease, one commit, regenerate in place. See [v1-concerns.md](../drafts/pre-design/v1-concerns.md).

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

The same page can be shown as BlockSuite in two versions. This is the version UI, not a second replica.

**1. After** — how the document **will look after this commit** (or already looks, if the commit is the live HEAD).

- **No lease:** After **is** the live published CRDT. Humans edit it (Notion-like). The entire diff since the last git commit is **one snapshot commit** (autocomment) when idle/flush runs.
- **Markdown lease:** After is a **preview CRDT** of `T0` + proposed hunks (not the published Y.Doc). View how the comment-commit will look. v1: preview is read-only; the holder writes markdown. Editing After as WYSIWYG during a lease is later (it would still be **one** comment-commit vs Before).

**2. Before** — the document **before** this comment/commit (last git / `T0`).

- Always a read-only CRDT of the parent.
- **Right rail:** comments pinned to text, like [Google Docs](https://docs.google.com) or Jira — threads on a selection, not hunk cards stuffed into the block tree.
- Markdown comment-commits **must** use this rail for the required why (and replies). Snapshot autocomments do not need a rail.

Toggle After / Before (and Diff) while a lease is held. Without a lease, After is the editor; Before is “last snapshot” for compare if we show it.

## Views

| Mode | When | Who types |
|---|---|---|
| **After (WYSIWYG)** | No lease | Anyone; live CRDT; snapshot+autocomment later |
| **After (preview)** | Lease held | Nobody in v1; proposal CRDT |
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
5. Every **markdown** (review) mutation has a comment-commit (git message + pin when the writer selected one). **WYSIWYG** mutations become snapshot commits with **autocomment** only — no review comment required.
6. Folder identity is `docId`; git path is a projection.
7. Links prefer `docId`, then path.
8. Git is the durable, human-readable history of published snapshots.

## Trust boundary for agents

An agent is a lease holder. It does not write the published CRDT incrementally. It writes a private markdown buffer, emits hunks with rationales, and waits for **human** accept — the same path for spec, plan, DoD, aims, and generated API docs. Humans keep intention; agents do the draft. Humans can comment, request regenerate, or reject without unlocking WYSIWYG.

On plans, a second agent must author DoD; the implementer cannot. See [venus-plan.md](../drafts/pre-design/venus-plan.md).
