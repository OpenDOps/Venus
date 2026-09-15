# Hub: page identity

**Not a new process.** Postgres table `page_identity` is a **Flush cache** of catalog **doc** nodes (uuid ↔ docname ↔ git_path). Live tree truth is the catalog Y.Doc. The hub merge loop does **not** walk catalog `Y.Map` items — persist is opaque Yjs. There is **no** `/api/pages` HTTP.

The **sidecar** (`crates/venus-sidecar`, after claim) hydrates the **catalog pin** with **y-octo** (Rust) and:

- walks `nodes` (not `fromDoc`, not markdown)
- writes `wiki/.venus/pages.yaml` (`pages:` + `folders:`)
- **replaces** `page_identity` rows for that wiki to match catalog `kind: doc` nodes

That walk is already required for `git mv`. It does **not** run on typing or on hub apply. Page pins in the same job still `fromDoc` to `.md`.

## What is in the plan

Two cheap facts, then when SQL moves:

1. **Catalog vs page persist is already `doc_id`.** Catalog Y.Doc is `CATALOG_DOC_ID`. Page typing persists under that page’s uuid. **One persist tick per Room** drains every buffer; each `INSERT` still uses its own `doc_id`. Filtering “is this catalog?” is that column. It does **not** mean the hub reads the map.
2. **Catalog persist is still opaque `BYTEA`.** Same apply → buffer → `INSERT crdt_update` as a page. Hub does not parse the update, does not observe Y.Map events, does not touch `page_identity`.
3. **`page_identity` + YAML move at Flush, in Rust.** Same git job as page convert. Sidecar (`crates/venus-sidecar`) hydrates the **catalog pin** with **y-octo** and walks `nodes` — not `fromDoc`, not markdown, not host JS, not the hub. Page pins in that job still go through M3 Rust `fromDoc` → `.md`. One decode of the catalog pin writes YAML, replaces SQL, and supplies `gitPath` for `git mv` / `git rm`.

```text
tree op (host Yjs)
  → apply / broadcast / persist catalog BYTEA     ← hub; no SQL; no tree walk
  → Flush / idle job (crates/venus-sidecar, Rust):
       catalog pin → y-octo hydrate → walk nodes → YAML + page_identity + gitPath
       page pin    → y-octo hydrate → fromDoc     → wiki/*.md
```

Typing a page never runs (3). A catalog-only persist tick never runs (3) either — only the git job.

## Not in the plan: parse catalog updates on persist to patch SQL

That would be: persist catalog `doc_id` **and** (in parallel) walk events → incremental `page_identity`. **Rejected.** Cheap filter by catalog id is already (1). The extra work is the problem:

| Idea | Why not |
|---|---|
| Parse the **update v1 blob** for events | It is not an event log. You cannot cheaply read “node X renamed.” Apply already happened in y-octo; the blob is opaque. |
| JS-style **observers** / incremental row patches | y-octo has no product observer API on this path. Folder rename / reparent changes **many** `git_path`s (descendants). Incremental is harder than replacing all page rows from the current map. |
| Same tick as persist, “in parallel” | Persist must stay `INSERT` BYTEA and must not wait on a projection. If SQL is a second statement and fails, catalog bytes are stored and SQL is wrong. If sidecar Flush also writes this table, two writers (live RAM vs pin clock) fight. |
| Rebuild SQL on every catalog persist from RAM | Cheaper than incremental, still a second persist path, still a second writer vs Flush, still stale after restart until the next catalog persist (or you walk the map on hydrate too). M4 does not need live `ListDocs`; the tree uses catalog. |

keck’s Format panic was “hub walks CRDT items.” Catalog is a small map, but M4 still keeps **one** persist path for every `doc_id`. Decode once where git already needs the map.

Canonical identifiers, filename filter, YAML shape, and M4 DoD: **[datamodel page-identity](../../../datamodel/page-identity.md)**. Live tree: **[CRDT tree](../../frontend/crdt-tree/)**. Git projection: [datamodel git](../../../datamodel/git.md). Hub still is **not** git, **not** `fromDoc`, **not** `jobs`.

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

Seed/ensure a row for home at migrate: uuid `PAGE_DOC_ID`, `doc_id` `doc:home`, name `home`, `git_path` `spec/home.md`. Catalog space is **not** a page row. After the first Flush, sidecar overwrites the wiki’s rows from the pin (home included).

`ListDocs` reads this table (lags until the next snapshot/Flush). Live UI uses the catalog Y.Doc, not this table.

## No HTTP

**Do not** add `POST`/`DELETE /api/pages`. Create / rename / reparent / delete are catalog Yjs ops only. Delete **authorization** (`home_protected`, `node_not_empty`) is the **host op** (and UI), not the collab path.

Do not `INSERT` from YAML. YAML and this table are both written from the catalog pin. Never YAML → catalog Yjs. Memory mode: no table; catalog op still runs.

Product rules: [datamodel page-identity — Delete](../../../datamodel/page-identity.md#delete).

## Invariants

1. Hub merge still treats catalog bytes as opaque, not as folder vs page in y-octo.
2. Catalog CRDT is live tree truth. This table and YAML are Flush projections. YAML → catalog is forbidden.
3. After Flush, SQL rows match catalog `kind: doc` at the pin. `crdt_*` for a deleted uuid may remain until a later GC; it is not a catalog node.
