# Page identity (uuid, docname, filename)

**Status:** product rule for [M4](../M4/README.md) (2026-09-15). **Not a new Compose service.**

**Live source of truth is CRDT.** The catalog Y.Doc is the wiki tree (folders, docs, names, `gitPath`, order). Each page’s Y.Doc is the body. `page_identity` is a **Postgres cache written at the git job** from the catalog pin — not a second editor, **not** an HTTP API.

| Owner | What it does | Must not do |
|---|---|---|
| **[CRDT tree](../components/frontend/crdt-tree/)** | Live catalog: `name` (docname), derive **filename** / `gitPath`, mint uuid on `createDoc`. Tree shows `name`. **Only** product path for create / rename / reparent / delete. | Treat path as identity. Scan `wiki/` for rows. Call a page-identity HTTP API (there is none). |
| **[Hub](../components/backend/hub/page-identity.md)** | Holds `page_identity` for sidecar / `ListDocs`. Wire A `?doc=`. Merge does **not** parse catalog. | Parse catalog `Y.Map` on persist. `/api/pages`. Write `wiki/` or YAML. Convert. |
| **Sidecar** (`crates/venus-sidecar`) | On Flush: pin catalog; **Rust y-octo hydrate**; walk `nodes`; `git mv`; overwrite `pages.yaml`; **replace** `page_identity`. Page pins in the same job: `fromDoc` → `.md`. | Trust YAML as uuid truth. Decode catalog in the hub or host JS. `fromDoc` / `MarkdownAdapter` the catalog. |

Hub persist of catalog is **opaque Yjs**. Typed create/rename/delete exist only as host catalog ops (Yjs transactions). SQL and git catch up at the **claimed job** in **Rust** (`venus-sidecar` + y-octo), not on each keystroke, not in `from-doc.js`.

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

Path is not identity. YAML is not identity. **Catalog CRDT** is live truth for the tree (including page uuid = node `id` / `docId`). Postgres `page_identity` lags until the next Flush/idle job. Folder identity stays catalog-only. YAML is the clone naming map from the **same catalog pin**.

There is **no `name` vs `gitPath` product fork.** They are two projections of the same node:

