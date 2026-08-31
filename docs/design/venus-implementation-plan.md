# Venus implementation plan

How to build the design in [venus-design.md](./venus-design.md) without taking all of AFFiNE: which packages to use, how they bind, and in what order. Stores: [datamodel](./datamodel/README.md). Dataflow: [architecture.md](./architecture.md). CRDT stack: [CRDT](./CRDT/README.md). Pin + git snapshotter: [LiveSnapshot](./LiveSnapshot/README.md). Words: [glossary.md](./glossary.md). Installed symbols (Actual imports, pins, Vite notes): [api-map.md](./api-map.md).

## Principle

Start with a **thin host** around BlockSuite. M1 proved the wire on OctoBase **keck**. **[M3.0](#m30--venus-hub-replace-keck-week)** replaces keck with a Venus-owned **hub** (apply + broadcast + persist) before git. Then catalog, snapshotter, lease, review. Do not start from the AFFiNE web app.

`SyncProvider` product kind is `venus` after M3.0 (`octobase` may remain as a wire alias; `memory` for tests). Cloud does **not** switch to Hocuspocus or nbstore. Licensing: [licensing.md](../legal/licensing.md). v1 scope and the git/WYSIWYG rule: [v1-concerns.md](../drafts/pre-design/v1-concerns.md). Spec-driven plans: [venus-plan.md](../drafts/pre-design/venus-plan.md). Pitch: [pitch.md](../marketing/pitch.md).

**Hosted runtime:** persist is **Postgres in Docker**. Collab front is a **second Docker**: M1 **`octobase` (keck)**; **from M3.0 `hub`**. Compose is `postgres` + that front (+ `web`). Do not put the front and Postgres in one container. On-device later: one hub process + local store, sync to hosted hub.

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

### Sync and persistence — Venus hub + Postgres (after M3.0)

| Piece | Use |
|---|---|
| **Venus hub** (M3.0) | WS front: Yjs apply + broadcast + persist, blob HTTP, export. **Own Docker** (`hub`). MIT/Apache Venus code. Not OctoBase. |
| **Postgres** | Hosted persist: Venus `crdt_*` + blobs + `dirty` / `workspace_lease`. **Own Docker** (`postgres:16`). Named volume. |
| **y-octo** | MIT. Optional Rust apply/compact; sidecar decode |
| **M1 (done)** | OctoBase **keck** proved the wire. Legacy after M3.0. Recon: [octobase.md](./LiveSnapshot/octobase.md) |
| **On device (later)** | One hub process + local SQLite; sync to hosted hub |

Binding (M3.0+):

```text
BlockSuite Store (Y.Doc)
        │  Yjs update binary
        ▼
Venus hub      (Compose `hub`)  WS + blob HTTP
        │  DATABASE_URL
        ▼
Postgres       (Compose `postgres`)  crdt_* + blobs
```

Client: `yjs` + `y-protocols` + subprotocol `AFFiNE` (M1 `OctoBaseKeckProvider` or `VenusHubProvider`). Do not invent a new CRDT. Do not use nbstore.

M1: keck in Docker. **M3.0 replaces that process** with the hub; same seam. Venus snapshotter reads export / replica, then pin. Dirty: SQL trigger on hub persist ([M3.0 HA](./M3.0/high-availability.md)). [CRDT — seam](./CRDT/README.md#seam).

**M1 implemented OctoBase keck** (not stock `y-websocket`). **M3.0 implements the Venus hub** (not Hocuspocus, not nbstore).

### Git — libgit2 or `simple-git`, one repo on disk

The wiki root is a real git repository. Venus is the only writer of published commits (users do not `git push` into the live tree in v1). Later: allow PRs from clones as “attach a commit” (import a patch as a review commit).

Use `isomorphic-git` or `simple-git` in Node, or `git2` in a small Rust sidecar next to y-octo. Prefer one sidecar: **y-octo + git2** so snapshot → markdown → commit stays in one process.

### Folder tree — Venus catalog, not a third-party TOC

| Need | Tool |
|---|---|
| In-page headings | BlockSuite outline widget |
| Wiki folders | Venus catalog space on the hub (Y.Doc) + a thin tree UI |
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

Custom UI on BlockSuite pages that are **review spaces**, not the published `Store`:

- **After** — commit After CRDT on the hub (how it will look). Editable; later edits are the **next** comment-commit ([datamodel](./datamodel/crdt.md#commit-before-and-after)).
- **Before** — parent/`T0` CRDT (readonly space) with a **right-rail** of comments (Google Docs / Jira).
- **Diff** — hunk overlay on those two docs.

Do not put hunks or threads in the published block schema. Hunk cards may sit in the rail or beside Diff.

### Identity (v1)

Single-workspace local users: display name + id in a config file or hub awareness. Auth can wait. Lease `holder` is that id.

## Bindings (the actual glue)

### 1. Collection ↔ hub workspace

```text
DocCollection.id        = hub workspace_id
collection.createDoc()  = create space / room with random space_id (mint once)
doc.spaceDoc / Y.Doc    = Space CRDT
collection.blobSync     = hub blob store
```

Create spaces **once**, then sync. Never create the same `space_id` independently on two devices.

### 2. Page ↔ git path

```text
catalog.node.docId  →  hub space / docId
catalog.node.gitPath → wiki/spec/crdt/lease.md
```

On accept (comment-commit) or snapshot flush ([LiveSnapshot](./LiveSnapshot/README.md)):

1. **Pin** dirty docs (Yjs update v1 + catalog) into Venus memory. Live CRDT is not paused.
2. `MarkdownAdapter.fromDoc` **on the pin** + write block-id sidecar (frontmatter or `wiki/.venus/ids/<docId>.json`).
3. Write `wiki/<gitPath>`.
4. `git add` / `git mv` if `gitPath` changed since last commit.
5. `git commit` — **autocomment** for WYSIWYG snapshots; **required review message** for markdown accept.

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

**Import (proposal → hunks → ops):**

Do **not** `toDocSnapshot` the whole buffer and treat that tree as the diff. Diff **markdown vs `markdown_T0`**, attribute with the frozen sidecar, then parse **hunk slices** only. [MDGate apply](./MDGate/apply.md).

```ts
// hunks = attribute(diff(markdown_T0, proposed), sidecar_T0)
// for each hunk: adapter on hunk.new / hunk.old (scratch), not the whole file
// apply: addBlock / deleteBlock / updateBlock / move — not replace the Y.Doc
```

Never `Y.applyUpdate` a freshly parsed doc over the live doc. That drops ids and concurrent (or post-lease) identity. Diff by id, emit BlockSuite ops.

### Markdown adapter gate (build this, do not debate it)

Yjs merging a block tree is already BlockSuite’s job. Venus’s share promise is **git markdown an agent can edit**. That lives or dies in `MarkdownAdapter` plus the sidecar, not in CRDT math. This is **implementation work**, not an open product hole. The design lives in **[MDGate](./MDGate/README.md)** (`fromDoc` + [apply](./MDGate/apply.md)). This section is the accept bar the design must meet.

If we skip it:

- Round-trip changes wording or structure the user never touched → fake hunks, agents “fix” noise.
- Block ids do not survive export/parse → diffs by “paragraph 3,” lease apply corrupts the tree.
- Unknown blocks (colors, some embeds) get dropped or rewritten → git is not a faithful snapshot.
- Links without `venus:doc:…` resolution → clone looks fine, apply breaks embeds.

OctoBase AGPL is a **distribution** constraint for **M1 keck** ([licensing.md](../legal/licensing.md)), not adapter correctness. **M3.0** removes keck from the product path. Neither keck nor the hub makes markdown round-trip correct.

**Fixture suite (M2 exit, before M6):** not a demo.

1. **Stable subset.** For the round-trippable types Venus documents (paragraph, heading, list, code, links, linked-doc in the stable markdown form), `fromDoc → toDoc → fromDoc` is byte-stable modulo listed whitespace rules.
2. **Sidecar ids.** Every exported block has a stable id in the Venus map (not user-visible syntax). Parse diffs **by id**. Re-export keeps the same ids if the writer did not delete the block.
3. **Opaque blocks.** Anything the adapter cannot name is exported as opaque (HTML comment / raw block). A markdown commit that does not touch that region **must not** rewrite it. Apply is a no-op for those ids.
4. **Loss documented.** Marks/colors/embeds that never round-trip. Agents are told that subset is WYSIWYG-only.
5. **One exporter.** Flush, lease `T0`, and snapshot commits share the same export path as review `old` slices.

Until that suite is green: no alternatives/stacks; do not tell agents “edit the `.md` in git and it will apply.” v1 agent path is lease → private buffer → hunks → accept (still through the adapter). Clone-and-PR import is later.

**Order:** after BlockSuite mounts and syncs, **[MDGate](./MDGate/README.md)** (including [subset](./MDGate/subset.md) / [fixtures](./MDGate/fixtures.md)), then **adapter + sidecar + snapshot writer**, not the review DAG. Apply is [apply.md](./MDGate/apply.md) (markdown vs `T0` → hunks → ops) before M6. Review UI without a faithful exporter produces commented diffs of adapter jitter.

### 4. Linked docs

Insert `affine:embed-linked-doc` with `pageId = docId`.  
Markdown: adapter + `docLinkBaseURLMiddleware`. Venus post-process adds `<!-- venus:doc:<id> -->` if the adapter does not preserve id.

Resolve on import: id comment → catalog `docId` → path relative to the current file.

### 5. Lease ↔ freeze

```text
acquire(docId, holder)
  → hub: GET …/export (or state vector); pin as T0
  → write Lease on review space
  → awareness: { docId, frozen: true, holder }
  → clients: store.readonly = true (or editor readonly extension)
```

Heartbeat on the lease. Steal requires confirm. See [lease-freeze-rationale.md](./lease-freeze-rationale.md).

Review session space: `venus:review:<docId>` (lease, threads, hunk **values**). Each commit also has **Before** and **After** hub spaces (BlockSuite Y.Docs, same block ids as `T0`). Comments are a Y.Array (sequence CRDT). Hunk bodies are Y.Map last-writer values on the session space, not on After.

### 6. Git history ↔ revert

```text
git log -- wiki/<path>
  → pick sha
  → read markdown at sha as markdown_prop
  → current pin is T0 (md + sidecar)
  → [apply.md](./MDGate/apply.md): diff files, hunks, review vs T0
  → accept applies ops + new git commit
```

Optional exact restore: save `y-octo` snapshot bytes at `.venus/snapshots/<docId>/<sha>.bin` on each published commit. Revert can load that blob into a scratch doc and still apply via id-diff, or replace if the user wants byte-identical CRDT (rare).

## Milestone plan

### M0 — Empty host (days)

**Status:** done (2026-08-29). Step-by-step: [M0/plan.md](./M0/plan.md). Board: [M0/M0.state.yaml](./M0/M0.state.yaml).

- Vite app, one `DocCollection`, one page, BlockSuite page editor.
- No sync. Prove editor + outline widget.

### M1 — OctoBase loop (week)

**Status:** done (2026-08-30). Step-by-step: [M1/plan.md](./M1/plan.md). Board: [M1/M1.state.yaml](./M1/M1.state.yaml).

- **Postgres** in Docker (`postgres:16`). **OctoBase keck** in a **second** Docker (`octobase`, `DATABASE_URL` → Postgres). Browser talks Yjs over WebSocket (`AFFiNE` subprotocol) to keck only.
- Thin JS client `OctoBaseKeckProvider` behind `SyncProvider`. Not stock `y-websocket` in this milestone.
- Two browser tabs edit the same page.
- Blobs: one image upload (bytes in Postgres via keck HTTP).
- **Doc export:** Venus (or `curl`) can `GET /api/block/venus-m0/export` (current Y.Doc, not markdown, not `T0`).

**Exit:** refresh / second client sees the same page.

### M2 — Markdown projection (week)

**Status:** done (2026-08-30). Step-by-step: [M2/plan.md](./M2/plan.md). Board: [M2/M2.state.yaml](./M2/M2.state.yaml). Contract: [MDGate](./MDGate/README.md) ([subset](./MDGate/subset.md), [fixtures](./MDGate/fixtures.md), [live-pane](./MDGate/live-pane.md)). Stores: [datamodel](./datamodel/README.md) — markdown is a **RAM projection**; Postgres stays Yjs; git sidecar is M3. Hang the pane on [architecture.md](./architecture.md#markdown-projection-add-here-before-coding-m2). Fixture accept bar: [adapter gate](#markdown-adapter-gate-build-this-do-not-debate-it).

- Read-only markdown pane: `MarkdownAdapter.fromDoc` on the **synced Store** ([live-pane.md](./MDGate/live-pane.md) single-flight loop in [M2 `step-loop`](./M2/plan.md#6-step-loop): in-place RAM splice or full export). **highlight.js** paints that string as source (not CodeMirror, not a rendered preview). Not keck `GET …/export`. Git pin is still full `fromDoc` on a pin.
- Block-id sidecar in **RAM** (and test goldens). No `wiki/.venus/ids/`, no Postgres markdown.
- **Export fixture suite** (`rt-*`, `side-*`, `incr-*`, opaque, loss, `one-exporter`, `e2e-pane`). **Apply rows (`ap-*`) are M6.**
- No git, lease, CodeMirror, or editable markdown.

**Exit:** [fixtures.md](./MDGate/fixtures.md) **export** rows green; WYSIWYG and markdown stay aligned on one client; pane is replaceable (no caret); one shared exporter.

### M3.0 — Venus hub (replace keck) (week)

**Status:** not started. Step-by-step: [M3.0/plan.md](./M3.0/plan.md). Board: [M3.0/M3.0.state.yaml](./M3.0/M3.0.state.yaml). Live CRDT HA: [M3.0/high-availability.md](./M3.0/high-availability.md).

Replace OctoBase **keck** with a Venus-owned **merge buffer**: apply Yjs, broadcast, persist ~1s to Venus Postgres tables (`crdt_snapshot` / `crdt_update` / `blob`). Same `AFFiNE` + `y-protocols` wire so M1 e2e stay green. One live owner per `workspace_id` (lease). Dirty SQL trigger on persist (no `jobs` yet). No JWST Block REST, no convert/git in the hub, no nbstore.

**Exit:** Compose **`postgres` + `hub` + `web`** (no `octobase`). Refresh / second tab / image / export work. Lease refuses a second owner. `dirty` upserts. Product collab is not AGPL keck.

Do not start M3 `wiki/` until this milestone is **done**.

### M3 — Git snapshotter (week)

**Status:** not started. **Gate:** [M3.0](./M3.0/README.md) **closed** and [LiveSnapshot/high-availability.md](./LiveSnapshot/high-availability.md) **Acceptance** (snapshotter beside the **hub**; 2026-08-31 “OctoBase stays” is superseded). Do not implement (`wiki/` writer, snapshotter process, git commit from the host) while M3.0 is open or LiveSnapshot HA is un-accepted. An M3 step plan comes **after** this gate.

Design: [LiveSnapshot](./LiveSnapshot/README.md) (pin copy, then convert; do not stall live CRDT). M3 is the **thin column** of [HA — M3 must keep this shape](./LiveSnapshot/high-availability.md#m3-must-keep-this-shape): RAM dirty list, in-process idle/Flush, replica encode or idle GET, one process, one `wiki/`. Same `from-doc.js` as [M2](./M2/README.md) on a **pin** — do not `fromDoc` the live Store for git.

Must not invert HA: dirty is clocks not keystrokes; one job per wiki; cut then convert (cut released before `fromDoc`); not in the hub; not markdown in Postgres; not `fromDoc` every keystroke; not per-block commits.

Parallel track (not this exit): after the commit, [LifeIndexing](./Agents/LifeIndexing.md) (**AB1**, [agentic-binding](./Agents/agentic-binding.md)) may gist/tag/graph dirty pages at that SHA. Do not put an LLM on convert or `last_flushed`. Snapshot git message stays autocomment. Bound chat (**AB2**) starts only after AB1 (**ask-only**; the agent does not write the wiki). Chat-edit (**AB3**) starts only after **M5–M6 checkout**, not when AB2 ships. History/why pack (**AB4**) starts only after **M6** comment-commits, not when AB1 ships; do not treat snapshot autocomment as why. **AB5** (code analyzer on the **product** git, two remotes; **select** CodeGraph CLI / Aider / both) is [code-bind](./Agents/code-bind.md) — not M3, not every wiki request.

- Init `wiki/` repo.
- Catalog v0: one folder, one doc, fixed path.
- Dirty set: only docs whose clock moved since last git (M3: the one page).
- Idle and/or “Flush”: **pin** Yjs bytes (sidecar replica or GET export), **then** `fromDoc` + sidecar, **one** git commit, **autocomment** (`snapshot: <title>`). No typed why. Convert the pin, not the live `Store`.
- UI: `git log` for that file (show autocomment vs later comment-commits).

**Exit:** clone `wiki/` elsewhere and read the page as markdown; casual WYSIWYG did not require a review comment; typing during flush still syncs between tabs.

### M4 — Folder tree + links + product header (1–2 weeks)

- Catalog CRDT: folders, reorder, rename, `gitPath`.
- Tree UI; drop to reparent (live CRDT).
- **Product header** on the same chrome: Undo / Redo on the open page’s `Store`; show the current page name. Not `@affine/core`. Not a history list.
- Layout: header top; folder tree left; page editor; in-page outline stays the heading TOC (not a second wiki tree).
- Publish includes `git mv`.
- `affine:embed-linked-doc` + markdown link round-trip (`docId` + path).

**Exit:** two pages, one link, move a page to another folder, git tree matches, link still resolves. Header undo/redo matches keyboard undo on the open page.

**After M4 (not this exit):** record the workspace’s **wiki remote + product remote@branch** (separate histories; submodules lean). Do **not** delay M5 for the analyzer. **Before AB5:** select CodeGraph CLI, Aider, or both ([code-bind — select](./Agents/code-bind.md#select-codegraph-cli-or-aider-or-both)). Design: [code-bind](./Agents/code-bind.md). AB5 / runner review: [agentic-binding — AB5](./Agents/agentic-binding.md#ab5--code-bind--aider), [product-plan](../product/product-plan.md#workspace-and-aider).

### M5 — Lease + freeze (week)

- Acquire / heartbeat / release.
- Editor readonly + banner while leased.
- Holder opens CodeMirror on the `T0` export.
- No apply yet: cancel drops the buffer.

**Exit:** second user cannot type in WYSIWYG during the lease; they see who holds it.

### M6 — Comment-commit (markdown only) (2 weeks)

Design: [MDGate apply](./MDGate/apply.md) (markdown vs `T0` + sidecar → hunks; not whole-file `toDoc`).

- Diff `markdown_T0` vs buffer; attribute with frozen sidecar → hunks. Overlay on **Before** WYSIWYG (Cursor-style).
- Submit **requires** a comment + optional selection pin on **Before** (right rail).
- Review UI: **After / Before / Diff** on those hub docs; Before has pinned comments (Google Docs / Jira). After is persisted, not a scratch Store.
- Accept → BlockSuite ops by hunk `blockId` (sequence order) → git comment-commit → archive After/Before → release.
- WYSIWYG must still only produce snapshot+autocomment (no comment-commit from typing).

**Exit:** a human markdown edit becomes a git commit with a real why and a visible After/Before; WYSIWYG snapshots stay autocommented.

Parallel (**AB4**, not this exit): [history/why pack](./Agents/agentic-binding.md#ab4--history--why-pack) may start **after this** — real comment-commit whys exist. Do not start AB4 because AB1 exists. Spatial AB1 is not pain 7.

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

## v1 dogfood — document the product

v1 of this plan does **not** need the runner or MCP to be useful. It needs to be the place **this product is documented** — for PMs, other managers, high-level software engineers, and CTOs who will never open Cursor, and for implementers who clone git.

**Audience split (force this):** that audience lives in Venus (tree, WYSIWYG, later accept). Implementers live against **accepted wiki git** (Cursor optional). If documenting Venus requires an IDE, v1 has failed the flow.

### Minimum useful (after M4)

Ship M2 → **M3.0** → M3 → M4, then **author the product in the wiki**, not only in `docs/` as a side tree.

| Need | Milestone |
|---|---|
| Honest markdown on disk | M2 adapter gate + M3 snapshot git |
| Folders PMs can navigate (spec, design, product, aims) | M4 catalog + tree + header |
| Casual writing without a review ceremony | WYSIWYG → snapshot + autocomment |
| Programmers read the same pages | `git clone` / pull of `wiki/` |

**Exit for dogfood:** a manager can create and edit pages in the tree; a second browser sees them; a clone of `wiki/` is readable markdown. Venus’s own pitch, product plan, and design notes can live (or be mounted) as wiki pages. `docs/` in this git may still be the engineering notebook until that move; the **product** docs that managers own must have a wiki home.

Do not wait for M7–M8. Do not wait for Hugo. Clone + live wiki is v1 documentation.

### Meaning-accept (after M6)

When a spec page **changes intention**, v1 must use comment-commit: After / Before / Diff + required human-language why. That is the PM review cycle (CodeSpeak-shaped, on spec). Casual typos stay snapshots.

**Exit:** a manager can accept (or reject) a spec change without opening Cursor or reading a chat. Git log on that file has a real why.

### Not in v1 dogfood

Runner, two-agent DoD, MCP, Hugo, alternatives/stacks, **analyzer review**. Identity can stay display name + id ([Identity (v1)](#identity-v1)). Named people (not `user1`) so a PM can see who holds a lease. After M4 you **may** record product remote + branch; you do not run Aider or CodeGraph in dogfood.

Header in M4 should already show **current page**. As soon as M5 exists, show **who holds the lease**. That is manager chrome, not programmer chrome.

## Suggested repo layout

```text
Venus/
  docker-compose.yml        # postgres + hub + web (M1: octobase)
  deploy/hub/               # Dockerfile: Venus merge buffer
  deploy/octobase/          # M1 keck image (legacy after M3.0)
  deploy/web/               # nginx + static host
  apps/web/                 # BlockSuite host + tree + review UI
  crates/venus-sidecar/     # y-octo + git2: decode export, then md + commit
  packages/catalog/         # catalog schema + ops
  packages/review/          # lease, thread, commit, hunk types
  packages/md-bridge/       # adapter + id map + id-diff → BlockSuite ops
  wiki/                     # git working tree (or **separate remote**; product code is another remote / submodule)
  docs/design/              # product + architecture + datamodel + CRDT + MDGate + milestone plans
  docs/devops/              # Compose now; Kubernetes later
  docs/drafts/pre-design/   # pitch-era notes (v1-concerns, venus-plan)
```

After M3.0 the collab image is **Venus hub** (not AGPL). Do not vendor OctoBase into `apps/web`. Postgres is a **second** image. M1 keck Dockerfile may remain under `deploy/octobase/` as history.

## Risks and how M0–M1 de-risk them

| Risk | Mitigation |
|---|---|
| OctoBase JS provider is incomplete | M1 shim; M3.0 hub keeps the same `y-protocols` interface |
| MarkdownAdapter drops block ids / jitter | **[MDGate](./MDGate/README.md)**; **export fixtures before M2 exit**; **apply fixtures before M6** ([adapter gate](#markdown-adapter-gate-build-this-do-not-debate-it)) |
| Adapter cannot express a block | Opaque raw block; markdown commit must not rewrite it |
| OctoBase AGPL | M1 keck only; M3.0 hub is Venus-owned. Keep app code separate either way |
| y-octo / Yjs version skew | Pin versions to the pair AFFiNE currently ships |
| Folder move vs dirty git tree | Only `git mv` on publish; catalog may lead git until then |
| Comment anchors after accept | Rebase optional; v1 review threads die with the session unless copied |

## What “simple BlockSuite + hub deployment” means

M0 + M1 proved editor + keck. **M3.0** is the product collab front:

1. Docker Compose: **`postgres` + `hub` + `venus-web`**. Postgres and hub are **separate** containers. keck is not required.
2. One workspace, IndexedDB optional, **Postgres** (via hub) is the source for refresh. SQLite is not the product store.
3. No git, no lease (product freeze), no catalog — those stay M3+.

Everything after M3.0 git is Venus snapshotter. Do not block M3.0 on review design.

## First implementation slice (when coding starts)

1. Playground page editor. **Done** — [M0](./M0/README.md).
2. OctoBase sync of that one doc (Postgres + keck in Compose). **Done** — [M1](./M1/README.md).
3. **[MDGate](./MDGate/README.md)** design is written. **Code:** [M2/plan.md](./M2/plan.md) — read-only markdown pane + **export fixture suite**. Apply: [apply.md](./MDGate/apply.md) before M6.
4. **Venus hub** — [M3.0/plan.md](./M3.0/plan.md). Replaces keck. HA: [M3.0/high-availability.md](./M3.0/high-availability.md).
5. **Then** git snapshotter ([LiveSnapshot](./LiveSnapshot/README.md) — M3 is the thin of that shape), gated on M3.0 **done** + [LiveSnapshot HA](./LiveSnapshot/high-availability.md) Acceptance. Then **folder tree + product header**, then lease. Do not start M3 code if M3.0 is open.

Do not start M2 **exit** until [fixtures.md](./MDGate/fixtures.md) export rows are green. Do not start M6 until apply rows are green ([apply.md](./MDGate/apply.md)). Snapshot git (autocomment) first, then freeze, then markdown comment-commits.

**v1 documenting this product:** after M4, dogfood the wiki as the manager-facing spec store; after M6, spec-intention changes use comment-commit. Details: [v1 dogfood](#v1-dogfood--document-the-product).

After the wiki spine (M8): runner, lease MCP, accept inbox, shared git hop — [product-plan](../product/product-plan.md). Bound chat (AB2) and chat-edit (AB3) are parallel: [agentic-binding](./Agents/agentic-binding.md). **Two gits + analyzer** (AB5): record remotes after M4; **select** CodeGraph CLI, Aider, or both; review product diffs after plan-step implement — [code-bind](./Agents/code-bind.md), [product-plan — Workspace and Aider](../product/product-plan.md#workspace-and-aider). Those files are not this milestone list. Flow invariants to force: [product-plan — Force these](../product/product-plan.md#force-these-or-it-is-not-the-flow).
