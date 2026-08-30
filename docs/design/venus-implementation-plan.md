# Venus implementation plan

How to build the design in [venus-design.md](./venus-design.md) without taking all of AFFiNE: which packages to use, how they bind, and in what order. Installed symbols (Actual imports, pins, Vite notes): [api-map.md](./api-map.md).

## Principle

Start with a **thin host** around BlockSuite + OctoBase. Add Venus as layers: catalog, git snapshotter, lease, review. Do not start from the AFFiNE web app.

OctoBase is pre-1.0 and AGPL. Use it for **prototyping** (local / single-server). **Replace it before Venus is a cloud service.** Keep the sync provider swappable (Yjs binaries, spaces, blobs — not OctoBase-specific APIs). Licensing: [licensing.md](../legal/licensing.md). v1 scope and the git/WYSIWYG rule: [v1-concerns.md](../drafts/pre-design/v1-concerns.md). Spec-driven plans: [venus-plan.md](../drafts/pre-design/venus-plan.md). Pitch: [pitch.md](../marketing/pitch.md).

**Prototype runtime (every milestone from M1 on):** persist is **Postgres in Docker**. OctoBase **keck is a second Docker**. Compose services are `postgres` + `octobase` (+ `web` when the host is containerized). Do not use SQLite as the product store. Do not put keck and Postgres in one container. Cloud Venus still replaces OctoBase (Hocuspocus / y-websocket + own Postgres); that is not M1.

## Tool choices

### Editor — BlockSuite AFFiNE preset

| Package | Use |
|---|---|
| `@blocksuite/affine` | Page editor, default `affine:*` schema, widgets |
| `@blocksuite/store` | `DocCollection` / workspace, Y.Doc, snapshots |
| `@blocksuite/affine/shared/adapters` | `MarkdownAdapter` |
| Outline widget | In-page heading TOC |
| `affine:embed-linked-doc` | Cross-page cards |
| `affine:embed-synced-doc` | Transclusion |

Host: Vite + React (or vanilla playground first). Mount `AffineEditorContainer` / current page-editor API on a `Store` from the collection.