- Tree and markdown link **text** use `name`.
- Git file and markdown **href** use `gitPath` (filenames).
- When the typed name is already a POSIX stem and unique among siblings, they look the same (`protocol` → `spec/protocol.md`). When `/` `\` controls or a sibling clash appear, **filename diverges**; the map stores both. Uuid never changes.

Home is the only pre-existing page. Put it in the same map (`uuid` = `PAGE_SQL_ID`, `docId` = `doc:home`, seed `name` = `home`, seed `gitPath` = `spec/home.md`). Identity never becomes `{uuid}.md`. **Rename** of the docname is allowed; **reparent** is not.

## Create (catalog op)

One product call: **host catalog ops** (`createDoc` / `createFolder`), **not** `mount-editor`, **not** HTTP. Ops **require** `createAt` (parent folder id; `null` = wiki root). No `POST /api/pages`. Memory mode is the same Yjs op. M4 ships **create page / create folder / delete** buttons. e2e clicks `getByTestId('venus-create-page')` / `venus-create-folder` / `venus-delete-node` when `VITE_TESTIDS` is on (Playwright m4). `window.__VENUS_*` may still exist for Vitest. Folders never get a `page_identity` row.

**UI parent**

- Toolbar / default create: **selected folder** is `createAt`. If the selection is a **doc**, use that doc’s `parentId` (the containing folder). If nothing is selected, hide/disable create (no silent `spec` fallback).
- **Right-click a folder:** New page / New folder. `createAt` = that folder’s id.
- `createAt` must be `kind: folder` or `null`. Creating under a page is rejected (pages are leaves).

On create in folder `createAt`:

1. Mint a **new UUID** (v4). That value is `uuid` and `docId`.
2. `workspace.createDoc(uuid)` once; empty `affine:page` seed + `resetHistory()` if the store is empty after sync (same seed-if-empty rule as home, but **no** fixed guid).
3. Catalog node: `kind: doc`, `id` / `docId` = uuid, `name` = uuid string, `parentId` = `createAt`, filename = uuid, `gitPath` = `{parentDir}/{uuid}.md`. Folders: `kind: folder`, `id` = `folder:` + uuid v4, `parentId` = `createAt`, no `docId`, no hub row.
4. Persist of the page Y.Doc uses `(workspace_id, uuid)` on `crdt_*`. Hub apply of catalog bytes does **not** invent a `page_identity` row. SQL + YAML catch up at the next git job from the catalog pin.
5. **`docMetas`:** `workspace.meta.docMetas[uuid].title` = catalog `name` (the uuid string until rename). Live linked-doc cards read this. Git export still post-processes from catalog `name`.
6. **Git:** not yet. First Flush writes `wiki/{gitPath}` (the `uuid.md` file), rewrites `pages.yaml` from the catalog pin, and replaces `page_identity` from that pin.

Do not invent `doc:protocol` in the host. Do not pre-create a second page at boot.

## Seed catalog

After catalog sync, if **`folder:spec`** or **`doc:home`** is missing, write those two nodes (home under spec, `gitPath` `spec/home.md`). Do **not** key seed-once on “map is empty” — two tabs can both see empty and mint two `spec` folders if `spec` used a random id.

- Seed folder: `id: folder:spec`, `kind: folder`, `name: spec`, `parentId: null`.
- Seed home: `id` / `docId`: `doc:home`, `parentId: folder:spec`. Set `docMetas['doc:home'].title` = catalog `name` (`home` until renamed). Affine page title / file H1 stays **Venus**.
- User-created folders still mint `folder:` + uuid v4.

A second seed must not duplicate. Rename of `spec` may change `name` / filename; the id stays `folder:spec`. Home **rename** is allowed; home **reparent** is not ([Home](#home)).

## Home

Identity is always `doc:home` / `PAGE_SQL_ID`. Seed: under `folder:spec`, file `spec/home.md`.

- **Rename** docname: allowed (same filename filter; Flush `git mv`). Header `venus-page-title` and the tree label follow `name`.
- **Reparent:** forbidden. UI does not drop home onto another folder; the op errors `home_protected`.
- **Delete:** forbidden (`home_protected`).
- **Tree:** the `doc:home` row stays **marked and highlighted as home** even when `name` is no longer `home`. `data-testid="venus-tree-home"`. Detect home by `docId`, not by the label string.

## Rename (tree)

User edits the **docname** in the tree (`venus-tree` rename, or equivalent).

1. **Normalize `name`.** Strip unprintable characters. Do not mint a new uuid.
2. **Derive `filename`** from that `name` (rules below). If a sibling already uses that filename, append `_1`, `_2`, … until unique in that folder.
3. Catalog: set `name`; set `gitPath` = `{parentDir}/{filename}.md` (folders: `{parentDir}/{filename}`).
4. Do not change `uuid` / `docId`. SQL `page_identity` is not updated in this op.
5. **`docMetas`:** set `workspace.meta.docMetas[docId].title` = new catalog `name` (home: `doc:home`).
6. **Git on next Flush:** sidecar `git mv` `{old}` → `{new}`; rewrite `pages.yaml` and `page_identity` from the catalog pin. Body clock unchanged → no `fromDoc`.

Reparent (drop) is the same uuid; recompute filename uniqueness in the **new** folder (may add `_1`); update `gitPath`. SQL / git wait for Flush. **Not home:** `reparent` of `doc:home` is `home_protected`. `parentId` stays `folder:spec`. Sibling `setOrder` inside `spec` is allowed. Renaming/moving folder `spec` may change home’s `gitPath` because the ancestor filename changed; that is not reparenting home.

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

Also reject **home** (`home_protected`): **delete** and **reparent**. Empty folder and leaf page (not home) may delete. Folder-with-children: `node_not_empty`. Same tokens on the **host op** (and UI). There is no hub HTTP for this.

On a successful page delete: catalog `nodes.delete` only. The **UI auto-switches to home** whenever the open uuid is not a catalog `kind: doc` — this tab if that page was open, a **second tab** still on it, and a later visit (reload / bookmark / `connect` of that uuid). Do **not** key that check off live SQL (`page_identity` lags until Flush). Header / editor / outline / md pane rebind to home; drop that page’s `?doc=` session. Hub wire A is unchanged: a well-formed uuid still binds (empty or leftover `crdt_*`; **no GC in M4**). The host must not show that uuid as an open page. Git waits for Flush: sidecar sees the uuid missing from the catalog pin → `git rm` the `.md`, rewrite `pages.yaml` from the pin, drop the SQL row. `crdt_*` for that uuid may remain until a later GC; it is not a tree row and not in YAML. Do not create a second catalog node for a deleted uuid.

Folder delete is catalog-only (no `page_identity` row). Next Flush drops that folder from YAML. Empty catalog folders are usually absent as git directories; they **are** listed under YAML `folders:`.

Details: [CRDT tree](../components/frontend/crdt-tree/) · [hub page-identity](../components/backend/hub/page-identity.md).

## Map file in git (projection)

Path: **`wiki/.venus/pages.yaml`**. Full naming map: **pages and folders**.

Sidecar **always overwrites** this file on Flush from **one Rust y-octo walk** of the catalog pin (`crates/venus-sidecar`):

- **`pages:`** catalog `kind: doc` (uuid ↔ docname ↔ git_path).
- **`folders:`** catalog folders (Yjs node id ↔ name ↔ folder `gitPath`). Empty folders appear here even when git has no directory.

Same walk **replaces** `page_identity` so `ListDocs` matches the pin. Not `fromDoc`. Not host JS. Hand-edits of uuid / folder id / path in git are **not** applied back. If YAML is broken, deleted, or lying, the next Flush restores it from the pin. Clones may read it for names; Venus must not treat it as live identity.

`tags` is reserved empty on pages in M4. Sibling **order** is not live truth here (catalog CRDT owns order).

```yaml
# projection; live identity is the catalog Y.Doc; this file is Flush-only
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

