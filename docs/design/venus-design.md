# Venus: spec-driven development in the agentic era

Venus is the **tool for spec-driven development** when agents write the code. It is an **LLM-wiki framework**: agentic jobs are the process, on by default, and they frame how you work. It is **not** a second AFFiNE, a Notion with a copilot, or a Cursor clone.

The wiki is the **spec store**. Each published page has two honest representations — not two replicas:

1. **Live collaborative document** — a BlockSuite block tree on a Yjs `Y.Doc`, synced through the **Venus hub** to Postgres.
2. **Git markdown tree** — ordinary folders and `.md` files that humans and LLMs clone. Markdown is a **clocked projection** of a **pin**, not a live CRDT and not an export dump.

```text
humans type WYSIWYG (no lease)     →  snapshot git  (autocomment)
markdown / agent / contract        →  lease + comment-commit  (required why)
                                   →  human meaning-accept
                                   →  published CRDT + git move together
```

**Loop bar:** the spec loop must stay **shorter than Notion + a PR**. Extra gates that do not prevent drift are bugs. Product keep/cut/order: [product-plan](../product/product-plan.md). What is unique (the join): [unique-features](../product/unique-features.md). Pains: [pains](../product/pains.md). Pitch: [pitch](../marketing/pitch.md).

## This document

This is the **product and data design**: what Venus *is*, what bytes mean, lease/git/views, and how agents attach. It is not a milestone plan and not a keep/cut list.

| Need | Where |
|---|---|
| Loop bar, category, uses, force-these, orchestration gap | [product-plan](../product/product-plan.md) |
| Unique join vs neighbors | [unique-features](../product/unique-features.md) |
| Where bytes live (spaces + `wiki/` tree) | [datamodel](./datamodel/README.md) |
| Who talks to whom | [architecture](./architecture.md) |
| Wire (Yjs, hub, export) | [CRDT](./CRDT/README.md) |
| No Rust/WASM in the editor; Rust = hub + `toDoc` worker | [CRDT/wasm.md](./CRDT/wasm.md) |
| Pin + git snapshotter + snapshotter HA | [LiveSnapshot](./LiveSnapshot/README.md) |
| Live CRDT HA (wiki sticky, Rust merge, persist, dirty) | [M3.0/high-availability.md](./M3.0/high-availability.md) |
| Hub fleet (gateway, HPA, shed-then-claim) | [hub-fleet.md](../devops/hub-fleet.md) (devops, after M3.0) |
| Adapter, pane, apply | [MDGate](./MDGate/README.md) |
| Spec graph, bound chat, two gits | [Agents](./Agents/README.md) |
| Tools and milestone order | [venus-implementation-plan.md](./venus-implementation-plan.md) |
| Words (export vs pin vs `T0` vs git snapshot) | [glossary](./glossary.md) |
| Why freeze | [lease-freeze-rationale.md](./lease-freeze-rationale.md) |
| Plan yaml / wiki vs product remotes | [venus-plan.md](../drafts/pre-design/venus-plan.md) |
| License (hub MIT/Apache; M1 keck AGPL) | [licensing.md](../legal/licensing.md) |

Shipped vs story: M0–M3.0 done (editor, wire, markdown pane, Venus hub). Dual store git is **M3** (in progress). M4–M8 (tree, lease, comment-commit, review, revert) are not started. Do not sell graph, bound chat, or apply as shipped.

## Product

### Goal

**Humans keep heart, instinct, faith and intention; agents do the work.** Enforced as: one spec that cannot silently drift.

- A PM, high-level engineer, or CTO writes prose in Venus (WYSIWYG, no Cursor required).
- An agent (or a source write) that **changes contract** goes through **one meaning-accept**.
- Implementers ship from **accepted wiki git**.
- Merge stays red until spec is **current or skipped**.

That path must beat: type in Notion, open a GitHub PR, maybe remember to update the page. Casual WYSIWYG is different: snapshot git + autocomment, no review comment.

