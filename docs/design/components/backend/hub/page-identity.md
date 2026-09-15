# Hub: page identity

**Not a new process.** The hub already owns Postgres for this wiki. M4 adds one table and an upsert / delete API so create / rename / reparent / delete can store **uuid ↔ docname ↔ git_path** without the merge loop walking the catalog `Y.Map`.

Canonical identifiers, filename filter, YAML shape, and M4 DoD: **[datamodel page-identity](../../../datamodel/page-identity.md)**. Live tree: **[CRDT tree](../../frontend/crdt-tree/)**. Git projection: [datamodel git](../../../datamodel/git.md). Hub still is **not** git, **not** `fromDoc`, **not** `jobs`.

## Why the hub

| Already true (M3.0) | M4 add |
|---|---|
| SQL `doc_id` is UUID; wire A `?doc=` | Created pages **mint** that uuid (v4). Home stays v5 of `doc:home`. |
| Hub does not parse CRDT items | Still true for merge. `page_identity` is an **explicit** row the create/rename/delete API writes. Delete **authorization** may read the catalog snapshot to count children. |
| One owner per `workspace_id` | Unchanged. Mapping is per wiki, many uuids. |

Sidecar **reads** this table on Flush for YAML `pages:`. It writes YAML `folders:` from the **catalog pin** (same pin as `git mv`). The hub does not write YAML.

## Table

Intent `page_identity` in `crates/venus-hub/src/schema.sql` (migrate with the others):

```text
workspace_id  UUID  not null
uuid          UUID  not null   -- PK with workspace; SQL doc_id; ?doc=
doc_id        TEXT  not null   -- BlockSuite guid (= uuid text for created pages)
name          TEXT  not null   -- docname
git_path      TEXT  not null   -- unique per workspace
primary key (workspace_id, uuid)
unique (workspace_id, git_path)
```

Seed/ensure a row for home: uuid `PAGE_DOC_ID`, `doc_id` `doc:home`, name `home`, `git_path` `spec/home.md`. Catalog space is **not** a page row.

## API

The **web host** calls HTTP JSON (same envelope as other hub HTTP; [rpc.md](../../../rpc.md)). The browser does not speak Venus gRPC. **No** `Hub` proto RPC for this table in M4. Sidecar Flush **SELECT**s Postgres. Add gRPC later **if** the sidecar must stop reading SQL.

| Method | Path | When |
|---|---|---|
| `POST` | `/api/pages/:workspace_id` | Create / rename / reparent. JSON `{ uuid, doc_id, name, git_path }` → upsert. Success `{ "data": { …row } }`. |
| `DELETE` | `/api/pages/:workspace_id/:uuid` | `deleteNode` of a **page**. Success `{ "data": { "uuid" } }`. Not used for folders. |

POST/DELETE match existing hub CORS (no PUT). Vite already proxies `/api` to the hub.

- **insert** on create (uuid, name = uuid string, git_path = `{folder}/{uuid}.md`)
- **update** `name` + `git_path` on rename / reparent (uuid unchanged; same POST upsert)
- **delete** the row on `deleteNode` of a page (not folders; folders have no row)

**Delete authorization** (same errors as the host op):

- `home_protected` — refuse home. HTTP **409** (gRPC `FAILED_PRECONDITION` if a later RPC exists).
- `node_not_empty` — any catalog node with `parentId === id`. Same 409. Hub may read the catalog snapshot **for this check only**. It still does not parse catalog for merge or `gitPath`.

Memory mode has no hub; skip these HTTP calls; host `deleteNode` is the check. Do not infer mapping from Yjs persist. Do not `INSERT` from YAML.

Product rules: [datamodel page-identity — Delete](../../../datamodel/page-identity.md#delete).

## Invariants

1. Hub merge still treats `page_identity` as opaque SQL, not as folder vs page in y-octo.
2. YAML → this table is forbidden. This table → YAML `pages:` is sidecar Flush. Catalog pin → YAML `folders:`. YAML → catalog is forbidden.
3. Successful page delete **removes** this row. `crdt_*` for that uuid may remain until a later GC; it is not a catalog node.