## DB (Flush cache, not a second tree)

Table Intent `page_identity` (hub `schema.sql`, [hub slice](../components/backend/hub/page-identity.md)). **Not** live tree truth — catalog CRDT is. The sidecar writes this table at the git job so `ListDocs` can stay SQL without the hub decoding catalog on merge.

```text
workspace_id  UUID  not null
uuid          UUID  not null   -- SQL doc_id, wire ?doc=, file identity
doc_id        TEXT  not null   -- BlockSuite guid; = uuid text for M4-created pages; doc:home for home
name          TEXT  not null   -- docname (tree label, UTF)
git_path      TEXT  not null   -- filename path (…/*.md)
primary key (workspace_id, uuid)
unique (workspace_id, git_path)
```

**No HTTP.** Catalog ops do not upsert this table. Sidecar **replaces** the wiki’s rows from the catalog pin (same decode as YAML). Never YAML → this table. Never YAML → catalog Yjs. If SQL and catalog disagree, **catalog wins** for the live UI; SQL catches up at the next job. Delete **authorization** (`node_not_empty`, `home_protected`) is the host op reading the live catalog — that is not merge.

## Git files

- Every published page is **one** `*.md`. No extensionless wiki pages.
- Create → first Flush: `{folder}/{uuid}.md`.
- Rename → next Flush: `{folder}/{filename}.md` via `git mv` (`filename` from the filter, not raw `name`).
- Delete → next Flush: catalog pin lacks that doc → `git rm` that `.md` and drop the SQL row. Sidecar `.venus/ids/<docId>.json` for that page is removed with the file.
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
   - **Then** catalog shows a new doc whose `name` and file stem are the new uuid; `docId` equals uuid; editor can open it (not home H1 only). SQL / YAML / git may still lack the row until Flush.
   - **When** Flush.
   - **Then** `wiki/spec/<uuid>.md` exists; YAML `pages:` lists `spec/<uuid>.md` → that uuid; YAML `folders:` lists `spec` with `id: folder:spec`. DB `page_identity` has that uuid. YAML page uuid matches DB. Fail if the file was pre-seeded as `protocol.md`. Fail if YAML has a uuid that is not in the catalog pin / DB.