**Main wiki feature** (not the loop bar): a [multidimensional spec graph](#multidimensional-spec-graph) — spatial binds + temporal why. Sell after AB4; AB4 needs M6 comment-commits.

### Category

Promote **the way of work first** — spec-driven development in the agentic era, successor to Scrum-as-ceremony — then Venus as the **tool for that work**.

Scrum optimized human coordination (sprint, ticket, Confluence page). Agents do not pull from a standup. The scarce question is **did we ship the world we meant**.

**Jira + Confluence was the agile pair.** Venus replaces that pair **for how software is designed, accepted, and implemented** — not a worse Jira (no kanban home), not an ops wiki. For **technical design**, it replaces Confluence. Notion-like chrome is lightness so the room shows up; the **object** is a cloneable spec. Intake and company ops can stay elsewhere.

Beachhead: teams that already keep spec in `docs/`. Market: teams still running agile-era process while **agents write the code**.

### Uses

| Use | What happens | Not |
|---|---|---|
| **Brainstorm** | Team call on live CRDT docs (no lease). Then agents **fit structure** under lease; humans meaning-accept. | Notion Agent typing the live page. |
| **Share (Cursor stays the IDE)** | `commit` markdown → team WYSIWYG → `git pull`. Venus is the share surface. | Replace Cursor as the editor. Dump/export. |
| **Session-bound commit** | After a live session, a background agent **unites** activity into a commit **bound to that session**. Measurable spec diff. | Idle `snapshot: <title>`. Page-history blob. Chat recap. |

WYSIWYG → git (you pull) is **M3**. Cursor commit → page (they see WYSIWYG) is **M6 apply**. Session-bound commit needs M3 git, then agent lease — do not sell idle flush as “what the call decided.”

### Unique join

Uniqueness is the **join**, not the editor. Catalog: [unique-features](../product/unique-features.md). Design must not ship “AFFiNE + a GitHub workflow.”

| Join | Design consequence |
|---|---|
| Honest dual store | Pin → `fromDoc` → git. Markdown is not a second replica. |
| Agentic-native | Surfaces exist so agents work (git, lease, hunks, Bind, why). Humans **aim**. |
| Agents cannot publish | Lease + meaning-accept. Never type the live CRDT. |
| Skip + merge check | No lease theater on a bugfix. Story not done on PR merge. |
| Accept inbox without Cursor | Tree + WYSIWYG + bless meaning is a complete path for PMs/CTOs. |
| Spec graph | LifeIndexing after git SHA; not on the pin cut. |
| Bound chat | Pack from the graph; AB2 ask-only; AB3 is lease. |
| Spec-bound PR review | `landed ↔ plan ↔ docs` at `{ wikiSha, productSha }`. Analyzer does not implement. |
| Two gits | Wiki remote ≠ product remote. |
| Plan/DoD are spec | Same lease/accept path. Not a ticket checklist. |
| LLM-wiki framework | Index, graph, leased edit, session-bound commit, PR review **on by default**. |

## Non-goals (v1)

- Merging two markdown buffers, or treating `.md` as a live CRDT.
- Concurrent WYSIWYG on the **published** page while a markdown lease is held (After is a sibling hub CRDT).
- Hunks, rationales, or review threads on **published** blocks.
- Forking AFFiNE Cloud (NestJS, GraphQL, copilot, nbstore, Hocuspocus, stock `y-websocket` server).
- Convert / git / `jobs` **inside** the hub.
- Whole-file `toDoc` as apply (mints new ids).
- Teaching agents Yjs.
- Aider / CodeGraph as wiki copilot or as the **implementer**. Cursor CLI writes the branch. Analyzers **review** against the spec.
- Full Notion databases, edgeless as a first-class surface.
- Kanban / cycles / points as the home screen.
- Two-agent DoD as a **hard** v1 block (warn only — a hard rule lengthens the loop).
- Alternatives/stacks **picker UI**, Hugo, Linear mirrors (data may exist; do not ship the ceremony).
- WASM, y-octo, or any Rust **in the client editor**. BlockSuite (Lit + `yjs` on `store.spaceDoc`) is the page. Rust is **only** hub merge and the `toDoc` / pin-convert worker ([CRDT — no client Rust](./CRDT/wasm.md)).

## Runtime stack

Venus is a **thin host** around BlockSuite plus a **Venus-owned collab front**. It is not “OctoBase in cloud and on devices.”

```text
Browser     BlockSuite Store  (yjs 13.6.32 on store.spaceDoc)
            SyncProvider kind venus  (M1: octobase alias)
                 │  y-protocols/sync  ·  subprotocol AFFiNE
                 ▼
Hub         Rust + y-octo: apply + broadcast + persist ~1s
            wiki sticky on workspace_id (lease)
            blob HTTP, doc export (Yjs bytes, not markdown)
                 │  DATABASE_URL
                 ▼
Postgres    crdt_snapshot / crdt_update / blob
            workspace_lease / dirty
                 │  AFTER persist UPSERT dirty
                 ▼
Snapshotter Rust worker: pin copy → y-octo hydrate → fromDoc / toDoc → wiki/ git     (M3, beside the hub)
                 │  after last_flushed
                 ▼
LifeIndexing  spatial (+ later temporal) at that SHA   (AB1 / AB4, async)
```

| Piece | Role | Not |
|---|---|---|
| **BlockSuite** `@blocksuite/affine` **0.22.4** | Page editor, `affine:*` schema, outline, linked-doc, `MarkdownAdapter` | `@affine/core` shell, GraphQL, copilot |
| **Yjs in the browser** | The page CRDT. Venus does not add a second client CRDT. | Markdown-as-Y.Text |
| **Venus hub** | **Rust + y-octo** merge buffer: apply + broadcast + persist ~1s. One live **owner per `workspace_id`** (wiki sticky / lease, not cookie, not `doc_id`). Compose `hub`. MIT/Apache. [M3.0](./M3.0/README.md), [hub HA](./M3.0/high-availability.md) | OctoBase keck, Node product hub, JWST Block REST, `fromDoc`/`toDoc`/git/`jobs` inside the hub |
| **Postgres** | Refresh truth for Yjs + blobs. Venus tables after M3.0. Compose `postgres` | Markdown store |
| **y-octo** | **Required** hub merge (hydrate / apply / compact). Same update v1 as `yjs@13.6.32`. | Client Store; WASM in the editor; AGPL |
| **Convert / `toDoc` worker** | **Rust** process beside the hub: y-octo hydrate of the pin, then M2 `MarkdownAdapter` (`fromDoc` / slice `toDoc`) | Convert in the hub; whole-file `toDoc` onto published; a second adapter dialect |
| **MDGate** | `fromDoc` + sidecar; pane splice; apply is markdown-vs-markdown + hunk slices | Pane splice as `T0` |
| **Git** | Share/history. Venus is the only v1 writer of the live `wiki/` tree | Users `git push` into live |
| **M1 keck** | Proved the wire. **Legacy** after M3.0. AGPL image may remain under `deploy/octobase/` | Product collab front |

Compose after M3.0: **`postgres` + `hub` + `web`**. Same-origin `/collaboration` and `/api`. Live CRDT HA: [M3.0 HA](./M3.0/high-availability.md). Snapshotter fleet: [LiveSnapshot HA](./LiveSnapshot/high-availability.md). On device later: native hub + local SQLite + **WebView BlockSuite** (no Rust in the editor), sync to hosted hub — not a local OctoBase.

`T0` is a **kept pin** (`pinThenFromDoc` / replica encode), not the next `GET …/export`. Export is the **current** tree in Postgres (can trail live RAM by the persist batch).

## Dual store

**Source of truth for layout:** [datamodel](./datamodel/README.md).

```text
Published + catalog + review + Before/After     hub → Postgres
        │  pin Yjs bytes (live never waits)
        │  fromDoc on the pin only
        ▼
wiki/*.md + .venus/ids + assets                 git   (wikiSha)
        │  last_flushed
        ▼
LifeIndexing                                    hidden graph at that SHA
                                                (not the spec)
```

Published blocks never hold hunks or rationales. Review After/Before are real hub spaces. Git is share/history. The index is a photograph of **committed** markdown + sidecar; it must not delay `last_flushed`.

Path is not identity. Catalog `docId` is. `git mv` follows catalog moves on the next commit that includes them.

## Pin then convert

Live collaboration **never** waits on markdown, git, or convert. [LiveSnapshot](./LiveSnapshot/README.md).

| Path | Source | Markdown for | `T0`? |
|---|---|---|---|
| **A — spectator** | Live synced Store | Read-only pane | No. Splice or full `fromDoc`. |
| **B — convert** | **Pinned Yjs bytes** | Git, lease `T0`, review `old` | Yes. Full `fromDoc` on an **offline clone**. |

Do not `fromDoc` the live published Store for git. Do not freeze the pane’s spliced RAM map as `T0`. Do not `Y.applyUpdate` of whole-file `toDoc` onto the published Y.Doc.

## Published document

A page is one BlockSuite doc (one hub space / `docId`):

```text
affine:page
  affine:surface          # present, unused in v1 page mode
  affine:note
    affine:paragraph | affine:list | affine:code | …
    affine:embed-linked-doc | affine:embed-synced-doc
```

Headings on this pin are `affine:paragraph` + `type` `h1`/`h2` (no `affine:heading` flavour).

On top of BlockSuite defaults:

1. **Stable `blockId`s** that survive export → edit → parse. Export writes them into a Venus **sidecar** (not user-visible markdown syntax). Parse diffs by id, not “paragraph 3.”
2. **Snapshot clock (`T0`)** — at lease acquire, pin Yjs bytes (replica encode preferred; idle GET export after persist flush is OK for M3). Keeping that pin is `T0`. The live HTTP GET is not.
3. **Stable `docId`** — hub space id. Git path can change; `docId` does not.

Markdown export is lossy for anything the adapter cannot name (colors, some embeds). Editable markdown is the **round-trippable subset**. Unknown blocks stay opaque and are not rewritten by a markdown commit unless the writer touches them. Gate: [MDGate](./MDGate/README.md).

## Folder tree (table of contents)

There are two different “TOCs.” Do not collapse them.

### In-page outline

BlockSuite’s **outline widget** (heading list for the open page). Use it as-is.

### Wiki folder tree

Canonical **share** layout is the git filesystem:

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

Live collaboration on that tree is a small CRDT **catalog** on the hub ([datamodel — catalog](./datamodel/crdt.md#catalog)):

```text
Catalog
  nodes: Map<nodeId, Node>

Node
  id
  kind            // folder | doc
  name
  parentId        // null = wiki root
  order           // fractional index among siblings
  docId?          // hub space id when kind=doc
  gitPath         // derived, cached: spec/crdt/lease.md
```

Moves = reparent + order. They do **not** rewrite page bodies. Next git commit that includes the move does `git mv`.

Why not “just git” as the live tree: two people moving folders need a merge; git directory-move merge is worse than a tiny CRDT. Why not “just the CRDT”: LLMs and reviewers must see real folders.

Identity: **path is not identity**. Links and leases key by `docId`. Renames do not break `affine:embed-linked-doc`.

Do not use AFFiNE explorer, Docusaurus, or VitePress as the live tree. Product header (undo/redo, current page, who holds the lease) is host chrome with this tree ([M4](./venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks)) — not `@affine/core`.

## Cross-document references

Use BlockSuite blocks, not a Venus hyperlink scheme:

- **`affine:embed-linked-doc`** — card / mention (`pageId` = `docId`).
- **`affine:embed-synced-doc`** — transclude.

Markdown export of a link:

```markdown
[Lease freeze](./crdt/lease.md)
<!-- venus:doc:8f3a… -->
```

Import resolves `venus:doc:…` first, path second. Path-only links from an LLM still work if the catalog can resolve them at apply time.

## Two git classes

Every published markdown tree change Venus writes is a git commit. Do not mix the classes. Details under [Apply and git](#apply-and-git).

| Class | When | Message | Input |
|---|---|---|---|
| **Snapshot** | Idle / Flush / flush-before-lease after WYSIWYG | Autocomment `snapshot: <title>` | Pin of dirty pages + catalog `git mv` |
| **Comment-commit** | Markdown lease **accept** (agent, contract, session-unite, revert) | **Required** why | Hunks vs `T0` |

Idle snapshot is **not** a session-bound commit and **not** temporal why. Filter `snapshot:` in `git log` when you want rationale.

Venus is the only v1 writer of the live tree. Clones do not `git push` into it. Later: import a patch from a clone as “attach a commit.”

Wiki git ≠ product git — [two gits](#two-gits-and-spec-bound-pr-review).

## Lease

Any **user or agent** may request a markdown lease on a page (later: folder batch). One writer. Published WYSIWYG on that page **freezes** until release. Reviewers still read, switch After / Before / Diff, comment, accept, reject.

```text
idle ──acquire──► leased (writer editing markdown / others reviewing)
                    │
                    ├─ accept → apply CRDT → git comment-commit → idle
                    ├─ reject session → drop proposals → idle
                    └─ timeout / steal (confirm) → drop or keep-pending policy
```

Acquire **flush-before-lease** so `T0` matches git HEAD. Pin via replica encode (preferred) or GET export after persist flush. Helper: [pin-convert](./MDGate/pin-convert.md). Why freeze: [lease-freeze-rationale.md](./lease-freeze-rationale.md).

Lease record (review session, **not** the published tree):

```text
Lease
  docId
  holder          // user or agent id
  snapshotClock   // T0
  snapshotTree    // optional frozen Yjs blob (same clock as first Before)
  acquiredAt
  heartbeatAt
```

While leased:

- **Published** BlockSuite is read-only.
- **After** / **Before** are hub docs ([below](#crdt-views-after-and-before)).
- First hunks: holder’s private markdown vs `markdown_T0` + `sidecar_T0`. After submit, After is editable; those edits are the **next** comment-commit.
- Everyone else writes **comments** on the Before view (Google Docs / Jira rail).

Agents **always** lease. They never type the live CRDT. Humans typing prose without contract use WYSIWYG (no lease).

## Comment-commits (markdown only)

Casual WYSIWYG does **not** create comment-commits. It creates snapshots with autocomment.

A **markdown** change to published content is a **comment-commit**: hunks plus a **required** why. That is the only path that must have a review comment. Apply: [MDGate apply](./MDGate/apply.md).

### Two entry points

**A. Markdown edit, then comment**

1. Acquire lease (flush-before-lease).
2. Edit markdown (agent or human). Private buffer — not a CRDT.
3. Venus diffs **markdown_T0 vs proposed markdown** (text), then **attributes** changes onto `sidecar_T0` ranges. Not whole-file `toDoc`. Not “paragraph 3.”
4. Submit: hunks are **one comment-commit**; a **comment is required**.
5. The why is **pinned to a selection** on Before (or After), rail style.

**B. Pin a comment first, diffs later**

1. Select text on **Before**.
2. Pin a comment in the **right rail** (thread). v1 need not create a commit with no hunks.
3. Later, under a lease, attach hunks to that thread.
4. Replies on the rail. Alternative/stacked commits are post-v1 **UI**.

Entry B without hunks does **not** freeze the page.

### Data

```text
Thread
  id
  docId
  anchor
  comments
  commits

Anchor
  { kind: live, blockId, start, end }                    // comment-only, no lease
  { kind: snapshot, clock, side: old|new, hunkId?, start, end }

Commit                         // proposal, not yet published git
  id
  threadId
  parentCommitId?              // over the parent, not a rewrite
  baseClock
  beforeDocId                  // hub space, readonly
  afterDocId                   // hub space, editable until accept
  author                       // user or agent
  message                      // required why
  sessionId?                   // set when this is a session-bound unite
  hunks[]
  status                       // draft | proposed | accepted | rejected | superseded

Hunk
  id
  blockId?                     // null if insert
  afterBlockId?
  kind                         // modify | insert | delete | move
  old / new                    // markdown + tree slice (values, not Y.Text)
  diff
```

After-edits after submit are a **new** commit (`parentCommitId`), not a rewrite. Persist After as soon as hunks exist — not tab RAM. [datamodel — Before/After](./datamodel/crdt.md#commit-before-and-after).

`parentCommitId` / Before / After ids are in the model from the start. **Do not** ship a multi-PR chooser in v1.

### Apply and git

**Snapshot (WYSIWYG, no lease)** — whole CRDT-vs-HEAD diff, **one** commit, autocomment. No review UI.

**Comment-commit (markdown lease)** — published CRDT and git move together on **accept**:

```text
accepted comment-commit + T0
  → BlockSuite ops on the published doc (by T0 block id)
  → fromDoc on the new pin + write wiki/<gitPath>
  → git add / git mv
  → git commit -m "<required why>"
  → archive the review commit (immutable)
  → enqueue LifeIndexing (must not delay last_flushed)
  → release lease if nothing proposed remains
```

Parse only **hunk slices** for `updateBlock` / `addBlock`. Untouched ids are no-ops. Opaque regions the writer did not touch must not be rewritten.

Rollback of a **proposal** is not `git revert` and not CRDT undo. `git revert` undoes an **already published** git commit (re-import that snapshot into the CRDT under lease).

## Session-bound commit

A live call is not done when the page moved. After brainstorm (or another **session**), a background agent sees activity and **unites** it into a **comment-commit bound to `sessionId`**.

```text
humans type the room (no lease)
      →  activity
      →  agent unites under lease (does not type live CRDT)
      →  comment-commit { sessionId, required why }
      →  meaning-accept
      →  spec diff anyone can open
```

Idle snapshot autocomment is **not** this. Prose-only sessions still get a named session commit so the room has a diff — that is the artifact, not a fourth gate on every typo. Contract still meaning-accept. Needs M3 git, then agent lease. Product: [product-plan — session-bound commit](../product/product-plan.md#product-feature-session-bound-commit).

## Snapshots, revert, history

| | Snapshot | Comment-commit |
|---|---|---|
| **Who** | Last WYSIWYG editor (or `venus-snapshot`) | Lease holder |
| **Why** | None (filter `snapshot:` out) | Required. Graph temporal axis. |
| **What** | `git show` | `git show` + rail |

Coalesce WYSIWYG: all typing since last git SHA is **one** snapshot, not one commit per keystroke.

Revert:

1. Choose a git commit (page or tree).
2. Acquire lease (revert **is** a source write).
3. Comment-commit: old = current `T0`, new = historical snapshot; why names the SHA.
4. Accept → apply → git “revert to \<sha\>”. Temporal graph: `reverts`.

Do not replay raw Yjs history as the user-facing version log. Yjs is live merge. Git is “what the wiki said on Tuesday.”

## CRDT views (After and Before)

The same page can be shown as BlockSuite in two **hub** docs. After is not a second published replica and not a tab-local scratch Store.

**1. After** — how the document **will look** if this commit (plus ancestors) is accepted.

- **No lease:** After **is** the live published CRDT. Humans edit it. The entire diff since last git is **one snapshot** when idle/flush runs.
- **Markdown lease:** After is the commit’s **After space** (clone of Before + hunk ops). Synced CRDT. Humans may edit it; those keystrokes are **not** published snapshots. Submitted After-edits are the **next** comment-commit. v1 holder still **creates** first hunks from markdown.

**2. Before** — the document **before** this comment-commit (lease `T0`, or parent After).

- Readonly CRDT of the parent.
- **Right rail:** comments pinned to text (Google Docs / Jira) — not hunk cards in the block tree.
- Markdown comment-commits **must** use this rail for the required why. Snapshot autocomments do not.

Toggle After / Before / Diff while leased. Diff overlays hunks (Cursor-style). Without a lease, After is the editor; Before is “last snapshot” if shown.

Accept is **meaning** on the **rendered** After / Before / Diff — not source-only marks. Cursor’s markdown preview has no diff; that is a pain Venus must close.

## Views

| Mode | When | Who types |
|---|---|---|
| **After (WYSIWYG)** | No lease | Anyone; live published CRDT; snapshot later |
| **After (proposal)** | Lease held | Anyone on the **After space** (not published) |
| **Before** | Compare or lease | Nobody; parent CRDT + **comment rail** |
| **Read-only markdown** | Always | Nobody; `fromDoc` / pane splice |
| **Leased markdown** | Holder only | Holder’s private buffer; parse → hunks, not a CRDT |
| **Diff** | Lease held | Nobody; hunk overlay + rail |
| **Bound chat** | Any | Ask-only until AB3; AB3 is a lease holder in the composer |

The wait state during a markdown lease is this review UI, not a spinner.

## Agentic design

Detail and AB order: [Agents](./Agents/README.md), [agentic-binding](./Agents/agentic-binding.md), [LifeIndexing](./Agents/LifeIndexing.md), [code-bind](./Agents/code-bind.md). This section is the product contract those files implement.

### Native (user-to-agent)

Venus is **agentic-native**. Git, lease, hunks, Bind pack, comment-commit why exist so **agents can work**. Human chrome **aims**: point at a span, bless meaning, skip. It is not a human wiki with a copilot bolted on (Notion Agent, Linear bots).

Agentic jobs are **on by default** (index, graph, leased edit, session-bound commit, spec-bound PR review). Newcomers are not staring at an empty wiki plus “set up an agent.” Included models (Gonka-native Kimi / MiniMax in subscription) are **how the default runs**, not the category. Do not lead with tokens.

Do not teach agents Yjs. Do not ship “copilot in the page” as the native write path.

### Trust boundary

An agent is a **lease holder**. It writes a private markdown buffer, emits hunks with rationales, and waits for **human meaning-accept** — the same path for spec, plan, DoD, aims, and generated API docs.

| Force | If you skip it |
|---|---|
| Spec / plan / DoD / docs **apply is human-only** | Agents become the record |
| Accept is **meaning**, not mute apply | You accepted code that happens to be markdown |
| Story that **moved spec** is not done on PR merge | Linear wins; spec rots |
| **Skip** when spec did not move | Teams abandon the flow |
| Agents do not type the live CRDT | Freeze is theater |
| This audience never **needs** Cursor to bless spec | Only implementers have a heart |

Planner ≠ DoD author ≠ implementer: **v1 warns**; hard refuse is later. On plans, a second agent **should** author DoD. [product-plan — Force these](../product/product-plan.md#force-these-or-it-is-not-the-flow).

### Multidimensional spec graph

**Main wiki feature.** Headings bind to headings **and** to the timeline of accepted whys, keyed by sidecar ids at a **git SHA**. Hidden. Not a fourth source of truth. Not shown as wiki body.

| Axis | Links | Ask |
|---|---|---|
| **Spatial** (AB1, after M3) | Direct links; logical `defines` / `depends-on` / `constrains` / `contradicts` | If I change this, what else is in force? |
| **Temporal** (AB4, after M6) | Same parts → comment-commits + rail (`decided-in`, `supersedes`, `reverts`) | Why is it designed this way? Which SHA do we revert? |

Snapshot autocomments are *what moved*, not why. Do **not** sell the graph as shipped at AB1. Do **not** run LifeIndexing on the pin cut or on live `fromDoc`. Failure of the LLM job leaves git HEAD valid.

Index nodes are **headings** (and page title), not 512-token chunks. Identity: `docId` + `blockId` at `wikiSha`.

### Bound chat and chat-edit

```text
human selects a span
        ▼
Bind { sha, docId, blockIds }     clocked pin
        ▼
pack = LifeIndexing expand        spatial; + why (AB4); + code only if recipe (AB5)
        ▼
chat turn = user text + pack      composer shows the quote, not the dump
```

- **AB2** — ask-only. No wiki write (not CRDT, not git, not `putHunks`). Pack may lag live (`last_indexed`).
- **AB3** — chat-edit **after M5–M6 checkout**. Same composer; publish is still lease + meaning-accept. Not Notion streaming into the published page. Not “AB2 next.”

Citation vs pack: the quote is the **live** selection; the pack is **`last_indexed`**. Do not mix pane splice offsets with stored graph edges.

### Two gits and spec-bound PR review

A workspace pins **two** clocks. Histories stay separate ([code-bind](./Agents/code-bind.md)).

| Tree | What | Clock |
|---|---|---|
| **Wiki** | Spec / plan / DoD / aims | `wikiSha` (`last_indexed` / lease `T0`) |
| **Product** | Application code | `productSha` (named branch pin, not the dirty worktree) |

Cursor **CLI** implements on the product checkout. **CodeGraph CLI and/or Aider** (select before AB5) **review** the PR or branch. They do not publish spec and do not replace Cursor as implementer.

When review runs, Venus builds a **bound pack**, not a repo dump:

```text
landed feature   (PR / branch at productSha)
      bound to
plan step        (accepted wiki plan)
      bound to
documentation    (spec / aims at wikiSha)
```

Review asks: mistakes, **did we follow the spec**, **how close to the goal**, **should wiki (or later Hugo) docs ship with this**. Flags missing docs; does not publish. v1: warning. Merge green stays **spec current or skip**.

Do not pack the codebase on every wiki ask. Do not delay M5 for AB5. Record remotes after M4.

### Plans and DoD

When a plan exists, it is a **wiki page** on the same lease/accept path. DoD is spec, not a ticket checklist. The board is yaml + the plan page; GitHub/Plane are optional mirrors. [venus-plan.md](../drafts/pre-design/venus-plan.md).

v1: not every story is a plan. Two-agent DoD **warns**. Runner, lease MCP, accept inbox are **after** the M0–M8 wiki spine ([product-plan — first-class gap](../product/product-plan.md#add-this--first-class-gap)). Header shows the **gate** (who holds the lease, what waits to accept), not a kanban.

Hugo / autodoc of aims and generated API markdown: same spec tree, **after** the loop bar is green. Publish only accepted git.

### After the wiki spine

M0–M8 makes the dual store and checkout honest. First-class orchestration is then:

- Accept inbox (PMs/CTOs never need Cursor).
- Lease MCP (agents put hunks; humans bless).
- Plan runner that kicks **Cursor CLI**, then spec-bound review.
- Shared checkout of wiki + product remotes (no copy-paste spec into chat).

Those do not replace this design. They consume it.

## Invariants

1. Markdown is never a second published replica.
2. The published block tree contains only **accepted** content.
3. At most one markdown lease per page.
4. While a lease is held, published WYSIWYG is frozen.
5. Every **markdown** (review) mutation is a comment-commit (required why). **Published** WYSIWYG becomes snapshot commits with **autocomment** only. WYSIWYG on a commit **After** space is still review: a **new** comment-commit, not a snapshot and not a rewrite of the parent.
6. Folder identity is `docId`; git path is a projection.
7. Links prefer `docId`, then path.
8. Git is the durable, human-readable history of published snapshots. Wiki git ≠ product git.
9. Live CRDT never waits on markdown, git, convert, or LifeIndexing.
10. One live **hub owner** per `workspace_id` (**wiki sticky** / lease). Not cookie/IP. Not `doc_id` sticky. Snapshotters compete on `jobs` (`SKIP LOCKED`); they do not sit in the hub.
11. Dirty is clocks on `(workspace_id, docId)`, upserted on **persist**, not on apply. The hub does not write `jobs`.
12. Apply is markdown-vs-markdown + sidecar attribution + hunk-slice parse. Never whole-file `toDoc` onto published.
13. **Flow:** spec / plan / DoD / docs publish only on **human** accept of meaning. Agents draft; they do not apply. Skip the lease when spec did not move. [product-plan — Force these](../product/product-plan.md#force-these-or-it-is-not-the-flow).
14. LifeIndexing is not the spec. Snapshot autocomment is not why. AB2 does not write the wiki.
15. Hub **merge** is Rust **y-octo**. Product `toDoc` / pin convert is a **Rust worker** hosting the M2 adapter. **No Rust in BlockSuite** (tab or WebView). Not a Node product hub. Convert is not inside the hub.

## Build order (spine vs parallel)

How to implement: [venus-implementation-plan.md](./venus-implementation-plan.md). Do not invert it.

| Track | Order |
|---|---|
| **Wiki spine** | M0 editor → M1 wire (keck, done) → M2 pane (done) → **M3.0 hub** → M3 git snapshotter → M4 tree + header → M5 lease/freeze → M6 comment-commit apply → M7 review UI → M8 revert |
| **Agentic (parallel)** | AB1 after M3 → AB2 ask-only → AB3 after M5–M6 → AB4 after M6 → AB5 remotes after M4, review after implement |
| **Product orchestration** | After M8: runner, MCP, accept inbox |

Do not start M3 while M3.0 is open. Do not start AB1 before `wiki/` exists. Do not start AB3 because AB2 exists.
