# CRDT tree

Collaborative **wiki folder tree**: folders and docs that several tabs (and later users) can rename, reorder, and reparent **at the same time**. The tree is a **Yjs document**, not a React list and not git directories.

**Status:** design. Implement in [M4](../../../M4/README.md) (`step-catalog-crdt` + `step-tree`). Data shape: [datamodel catalog](../../../datamodel/crdt.md#catalog). **Page uuid / docname / filename:** [page-identity](../../../datamodel/page-identity.md) (hub SQL: [hub page-identity](../../backend/hub/page-identity.md)). Hub process: [backend hub](../../backend/hub/). Wire: [CRDT](../../../CRDT/README.md).

Git folders are the **share** layout after Flush (`git mv`). This CRDT is the **live** tree. Sibling order lives here, not in git.

## What it is

Two layers. Do not collapse them.

```text
Catalog CRDT     Y.Doc guid venus:catalog     ← source of truth
        │  observe / transaction
        ▼
CatalogTree      React view + drag-and-drop   ← disposable UI
```

- **Catalog** is a plain Y.Doc (`Y.Map` of nodes), **not** an `affine:page`. Path is not identity; **uuid** / `docId` is. Tree rows show **name** (docname). Git uses a derived POSIX **filename** (`gitPath`).
- **CatalogTree** is a controlled React component. It never owns the tree. Drop, rename, and create call catalog ops (filename filter + hub `page_identity` upsert); Yjs emits updates; the hub fans them out; every tab’s component re-renders from the same map.

AFFiNE’s sidebar is `@affine/core` explorer + OrganizeService. Venus does **not** take that. Outline (BlockSuite headings) is a different TOC.

## How it syncs on the existing hub

The catalog is **one more space** on the wiki the hub already owns. No new Compose service, no catalog REST, no second lease grain.

```text
Tab A CatalogTree     Tab B CatalogTree
        │  drop / rename                          │
        ▼                                         ▼
   catalog Y.Doc                             catalog Y.Doc
   (browser yjs@13.6.32)                     (same guid)
        │  y-protocols/sync  AFFiNE WS            │
        └──────────────►  hub  ◄──────────────────┘
                          Room for workspace_id
                          apply / broadcast / persist
                          **per doc_id** (home, protocol, catalog)
                                │
                                ▼
                          Postgres crdt_* + dirty + page_identity
```

| Hub fact (M3.0) | Catalog (M4) |
|---|---|
| One live owner per `workspace_id` | Unchanged. Catalog is not a second wiki. |
| Room RAM + persist is per `doc_id` | Catalog SQL uuid = `4fe5c16e-4be3-5700-a456-ecc8e86cdf1a` (v5 of `venus:catalog`). Pages keep their own ids. |
| Bare `/collaboration/:workspace` + export | Still **home**. Second docs: **same path** + `?doc=<sql uuid>` ([M4 wire A](../../../M4/plan.md#spaces-and-git)). SharedWorker (step 8) does not change the URL. |
| Dirty trigger on `crdt_update` | Catalog persist upserts `dirty(workspace, catalog_uuid, clock)` like any page. One `jobs` row per wiki. Flush pins catalog **with** dirty pages so `gitPath` matches files. |
| Hub does not walk CRDT items | Still true. Hub never knows “folder” vs “page” from y-octo. **`page_identity` is a separate upsert** on create/rename ([hub page-identity](../../backend/hub/page-identity.md)). |

`SyncProvider.connect('venus:catalog', catalog.spaceDoc)` sits next to `connect('doc:home', page.spaceDoc)` — **two sockets** (A) unless a SharedWorker is holding them for this profile. `mount-editor` imports neither.

Memory mode (`VITE_SYNC_URL` unset): one in-tab Y.Doc; tree still works; second tab does not sync (same as M0). No SharedWorker.

## Concurrent tabs and users

Same merge rules as a published page. Two people typing in WYSIWYG already share a Y.Doc through this hub; the catalog is that loop on a smaller map.

| Action | CRDT | Conflict |
|---|---|---|
| Create folder/doc | `nodes.set(id, …)` + mint **uuid** once + hub insert `page_identity` | Two creates → two uuids. Never mint the same `space_id` on two devices. First `gitPath` is `{folder}/{uuid}.md`. |
| Rename | `name` + derived **filename** / `gitPath` + hub update | Last writer on `name`. Filename: POSIX filter, sibling `_1`. Uuid unchanged. |
| Reparent / reorder | `parentId` + fractional `order` + recompute `gitPath` uniqueness in the new folder | Concurrent moves of **different** nodes merge. Same node: last writer on those keys. |
| Delete | `deleteNode(id)` | **Reject** if any `parentId === id` (`node_not_empty`). UI hides delete. Hub API same error. No cascade, no reparent-on-delete. Home: `home_protected`. Leaf page also drops `page_identity`; Flush `git rm`. If that page was open → **home**. |

**Cycles:** the op rejects a drop onto a descendant **before** the transaction. Reparent onto a `kind: doc` node is also rejected (pages are leaves). If a weird merge still cycles, readers skip the loop and the next edit repairs.

**`gitPath`:** derived from parent **filenames** (not raw `name`) at read time / in the same transaction as rename. Do not treat a cached path as identity. Sidecar `git mv` uses the pin’s map at cut clock T plus DB `page_identity`. Rules: [page-identity](../../../datamodel/page-identity.md).

**Expanded/collapsed** rows are **local UI** (not synced). “Who holds the lease” is M5, not this tree.

**Awareness** (cursor / “Alice is dragging”) is optional later. M4 exit is: drop in A appears in B without reload.

## Node map

Same as [datamodel](../../../datamodel/crdt.md#catalog):

```text
nodes: Y.Map<nodeId, Y.Map>
  id, kind: folder | doc, name, parentId | null,
  order,          // fractional index among siblings
  docId?,         // kind=doc: uuid (created) or doc:home; omitted on folders
  gitPath         // derived from POSIX filenames; cache only, not identity
```

`name` is the tree label (UTF docname). `gitPath` last segment is the POSIX **filename** (escape `/` `\`, sibling `_1`). Docs always end in `.md`. Create: filename = uuid until rename.

**Ids:** created pages `id === docId === uuid`. Home `id`/`docId` = `doc:home`. Seed folder `spec` = **`folder:spec`**. User folders `id` = `folder:` + uuid v4 (not in `page_identity`, not on `?doc=`). Wiki root = `parentId: null` (no stored root node). `kind: doc` means “this row is a page”; `kind: folder` means grouping only. Flush YAML `folders:` is that id ↔ name ↔ folder `gitPath` for clones; live names stay this Y.Map.

Ops (`createFolder`, `createDoc`, `rename`, `reparent`, `setOrder`, `deleteNode`) take **`createAt`** (parent folder id) on create. They run in **one Yjs transaction** (delete only after the children/home checks pass) and **upsert or delete `page_identity` on the hub**. Reparent does **not** touch page bodies. Seed-once after catalog sync: if **`folder:spec`** or **`doc:home`** is missing, write those two (not “map is empty”).

Order: fractional index **string** on the node. Not git. Helper (`generateKeyBetween`) pinned when writing `ops.js` (step 3).

## React component

Host file: `apps/web/src/host/catalog/CatalogTree.tsx` (or `mount-tree` wrapping it). App mounts it in the left slot. Not a BlockSuite widget.

```tsx
<CatalogTree
  catalog={catalogDoc}       // Y.Doc venus:catalog (already synced)
  selectedDocId={openDocId}
  onOpenDoc={(docId) => …}   // rebind editor / header / outline
/>
```

Drop / rename / create **do not** go through props as a new tree object. They call catalog ops, which mutate `catalogDoc`. The component `observe`s `nodes` and re-renders.

`data-testid="venus-tree"`. Click a doc → `onOpenDoc`. Drop a row onto a folder (or between siblings) → `reparent` + `setOrder`.

### Which React tree

The catalog stays ours. The **view** should still be a real tree with ordered drag-and-drop and keyboard, not a hand-rolled `<ul>` and not AFFiNE explorer.

| Candidate | Verdict |
|---|---|
| **AFFiNE `ExplorerTreeRoot`** | No. `@affine/core`, OrganizeService, workbench. Forbidden. |
| **React Aria `Tree`** | A11y only. No ordered tree DnD. We would still write drop. |
| **react-arborist** | Built-in DnD, but opinionated DOM and an internal store that fights Yjs. |
| **@dnd-kit sortable tree** | DnD kit only; we still build tree a11y. AFFiNE left dnd-kit for pragmatic-dnd. |
| **HTML5 DnD custom list** | Small, but keyboard / drop-between / a11y become a second project. |
| **`@headless-tree/react`** | **Yes.** Headless: `dataLoader` reads the Y.Map; `onDrop` writes catalog ops. Ordered DnD, keyboard, rename, `role="tree"`. MIT. No `@affine/core`. Pin Actual **`1.7.0`** (+ `@headless-tree/core@1.7.0`) in [api-map](../../../api-map.md). |

Headless Tree must stay a **view**:

- `getItem` / `getChildren` = snapshot of catalog nodes sorted by `order`.
- `createOnDropHandler` (or equivalent) → `reparent` + fractional `order`. **Do not** let the library keep the canonical children array.
- Expanded set = React state.
- Features: sync data loader + drag-and-drop + selection + hotkeys. **Rename in the tree is required** in M4 (docname → filename filter). Create page/folder: `venus-create-page` / `venus-create-folder`; parent is **`createAt`** (selected folder, or the folder that was right-clicked).

Do not add `@atlaskit/pragmatic-drag-and-drop` unless headless-tree’s bundled DnD fails recon.

## Files (M4)

```text
apps/web/src/host/catalog/
  schema.js          Y.Map nodes
  ops.js             create / rename / reparent / deleteNode / seed-once
  git-path.js        derive filename + gitPath (POSIX filter, sibling _1)
  CatalogTree.tsx    @headless-tree/react view
  CatalogTree.css    host chrome only
```

`mount-editor.js` does not import this folder.

## Invariants

1. Catalog is a Y.Doc on the hub wiki. The React tree is a view.
2. One hub owner per `workspace_id`. Catalog is another `doc_id`, not another owner.
3. Drop is live CRDT. `git mv` waits for Flush.
4. Outline stays headings. This tree stays folders/docs.
5. No `@affine/core`. No catalog REST that *is* the tree. Create/rename **does** upsert hub `page_identity`; delete **does** remove the page row. No markdown in Postgres.
6. Tree and **`venus-page-title`** display **name**. Git and linked-doc hrefs use **gitPath**. Uuid is identity.
7. Delete of a node with children is an error on every path (UI hidden, host op, hub API). No cascade.