2. **Rename**
   - **When** you rename that row in the tree to `protocol`.
   - **Then** catalog `name` is `protocol`, `gitPath` is `spec/protocol.md`; uuid **unchanged**. SQL / YAML may still show the old name until Flush.
   - **When** Flush.
   - **Then** git has `spec/protocol.md` and **not** `spec/<uuid>.md`; YAML key `spec/protocol.md` with `name: protocol`; DB `name` / `git_path` match; uuid unchanged.

3. **Filter path characters (do not reject)**
   - **When** you rename to `foo/bar`.
   - **Then** tree `name` is `foo/bar` (controls stripped only); filename / `gitPath` is `spec/foo_bar.md`; uuid unchanged.
   - **When** Flush.
   - **Then** git has `foo_bar.md`, not a nested `foo/bar.md`. YAML key `spec/foo_bar.md` with `name: foo/bar`.

4. **Sibling clash**
   - **Given** `protocol` already exists in `spec`.
   - **When** you rename another page to `protocol`.
   - **Then** that page’s catalog filename is `protocol_1.md` (or next free `_n`); uuid unchanged.
   - **When** Flush.
   - **Then** YAML maps `spec/protocol_1.md` → that uuid with `name: protocol`.

5. **YAML is not truth**
   - **When** you corrupt or delete `pages.yaml` and Flush again.
   - **Then** YAML is restored from the catalog pin (`pages:` and `folders:`; `folder:spec` still listed). DB `page_identity` matches that pin. Pages still open by uuid; no new uuid minted; catalog folder ids unchanged.

6. **Home**
   - **Then** `doc:home` still hydrates; one `workspace_lease` owner. Seed file is `spec/home.md` until renamed.
   - **When** you rename home’s docname (e.g. `Welcome`).
   - **Then** tree label / header follow `name`; `[data-testid=venus-tree-home]` is still that row (highlighted as home); uuid / `docId` unchanged. Flush `git mv`s the `.md`.
   - **When** you try to reparent home under another folder.
   - **Then** UI does not complete the drop; op returns `home_protected`; `parentId` stays `folder:spec`.

7. **Delete**
   - **When** you try to delete `spec` while `home` (or any child) is still in it.
   - **Then** UI has no delete; op returns `node_not_empty`; catalog and git unchanged.
   - **When** you create a page, Flush, then delete that leaf (not home).
   - **Then** the row is gone from the catalog immediately; UI auto-switches to **home** if that uuid was open (this tab, a **second tab**, or a later visit). Hub may still bind the uuid. SQL / git / YAML may still list it until the next Flush.
   - **When** Flush.
   - **Then** the row is gone from DB `page_identity`; git no longer has that `.md`; YAML `pages:` has no that uuid. Home still hydrates.

8. **Folders in YAML**
   - **When** you create a folder under `spec`, Flush.
   - **Then** YAML `folders:` lists that folder’s `gitPath` with `id` (`folder:` + uuid) and `name`. Git may have no empty directory.
   - **When** you delete that empty folder, Flush.
   - **Then** that folder is gone from YAML `folders:`. `folder:spec` remains.

### Do not pass if

- Second page existed only because recon hardcoded `doc:protocol`.
- Git wrote `[untitled](./workspace/…)` as the page file name.
- Rename minted a new space / new uuid.
- Flush wrote YAML from a previous git file or from live SQL instead of the catalog pin (`pages:` and `folders:`).
- `foo/bar` created a directory `foo/` in git.
- Two siblings overwrote the same `.md`.
- Delete of a non-empty folder succeeded or cascaded children.
- Home was deleted.
- Home was reparented out of `spec`.
- After delete, the UI stayed on that uuid (blank / leftover CRDT) instead of switching to home (catalog check, not live SQL).