Do **not** take `@affine/core` explorer, GraphQL, or copilot. Those pull the whole product. A thin **Venus product header** (undo/redo, current page) is host chrome in [M4](#m4--folder-tree--links--product-header-12-weeks), not that package.

### Live CRDT — Yjs in the browser

BlockSuite already owns a Y.Doc per page. Venus does not replace that with a second client CRDT.

### Sync and persistence — OctoBase keck + Postgres (prototype) + y-octo

| Piece | Use |
|---|---|
| **OctoBase keck** | Prototype WS front: space-per-page, blob HTTP, Yjs sync. **Own Docker** (`octobase`). **Not for cloud.** |
| **Postgres** | Prototype persist for Yjs docs **and** blobs. **Own Docker** (`postgres:16`). Named volume. |
| **y-octo** | MIT. Server merge (`merge_updates_in_apply_way`), snapshots, binary parse; OK in cloud |
| **Cloud sync (later)** | Hocuspocus or y-websocket + own persistence; same Yjs provider interface |

Binding:

```text
BlockSuite Store (Y.Doc)
        │  Yjs update binary
        ▼
OctoBase keck  (Compose `octobase`)  WS + blob HTTP
        │  DATABASE_URL
        ▼
Postgres       (Compose `postgres`)  docs + blobs
```

Client: Venus `OctoBaseKeckProvider` (`yjs` + `y-protocols` + subprotocol `AFFiNE`). There is no npm OctoBase client. Do not invent a new CRDT.

Server: keck in Docker, Postgres in another Docker. Venus services (lease, git) sit beside them and read snapshots through y-octo / keck export.

Keep the provider interface swappable from day one: the design depends on Yjs binaries and spaces, not OctoBase-specific block APIs. **M1 implements OctoBase keck only** (not stock `y-websocket`). The **cloud** path is still Hocuspocus / y-websocket + own Postgres (no OctoBase). y-octo may still sit on the server for merge/snapshots.

### Git — libgit2 or `simple-git`, one repo on disk

The wiki root is a real git repository. Venus is the only writer of published commits (users do not `git push` into the live tree in v1). Later: allow PRs from clones as “attach a commit” (import a patch as a review commit).

Use `isomorphic-git` or `simple-git` in Node, or `git2` in a small Rust sidecar next to y-octo. Prefer one sidecar: **y-octo + git2** so snapshot → markdown → commit stays in one process.

### Folder tree — Venus catalog, not a third-party TOC

| Need | Tool |
|---|---|
| In-page headings | BlockSuite outline widget |
| Wiki folders | Venus catalog space in OctoBase + a thin tree UI |
| Files on disk | git working tree |

Do not use a docs-framework TOC (Docusaurus, VitePress) as the live tree. Those assume a static build. Do not use AFFiNE’s explorer.

Tree UI: any accessible tree (e.g. React Aria Tree, or a small custom list). Data comes only from the catalog CRDT. Drop = catalog reparent.

### Product header — Venus chrome, with the folder tree

BlockSuite’s **page** widgets are in-page only: slash menu, selection format toolbar, drag-handle, heading outline. Desktop has **no** persistent undo/redo bar. AFFiNE puts those buttons in `@affine/core`’s header. Venus does not take that shell.

When the wiki tree appears ([M4](#m4--folder-tree--links--product-header-12-weeks)), the host already needs a **layout chrome** (tree left, editor center, outline right). Put a **thin product header** on that same slice:

| In the header | How |
|---|---|
| Undo / Redo | `store.undo()` / `store.redo()`; disable from `store.canUndo` / `store.canRedo` (or `store.history.canUndo$` / `canRedo$`). Same stack as ⌘Z / Ctrl+Z. |
| Current page | Catalog title / `gitPath` of the open doc |

Do **not** build a history timeline. `store.history.undoManager` is a Yjs transaction stack, not labeled “typed hello” / “inserted list.” AFFiNE does not ship that panel either.

Do **not** copy AFFiNE explorer, copilot, or GraphQL. Header is a few host controls on the open `Store`, next to the tree.

Until M4, M0–M3 stay keyboard undo and BlockSuite widgets only.

### Markdown editor (lease holder only)

CodeMirror 6. Private buffer. Not Yjs. Parse/apply through `MarkdownAdapter` + block-id map.

### Review overlay

Custom UI on a **read-only** BlockSuite page:

- **After** — preview CRDT of the pending comment-commit (how it will look).
- **Before** — parent/`T0` CRDT with a **right-rail** of comments pinned to text (Google Docs / Jira).
- **Diff** — hunk overlay.

Do not put hunks or threads in the block schema. Hunk cards may sit in the rail or beside Diff.

### Identity (v1)

Single-workspace local users: display name + id in a config file or OctoBase awareness. Auth can wait. Lease `holder` is that id.

## Bindings (the actual glue)

### 1. Collection ↔ OctoBase workspace

```text
DocCollection.id        = OctoBase workspace_id
collection.createDoc()  = create Space with random space_id (OctoBase rule)
doc.spaceDoc / Y.Doc    = Space CRDT
collection.blobSync     = OctoBase blob store
```

Create spaces **once**, then sync. Never create the same `space_id` independently on two devices.

### 2. Page ↔ git path

```text
catalog.node.docId  →  OctoBase space
catalog.node.gitPath → wiki/spec/crdt/lease.md
```

On accept (comment-commit) or snapshot flush:

1. `MarkdownAdapter.fromDoc` + write block-id sidecar (frontmatter or `wiki/.venus/ids/<docId>.json`).
2. Write `wiki/<gitPath>`.
3. `git add` / `git mv` if `gitPath` changed since last commit.
4. `git commit` — **autocomment** for WYSIWYG snapshots; **required review message** for markdown accept.

Sidecar (recommended) rather than HTML comments in the body:

```json
{
  "docId": "8f3a…",
  "clock": "<state-vector-or-snapshot-id>",
  "blocks": [
    { "id": "b1", "start": 0, "end": 12 }
  ]
}
```

Ranges are rebuilt on every export. Ids are stable.

### 3. Markdown ↔ block tree

Always through BlockSuite, never a hand-rolled mdast → CRDT.

**Export (`T0` or published):**

```ts
const transformer = store.getTransformer([
  titleMiddleware(collection.meta.docMetas),
  docLinkBaseURLMiddleware(collection.id),
  embedSyncedDocMiddleware('content'),
]);
const adapter = new MarkdownAdapter(transformer, store.provider);
const { file } = await adapter.fromDoc(store);
```

**Import (proposal → snapshot → ops):**

```ts
const proposed = await adapter.toDocSnapshot({ file: markdown, assets });
// diff proposed snapshot vs T0 snapshot by block id
// apply: addBlock / deleteBlock / updateBlock / move — not replace the whole Y.Doc
```

Never `Y.applyUpdate` a freshly parsed doc over the live doc. That drops ids and concurrent (or post-lease) identity. Diff by id, emit BlockSuite ops.

### Markdown adapter gate (build this, do not debate it)

Yjs merging a block tree is already BlockSuite’s job. Venus’s share promise is **git markdown an agent can edit**. That lives or dies in `MarkdownAdapter` plus the sidecar, not in CRDT math. This is **implementation work**, not an open product hole.

If we skip it:

- Round-trip changes wording or structure the user never touched → fake hunks, agents “fix” noise.
- Block ids do not survive export/parse → diffs by “paragraph 3,” lease apply corrupts the tree.
- Unknown blocks (colors, some embeds) get dropped or rewritten → git is not a faithful snapshot.
- Links without `venus:doc:…` resolution → clone looks fine, apply breaks embeds.

OctoBase AGPL is a **distribution** constraint ([licensing.md](../legal/licensing.md)), not this. Swapping OctoBase for Hocuspocus does not make round-trip correct.

**Fixture suite (M2 exit, before M6):** not a demo.

1. **Stable subset.** For the round-trippable types Venus documents (paragraph, heading, list, code, links, linked-doc in the stable markdown form), `fromDoc → toDoc → fromDoc` is byte-stable modulo listed whitespace rules.
2. **Sidecar ids.** Every exported block has a stable id in the Venus map (not user-visible syntax). Parse diffs **by id**. Re-export keeps the same ids if the writer did not delete the block.
3. **Opaque blocks.** Anything the adapter cannot name is exported as opaque (HTML comment / raw block). A markdown commit that does not touch that region **must not** rewrite it. Apply is a no-op for those ids.
4. **Loss documented.** Marks/colors/embeds that never round-trip. Agents are told that subset is WYSIWYG-only.
5. **One exporter.** Flush, lease `T0`, and snapshot commits share the same export path as review `old` slices.

Until that suite is green: no alternatives/stacks; do not tell agents “edit the `.md` in git and it will apply.” v1 agent path is lease → private buffer → hunks → accept (still through the adapter). Clone-and-PR import is later.

**Order:** after BlockSuite mounts and syncs, the next slice is **adapter + sidecar + snapshot writer**, not the review DAG. Review UI without a faithful exporter produces commented diffs of adapter jitter.

### 4. Linked docs

Insert `affine:embed-linked-doc` with `pageId = docId`.  
Markdown: adapter + `docLinkBaseURLMiddleware`. Venus post-process adds `<!-- venus:doc:<id> -->` if the adapter does not preserve id.

Resolve on import: id comment → catalog `docId` → path relative to the current file.

### 5. Lease ↔ freeze

```text
acquire(docId, holder)
  → OctoBase: read snapshot clock + snapshot
  → write Lease on review space
  → awareness: { docId, frozen: true, holder }
  → clients: store.readonly = true (or editor readonly extension)
```

Heartbeat on the lease. Steal requires confirm. See [lease-freeze-rationale.md](./lease-freeze-rationale.md).

Review space: one OctoBase space `venus:review:<docId>` or a single `venus:review` doc keyed by `docId`. Comments are a Y.Array (sequence CRDT). Hunk bodies are Y.Map last-writer values.

### 6. Git history ↔ revert

```text
git log -- wiki/<path>
  → pick sha
  → read markdown + sidecar at sha
  → adapter.toDocSnapshot
  → open as review commit vs current T0
  → accept applies ops + new git commit
```

Optional exact restore: save `y-octo` snapshot bytes at `.venus/snapshots/<docId>/<sha>.bin` on each published commit. Revert can load that blob into a scratch doc and still apply via id-diff, or replace if the user wants byte-identical CRDT (rare).

## Milestone plan

### M0 — Empty host (days)

**Status:** done (2026-08-29). Step-by-step: [M0/plan.md](./M0/plan.md). Board: [M0/M0.state.yaml](./M0/M0.state.yaml).

- Vite app, one `DocCollection`, one page, BlockSuite page editor.
- No sync. Prove editor + outline widget.

### M1 — OctoBase loop (week)

**Status:** in progress. Steps 1–3 done (2026-08-30). Step-by-step: [M1/plan.md](./M1/plan.md). Board: [M1/M1.state.yaml](./M1/M1.state.yaml).

- **Postgres** in Docker (`postgres:16`). **OctoBase keck** in a **second** Docker (`octobase`, `DATABASE_URL` → Postgres). Browser talks Yjs over WebSocket (`AFFiNE` subprotocol) to keck only.
- Thin JS client `OctoBaseKeckProvider` behind `SyncProvider`. Not stock `y-websocket` in this milestone.
- Two browser tabs edit the same page.
- Blobs: one image upload (bytes in Postgres via keck HTTP).
- y-octo snapshot API reachable from Venus (keck `GET /api/block/venus-m0/export`).

**Exit:** refresh / second client sees the same page.

### M2 — Markdown projection (week)

- Read-only markdown pane: `MarkdownAdapter.fromDoc` on updates.
- Block-id sidecar generated on export.
- **Adapter gate fixture suite** (see [Markdown adapter gate](#markdown-adapter-gate-build-this-do-not-debate-it)).
- No editable markdown yet.

**Exit:** suite green on the documented subset; WYSIWYG and markdown stay aligned on one client; markdown pane is replaceable (no caret).

### M3 — Git snapshotter (week)

- Init `wiki/` repo.
- Catalog v0: one folder, one doc, fixed path.
- Idle and/or “Flush”: export md, **one** git commit of the full diff, **autocomment** (`snapshot: <title>`). No typed why.
- UI: `git log` for that file (show autocomment vs later comment-commits).

**Exit:** clone `wiki/` elsewhere and read the page as markdown; casual WYSIWYG did not require a review comment.

### M4 — Folder tree + links + product header (1–2 weeks)

- Catalog CRDT: folders, reorder, rename, `gitPath`.
- Tree UI; drop to reparent (live CRDT).
- **Product header** on the same chrome: Undo / Redo on the open page’s `Store`; show the current page name. Not `@affine/core`. Not a history list.
- Layout: header top; folder tree left; page editor; in-page outline stays the heading TOC (not a second wiki tree).
- Publish includes `git mv`.
- `affine:embed-linked-doc` + markdown link round-trip (`docId` + path).

**Exit:** two pages, one link, move a page to another folder, git tree matches, link still resolves. Header undo/redo matches keyboard undo on the open page.

### M5 — Lease + freeze (week)

- Acquire / heartbeat / release.
- Editor readonly + banner while leased.
- Holder opens CodeMirror on the `T0` export.
- No apply yet: cancel drops the buffer.

**Exit:** second user cannot type in WYSIWYG during the lease; they see who holds it.

### M6 — Comment-commit (markdown only) (2 weeks)

- Diff `T0` vs parse(buffer) by block id → hunks.
- Submit **requires** a comment + optional selection pin on **Before** (right rail).
- Review UI: **After / Before / Diff**; Before has pinned comments (Google Docs / Jira).
- Accept → BlockSuite ops → git comment-commit → release.
- WYSIWYG must still only produce snapshot+autocomment (no comment-commit from typing).

**Exit:** a human markdown edit becomes a git commit with a real why and a visible After/Before; WYSIWYG snapshots stay autocommented.

### M7 — Threads, alternatives, stacks (2 weeks)

- Pin-only comment (no hunks) on **Before** (published or `T0`), right rail.
- Replies.
- Attach a later commit to that thread (needs lease).
- Alternative commits (same base) and `parentCommitId` stacks.
- Agent is just another holder: same API (`acquire`, `putHunks`, `comment`).

**Exit:** two competing proposals on one pin; accept one; the other is superseded.

### M8 — Revert + agent loop (week+)

- Revert from git sha as a review commit.
- Regenerate: comment on a hunk → holder replaces that commit’s hunks.
- Optional snapshot blobs in `.venus/snapshots/`.

**Exit:** revert a page to last Tuesday through the same review UI.

## Suggested repo layout

```text
Venus/
  docker-compose.yml        # postgres + octobase (+ web from M1 step 8)
  deploy/octobase/          # Dockerfile: keck from pinned git SHA
  apps/web/                 # BlockSuite host + tree + review UI
  crates/venus-sidecar/     # y-octo + git2: snapshot, export, commit
  packages/catalog/         # catalog schema + ops
  packages/review/          # lease, thread, commit, hunk types
  packages/md-bridge/       # adapter + id map + id-diff → BlockSuite ops
  wiki/                     # git working tree (or separate repo)
  docs/drafts/pre-design/   # these docs
```

OctoBase stays an **external Docker image** (AGPL). Do not vendor it into `apps/web`. Postgres is a **second** image.

## Risks and how M0–M1 de-risk them

| Risk | Mitigation |
|---|---|
| OctoBase JS provider is incomplete | Yjs-protocol shim; keep provider behind one interface |
| MarkdownAdapter drops block ids / jitter | Sidecar + **fixture suite before M6** ([adapter gate](#markdown-adapter-gate-build-this-do-not-debate-it)) |
| Adapter cannot express a block | Opaque raw block; markdown commit must not rewrite it |
| OctoBase AGPL | Accept for server/sidecar; keep Venus app code separate |
| y-octo / Yjs version skew | Pin versions to the pair AFFiNE currently ships |
| Folder move vs dirty git tree | Only `git mv` on publish; catalog may lead git until then |
| Comment anchors after accept | Rebase optional; v1 review threads die with the session unless copied |

## What “simple BlockSuite + OctoBase deployment” means

M0 + M1 only:

1. Docker Compose: **`postgres` + `octobase` + `venus-web`**. Postgres and keck are **separate** containers.
2. One workspace, IndexedDB optional, **Postgres** (via keck) is the source for refresh. SQLite is not the product store.
3. No git, no lease, no catalog.

Everything after that is Venus. Do not block M1 on review design.

## First implementation slice (when coding starts)

1. Playground page editor.
2. OctoBase sync of that one doc (Postgres + keck in Compose).
3. Read-only markdown pane + **adapter fixture suite**.
4. Then catalog + git snapshotter, then **folder tree + product header**, then lease.

Do not start M6 until the adapter gate is green. Snapshot git (autocomment) first, then freeze, then markdown comment-commits.
