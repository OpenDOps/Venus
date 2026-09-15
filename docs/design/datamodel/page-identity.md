# Page identity (uuid, docname, filename)

**Status:** product rule for [M4](../M4/README.md) (2026-09-15). **Not a new Compose service.** Three existing owners share this map:

| Owner | What it does | Must not do |
|---|---|---|
| **[Hub](../components/backend/hub/page-identity.md)** | Postgres table `page_identity`; upsert on create / rename / reparent API. SQL `doc_id` = uuid. Wire A `?doc=`. | Parse catalog `Y.Map`. Write `wiki/` or YAML. Convert. |
| **[CRDT tree](../components/frontend/crdt-tree/)** | Live catalog: `name` (docname), derive **filename** / `gitPath`, mint uuid on `createDoc`. Tree shows `name`. | Treat path as identity. Scan `wiki/` for rows. |
| **Sidecar** (M3 snapshotter) | On Flush: `git mv` to `gitPath`; overwrite `wiki/.venus/pages.yaml` (`pages:` from DB, `folders:` from catalog pin). | Trust YAML as uuid truth. Use catalog as page uuid identity. |

DoD: [M4 plan step-verify](../M4/plan.md#9-step-verify) **Create → rename → map**. Playwright: `e2e/m4-create-rename.spec.ts`. Symbols: [api-map.md](../api-map.md). Git files: [git.md](./git.md). Catalog fields: [crdt.md](./crdt.md#catalog).

Do **not** mint a second seed page (`doc:protocol` / `spec/protocol.md`). Home stays M3 (`doc:home` → `spec/home.md`). Every other page is **created through the API**, then named in the tree.

## Identifiers

| Name | What | Stable across rename / move? |
|---|---|---|
| **uuid** | Hub SQL `doc_id` (hyphenated UUID). Wire A `?doc=`. Initial file stem. Map key. | Yes |
| **docId** | BlockSuite `createDoc` id / linked-doc `pageId` / `<!-- venus:doc:… -->`. For M4-created pages **= the same uuid string**. Home stays `doc:home` (M3). | Yes |
| **folder id** | Catalog-only. Seed `spec` is **`folder:spec`**. User folders: `folder:` + uuid v4. Not SQL, not `?doc=`, not `page_identity`. | Yes |
| **name** (docname) | Catalog / tree label. UTF. What humans type. Linked-doc `[title]`. | No |
| **filename** | POSIX-safe file stem under the parent folder. Derived from `name` (filter + sibling `_1`, `_2`). | No — follows rename; uuid does not change |
| **gitPath** | POSIX path of the `.md` (or folder) under `wiki/` (no `wiki/` prefix). Docs always end in `.md`. Built from **filename**s, not from raw `name`. | No |

Path is not identity. YAML is not identity. Postgres `page_identity` is identity for **pages** (uuid ↔ docId ↔ git_path ↔ name). Folder identity is the catalog Yjs node id (`folder:spec`, `folder:` + uuid). YAML stores both so clones can name-map pages and folders.

There is **no `name` vs `gitPath` product fork.** They are two projections of the same node:

- Tree and markdown link **text** use `name`.
- Git file and markdown **href** use `gitPath` (filenames).
- When the typed name is already a POSIX stem and unique among siblings, they look the same (`protocol` → `spec/protocol.md`). When `/` `\` controls or a sibling clash appear, **filename diverges**; the map stores both. Uuid never changes.

Home is the only pre-existing page. Put it in the same map (`uuid` = `PAGE_SQL_ID`, `docId` = `doc:home`, `name` = `home`, `gitPath` = `spec/home.md`). Do not rename home to `{uuid}.md` in M4.

## Create (API)

One product call (host catalog ops → hub upsert, **not** `mount-editor`). Ops **require** `createAt` (parent folder id; `null` = wiki root). Hub write is **HTTP JSON**: `POST /api/pages/:workspace_id` (upsert), `DELETE /api/pages/:workspace_id/:uuid`. Envelope: [rpc.md](../rpc.md). No gRPC for this table in M4. Memory mode skips HTTP; catalog op still runs. e2e may use `data-testid="venus-create-page"` or `window.__VENUS_createPage({ createAt })`. Same for folders (`venus-create-folder`) — folders do **not** POST `page_identity`.

**UI parent**

- Toolbar / default create: **selected folder** is `createAt`. If the selection is a **doc**, use that doc’s `parentId` (the containing folder). If nothing is selected, hide/disable create (no silent `spec` fallback).
- **Right-click a folder:** New page / New folder. `createAt` = that folder’s id.
- `createAt` must be `kind: folder` or `null`. Creating under a page is rejected (pages are leaves).

On create in folder `createAt`:

1. Mint a **new UUID** (v4). That value is `uuid` and `docId`.
2. `workspace.createDoc(uuid)` once; empty `affine:page` seed + `resetHistory()` if the store is empty after sync (same seed-if-empty rule as home, but **no** fixed guid).
3. Catalog node: `kind: doc`, `id` / `docId` = uuid, `name` = uuid string, `parentId` = `createAt`, filename = uuid, `gitPath` = `{parentDir}/{uuid}.md`. Folders: `kind: folder`, `id` = `folder:` + uuid v4, `parentId` = `createAt`, no `docId`, no hub row.
4. **Hub:** insert `page_identity` (`workspace_id`, `uuid`, `doc_id` = uuid, `name`, `git_path`). Persist of the Y.Doc still uses `(workspace_id, uuid)` on `crdt_*`.
5. **Git:** not yet. First Flush writes `wiki/{gitPath}` (the `uuid.md` file) and rewrites `pages.yaml` (`pages:` from DB, `folders:` from catalog pin).

Do not invent `doc:protocol` in the host. Do not pre-create a second page at boot.

## Seed catalog

After catalog sync, if **`folder:spec`** or **`doc:home`** is missing, write those two nodes (home under spec, `gitPath` `spec/home.md`). Do **not** key seed-once on “map is empty” — two tabs can both see empty and mint two `spec` folders if `spec` used a random id.

- Seed folder: `id: folder:spec`, `kind: folder`, `name: spec`, `parentId: null`.
- Seed home: `id` / `docId`: `doc:home`, `parentId: folder:spec`.
- User-created folders still mint `folder:` + uuid v4.

A second seed must not duplicate. Rename of `spec` may change `name` / filename; the id stays `folder:spec`.

## Rename (tree)

User edits the **docname** in the tree (`venus-tree` rename, or equivalent).

1. **Normalize `name`.** Strip unprintable characters. Do not mint a new uuid.
2. **Derive `filename`** from that `name` (rules below). If a sibling already uses that filename, append `_1`, `_2`, … until unique in that folder.
3. Catalog: set `name`; set `gitPath` = `{parentDir}/{filename}.md` (folders: `{parentDir}/{filename}`).
4. **Hub:** update `page_identity.name` and `git_path` for that uuid. Do not change `uuid` / `doc_id`.
5. **Git on next Flush:** sidecar `git mv` `{old}` → `{new}`; rewrite `pages.yaml` (pages from DB, folders from catalog pin). Body clock unchanged → no `fromDoc`.

Reparent (drop) is the same uuid; recompute filename uniqueness in the **new** folder (may add `_1`); update `gitPath` / DB. Still Flush before git.

### Docname vs filename (not a conflict)

| | **name** (docname) | **filename** |
|---|---|---|
| Where | Tree, header, linked-doc `[title]`, YAML `name` | `gitPath` last segment, YAML map **key**, git `.md` |
| Characters | Mostly UTF-8. No unprintable / control. `/` and `\` are not path segments — they are **escaped** when deriving filename. Actual: keep in `name` after stripping controls. | POSIX file name: no `/`, no NUL, no `\`. UTF-8 otherwise OK. No leading/trailing `.` or space. Not `.` or `..`. Docs: stem has no `.md` suffix (git appends `.md`). |
| Collision | Two docs may share a similar label | Unique among siblings: `Café`, then `Café_1` |

`gitPath` is **always** `posix.join(parent filenames…, filename + '.md')` for docs. It is not `join(parent names, name + '.md')` when those strings differ. Identity stays uuid.

### Filename filter

Input = trimmed `name` (docname). Then:

1. Drop Unicode general category **Cc** (controls), NUL, and other unprintable scalars (DEL, unpaired surrogates). Do not keep them in `name` either.
2. Replace `/` and `\` with `_` (path separators must not appear in a file stem).
3. Replace any remaining character that cannot be a POSIX file name byte (only `/` and NUL are forbidden on POSIX; we also forbid `\`) — keep UTF-8.
4. Collapse runs of `_`; strip leading/trailing `.` `_` space.
5. If the stem is empty, `.`, `..`, or `*.md` as a whole-name trick: use the uuid hyphenated string as filename (still unique).
6. Among **siblings**, if `filename` exists: `stem_1`, `stem_2`, … (if `stem_1` exists go to `_2`). Never overwrite another uuid’s file.

Do **not** reject the rename because of `/` or a clash. Filter + numeration. Uuid unchanged.

## Delete

M4 includes delete (ops, tree UI, Flush `git rm`). One op: `deleteNode(id)`. Catalog `kind` is **folder | doc**, not two delete APIs:

- **folder** — grouping only. No Y.Doc, no `page_identity`. May have children.
- **doc** — a page (uuid, space, map row, `*.md`). Always a leaf.

Only **folders** (and wiki root) may be parents. Reparent onto a `kind: doc` node is rejected. That is why a page is always a leaf under the same `deleteNode` check — not a second delete API.

Also reject **home** (`home_protected`, HTTP 409 / gRPC `FAILED_PRECONDITION`). Empty folder and leaf page (not home) may delete. Folder-with-children: `node_not_empty` (same status).

On a successful page delete: catalog `nodes.delete`; hub `DELETE` `page_identity` for that uuid. If that page was **open**, switch the open Store to **home** (header / editor / outline / md pane rebind; drop that page’s `?doc=` session). Git waits for Flush: `git rm` the `.md`, rewrite `pages.yaml` (pages from DB, folders from catalog pin). `crdt_*` for that uuid may remain until a later GC; it is not a tree row and not in YAML. Do not create a second catalog node for a deleted uuid.

Folder delete is catalog-only (no `page_identity` row). Next Flush drops that folder from YAML. Empty catalog folders are usually absent as git directories; they **are** listed under YAML `folders:`.

Details: [CRDT tree](../components/frontend/crdt-tree/) · [hub page-identity](../components/backend/hub/page-identity.md).

## Map file in git (projection)

Path: **`wiki/.venus/pages.yaml`**. Full naming map: **pages and folders**.

Sidecar **always overwrites** this file on Flush:

- **`pages:`** from DB `page_identity` (uuid ↔ docname ↔ git_path).
- **`folders:`** from the catalog pin (Yjs node id ↔ name ↔ folder `gitPath`). Empty folders appear here even when git has no directory.

Hand-edits of uuid / folder id / path in git are **not** applied back. If YAML is broken, deleted, or lying, the next Flush restores it (pages from DB, folders from catalog pin). Clones may read it for names; Venus must not treat it as live identity.

`tags` is reserved empty on pages in M4. Sibling **order** is not live truth here (catalog CRDT owns order).

```yaml
# projection; live identity is Postgres page_identity (pages) + catalog Y.Doc (folders)
pages:
  spec/home.md:
    uuid: 395cd07b-bdb1-5f54-ada8-e9a3fabb6a20
    docId: doc:home
    name: home
    tags: []
  spec/a1b2c3d4-e5f6-7890-abcd-ef1234567890.md:
    uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890
    docId: a1b2c3d4-e5f6-7890-abcd-ef1234567890
    name: a1b2c3d4-e5f6-7890-abcd-ef1234567890
    tags: []
folders:
  spec:
    id: folder:spec
    name: spec
```

Page **key** = file `gitPath` (`*.md`). Folder **key** = folder `gitPath` (no `.md`). `name` is the tree label. Folder `id` is the catalog Yjs node id (`folder:spec`, or `folder:` + uuid for user folders). Nested example:

```yaml
folders:
  spec:
    id: folder:spec
    name: spec
  spec/notes:
    id: folder:aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee
    name: notes
```

After rename of the created page to `protocol` (filename still `protocol`):

```yaml
pages:
  spec/protocol.md:
    uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890
    docId: a1b2c3d4-e5f6-7890-abcd-ef1234567890
    name: protocol
    tags: []
folders:
  spec:
    id: folder:spec
    name: spec
```

After rename to `foo/bar` (filename escaped) or a sibling clash (`protocol_1`):

```yaml
pages:
  spec/foo_bar.md:
    uuid: a1b2c3d4-e5f6-7890-abcd-ef1234567890
    name: foo/bar
    tags: []
```

Same uuid. Old `uuid.md` key is gone. YAML page **key** is always the file; `name` is the tree label. Folder rename updates the `folders:` key and `name`; `id` is unchanged.

## DB (source of truth for **page** identity)

Table Intent `page_identity` (hub `schema.sql`, [hub slice](../components/backend/hub/page-identity.md)):

```text
workspace_id  UUID  not null
uuid          UUID  not null   -- SQL doc_id, wire ?doc=, file identity
doc_id        TEXT  not null   -- BlockSuite guid; = uuid text for M4-created pages; doc:home for home
name          TEXT  not null   -- docname (tree label, UTF)
git_path      TEXT  not null   -- filename path (…/*.md)
primary key (workspace_id, uuid)
unique (workspace_id, git_path)
```

Hub writes this on create / rename / reparent **API**, and **deletes** the row on page `deleteNode`. **Not** by parsing YAML. Hub still does not walk catalog Y.Map items for merge. Delete **authorization** may read the catalog snapshot to count children (`node_not_empty`, HTTP 409 / gRPC `FAILED_PRECONDITION`) — that is not merge.

Restore: `SELECT` `page_identity` → YAML `pages:`; catalog pin → YAML `folders:`. Never YAML → this table for uuid. Never YAML → catalog Yjs.

## Git files

- Every published page is **one** `*.md`. No extensionless wiki pages.
- Create → first Flush: `{folder}/{uuid}.md`.
- Rename → next Flush: `{folder}/{filename}.md` via `git mv` (`filename` from the filter, not raw `name`).
- Delete → next Flush: `git rm` that `.md` (row already gone from DB). Sidecar `.venus/ids/<docId>.json` for that page is removed with the file.
- Sidecar ids stay `.venus/ids/<docId>.json` (guid / uuid string), not path-keyed.

## E2E / DoD (M4 must not close without this)

**File:** `apps/web/e2e/m4-create-rename.spec.ts`  
**Command:** `pnpm test:e2e:m4` (Compose `postgres` + `hub` + `web` + sidecar).  
**Board:** [M4 step-verify](../M4/plan.md#9-step-verify) scenario **Create → rename → map**. Fail closed: M4 is not `done` if this spec is skipped or demo-only.

### Given

Compose stack. M0 wiki with seeded **home only** (catalog `spec` + `home`). No `doc:protocol` in the catalog.

### When / Then

1. **Create**
   - **When** you create a page in `spec` (UI `venus-create-page` or the documented API).
   - **Then** catalog shows a new doc whose `name` and file stem are the new uuid; DB `page_identity` has that uuid; `docId` equals uuid; editor can open it (not home H1 only).
   - **When** Flush.
   - **Then** `wiki/spec/<uuid>.md` exists; YAML `pages:` lists `spec/<uuid>.md` → that uuid; YAML `folders:` lists `spec` with `id: folder:spec`. YAML page uuid matches DB. Fail if the file was pre-seeded as `protocol.md`. Fail if YAML has a uuid that is not in DB.

2. **Rename**
   - **When** you rename that row in the tree to `protocol`.
   - **Then** catalog `name` is `protocol`, `gitPath` is `spec/protocol.md`; DB uuid **unchanged**; DB `name` / `git_path` match. YAML has `name: protocol` under key `spec/protocol.md`.
   - **When** Flush.
   - **Then** git has `spec/protocol.md` and **not** `spec/<uuid>.md`; uuid unchanged.

3. **Filter path characters (do not reject)**
   - **When** you rename to `foo/bar`.
   - **Then** tree `name` is `foo/bar` (controls stripped only); filename / `gitPath` is `spec/foo_bar.md`; uuid unchanged. YAML key `spec/foo_bar.md` with `name: foo/bar`.
   - **When** Flush.
   - **Then** git has `foo_bar.md`, not a nested `foo/bar.md`.

4. **Sibling clash**
   - **Given** `protocol` already exists in `spec`.
   - **When** you rename another page to `protocol`.
   - **Then** that page’s file is `protocol_1.md` (or next free `_n`); uuid unchanged; YAML maps `spec/protocol_1.md` → that uuid with `name: protocol`.

5. **YAML is not truth**
   - **When** you corrupt or delete `pages.yaml` and Flush again.
   - **Then** YAML is restored: `pages:` from DB (paths **and** docnames); `folders:` from catalog pin (`folder:spec` still listed). Pages still open by uuid; no new uuid minted; catalog folder ids unchanged.

6. **Home untouched**
   - **Then** `spec/home.md` and `doc:home` still hydrate; one `workspace_lease` owner.

7. **Delete**
   - **When** you try to delete `spec` while `home` (or any child) is still in it.
   - **Then** UI has no delete; op and hub return `node_not_empty`; catalog and git unchanged.
   - **When** you create a page, Flush, then delete that leaf (not home), then Flush.
   - **Then** the row is gone from the tree and from DB `page_identity`; git no longer has that `.md`; YAML `pages:` has no that uuid. Home still hydrates.

8. **Folders in YAML**
   - **When** you create a folder under `spec`, Flush.
   - **Then** YAML `folders:` lists that folder’s `gitPath` with `id` (`folder:` + uuid) and `name`. Git may have no empty directory.
   - **When** you delete that empty folder, Flush.
   - **Then** that folder is gone from YAML `folders:`. `folder:spec` remains.

### Do not pass if

- Second page existed only because recon hardcoded `doc:protocol`.
- Git wrote `[untitled](./workspace/…)` as the page file name.
- Rename minted a new space / new uuid.
- Flush wrote YAML from a previous git file instead of DB `pages:` + catalog-pin `folders:`.
- `foo/bar` created a directory `foo/` in git.
- Two siblings overwrote the same `.md`.
- Delete of a non-empty folder succeeded or cascaded children.
- Home was deleted.
