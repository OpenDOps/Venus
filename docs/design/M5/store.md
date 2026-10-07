# Store — SurrealDB

Graph database for [extract](./extract.md) and [connect](./connect.md). **SurrealDB 2**, own Compose service, own volume. Not a table in the hub Postgres. Not a file under `wiki/` (a clone stays markdown + sidecar; agents that need the graph talk to Venus).

## Why this engine

Pages, headings, mentions, and typed edges are one dataset: records plus `RELATE`. Traversal (`->links_to->page`, `<-mentions`) is the pack query. Word search is a projection onto the `surreal-search` cluster after this commit ([search-scale](./search-scale/README.md)). Exact `norm` lookups stay here. Search nodes run the full SurrealDB server and the search schema only.

Vectors are a later index on `heading` if candidate recall needs them. They are not required to create edges.

## Process

```text
Compose (profile graph)
  postgres          crdt_* + blob + dirty / jobs     (unchanged)
  hub               Yjs apply + broadcast            (unchanged)
  sidecar           pin → fromDoc → git              (unchanged; enqueues graph)
  surreal-graph     namespace graph, a copy of each wiki graph   (this design)
  surreal-graph-1/2 more graph copies in HA (pool graph, ack 2 of 3)
  surreal-search-0  a copy of each search shard                  (this design)
  surreal-search-1  a copy of each search shard
  graph worker      crates/venus-graph + surrealastic
                    one write per job; the layout applies shards
```

| | |
|---|---|
| Image | `surrealdb/surrealdb:v2.7.0` |
| Command | `start --bind 0.0.0.0:8000 --user venus --pass … rocksdb:/data/graph.db` |
| Volume | `surreal-graph-data` → `/data` |
| Listen | Graph `127.0.0.1:8000`. Search nodes `127.0.0.1:8001` and `:8002`. Not published wide. |
| Client | Rust crate `surrealdb` `2.7.0` in `crates/venus-graph` (`protocol-ws`, `rustls`, no embedded RocksDB). WebSocket from the `graph` service only |

Namespace `graph`, database = `workspace_id` (one database per wiki). That database is a **replica set**: 1 copy in dev, 3 in HA, acked on 2 ([scale — replication layer](./scale.md#the-replication-layer)). Every write goes through `crates/surrealastic`; each copy also holds `_repl:state` and `_repl_log` ([scale — the log](./scale.md#the-log)). The search projection is namespace `search` on the nodes in [`repl_copy`](./scale.md#postgres-tables). A second wiki must not share this database.

The hub **does not** get a SurrealDB client. The browser **does not** open SurrealDB. AB2 will call a small read API on `venus-graph` (or in-process query) later. Until then the accept bar is the data in SurrealDB, checked by tests.

Auth is the same leftover as the hub ([implementation plan — identity](../venus-implementation-plan.md#identity-v1)): local / trusted network. Do not design OIDC in this folder. Credentials are Compose env, not a new product login.

Job queue is a new Postgres table `graph_jobs` (one row per workspace: `wiki_sha`, dirty doc ids), claimed `SKIP LOCKED`, inserted **after** `last_flushed` commits. The flush `jobs` row stays the snapshotter’s (primary key `workspace_id`, reason `idle` | `flush` | `lease`). One graph writer per workspace. Plan: [search-scale/plan.md](./search-scale/plan.md#6-step-enqueue).

## Clocks

The working graph is the **latest successful index of each page**, not a full copy per git SHA.

| Record | Clock |
|---|---|
| `page.indexed_sha` | SHA whose markdown was extracted |
| `page.search_sha` | Column remains. The projection clock is the commit `lsn` the shard has applied ([scale — the layout](./scale.md#the-layout)). The owner does not stamp this from a shard ack, and does not retry from `search_error`. |
| `page.hkey` | First 8 bytes of `sha256(docId)`, big-endian, shifted right by 1 (63 bits). `hkey % shard_count` is the search shard ([scale](./scale.md#shards)). |
| `page.pass` | Monotonic int; mentions and extractor edges from older passes are deleted |
| `page.body` hashes on headings | Skip extract + model when unchanged |
| `graph_meta:workspace` | `schema` (int), `glossary_id` (blob hash), `ner` (bool), `wiki_sha` (last job SHA, even if some pages lag) |
| `last_indexed[docId]` | Also the LifeIndexing clock: `{ gitSha, incrementalCount, lastFullAt }`. Lives on the `page` row. |

A lease pack uses this graph only when `page.indexed_sha` is the `T0` SHA or an ancestor that still matches the heading `body_hash`. If the index lags, AB2 says so. Do not mix live pane offsets into `start` / `end`.

Direct edges at an **old** SHA are re-derived by reading that commit’s markdown (they are not a second stored graph). Model edges are whatever the last successful semantic pass wrote, stamped with the SHA they came from (`indexed_sha` on the edge). AB4 may add commit nodes later; do not snapshot the whole graph on every flush.

Poison page: set `page.index_error`, leave the previous pass in place, continue the wiki job.

Schema bump (`graph_meta.schema`): full re-extract. Model edges rebuild only after extract succeeds.

## Schema

Ids:

| Node | Record id |
|---|---|
| Page | `page:⟨docId⟩` |
| Heading | `heading:[docId, blockId]` |
| Term | `term:[glossaryDocId, blockId]` |
| Symbol | `symbol:[docId, lang, kind, name]` |
| Mention | `mention:[docId, blockId, kind, start]` |

`person` / `org` / `team` are mention kinds joined on `norm`. They are not records of their own.

`docId` and `blockId` are the catalog id and the sidecar id. Path is a field, never the id.

Edges are relation tables. Every edge carries `from_doc`, `indexed_sha`, `source`. A job writes one doc’s rows and that doc’s outbound edges only, with explicit ids, so a log entry gives the same result on every copy and when replayed ([scale — guarded write](./scale.md#guarded-write)).

```surql
DEFINE NAMESPACE graph;
DEFINE DATABASE ⟨77e4a2b1-8b40-5979-a73c-fd4477216d00⟩;

DEFINE TABLE graph_meta SCHEMAFULL;
DEFINE FIELD schema       ON graph_meta TYPE int;
DEFINE FIELD glossary_id  ON graph_meta TYPE option<string>;
DEFINE FIELD ner          ON graph_meta TYPE bool DEFAULT false;
DEFINE FIELD wiki_sha     ON graph_meta TYPE option<string>;
DEFINE TABLE page SCHEMAFULL;
DEFINE FIELD git_path     ON page TYPE string;
DEFINE FIELD title        ON page TYPE string;
DEFINE FIELD indexed_sha  ON page TYPE string;
DEFINE FIELD search_sha   ON page TYPE option<string>;
DEFINE FIELD search_error ON page TYPE option<string>;
DEFINE FIELD hkey         ON page TYPE int;
DEFINE FIELD pass         ON page TYPE int;
DEFINE FIELD gist         ON page TYPE option<string>;
DEFINE FIELD index_error  ON page TYPE option<string>;
DEFINE FIELD semantic_error ON page TYPE option<string>;
DEFINE FIELD incremental_count ON page TYPE int DEFAULT 0;

DEFINE TABLE heading SCHEMAFULL;
DEFINE FIELD doc_id       ON heading TYPE string;
DEFINE FIELD block_id     ON heading TYPE string;
DEFINE FIELD level        ON heading TYPE int;
DEFINE FIELD text         ON heading TYPE string;
DEFINE FIELD body         ON heading TYPE string;
DEFINE FIELD body_hash    ON heading TYPE string;
DEFINE FIELD gist         ON heading TYPE option<string>;
DEFINE FIELD indexed_sha  ON heading TYPE string;

DEFINE TABLE mention SCHEMAFULL;
DEFINE FIELD doc_id       ON mention TYPE string;
DEFINE FIELD block_id     ON mention TYPE string;
DEFINE FIELD kind         ON mention TYPE string;
DEFINE FIELD text         ON mention TYPE string;
DEFINE FIELD norm         ON mention TYPE string;
DEFINE FIELD start        ON mention TYPE int;
DEFINE FIELD end          ON mention TYPE int;
DEFINE FIELD pass         ON mention TYPE int;
DEFINE FIELD indexed_sha  ON mention TYPE string;

DEFINE TABLE term SCHEMAFULL;
DEFINE FIELD name         ON term TYPE string;
DEFINE FIELD doc_id       ON term TYPE string;
DEFINE FIELD block_id     ON term TYPE string;

DEFINE TABLE symbol SCHEMAFULL;
DEFINE FIELD doc_id       ON symbol TYPE string;
DEFINE FIELD lang         ON symbol TYPE string;
DEFINE FIELD kind         ON symbol TYPE string;
DEFINE FIELD name         ON symbol TYPE string;

-- outline + catalog
DEFINE TABLE contains TYPE RELATION IN page|heading OUT page|heading SCHEMAFULL;
-- direct citation
DEFINE TABLE links_to TYPE RELATION IN page|heading OUT page|heading SCHEMAFULL;
DEFINE FIELD resolved ON links_to TYPE bool;
DEFINE FIELD href     ON links_to TYPE option<string>;
DEFINE FIELD via      ON links_to TYPE string;
-- synced embed
DEFINE TABLE transcludes TYPE RELATION IN heading OUT page SCHEMAFULL;
-- heading order
DEFINE TABLE same_page TYPE RELATION IN heading OUT heading SCHEMAFULL;
-- heading → hit
DEFINE TABLE mentions TYPE RELATION IN heading OUT mention SCHEMAFULL;
-- mention → glossary term or symbol
-- (email / phone / uuid / hash / endpoint / person / org / team have no out edge)
DEFINE TABLE of TYPE RELATION IN mention OUT term|symbol SCHEMAFULL;

-- logical binds. source distinguishes glossary / model.
DEFINE TABLE defines TYPE RELATION IN heading OUT heading|term|symbol SCHEMAFULL;
DEFINE TABLE depends_on  TYPE RELATION IN heading OUT heading SCHEMAFULL;
DEFINE TABLE constrains  TYPE RELATION IN heading OUT heading SCHEMAFULL;
DEFINE TABLE contradicts TYPE RELATION IN heading OUT heading SCHEMAFULL;
DEFINE TABLE supersedes  TYPE RELATION IN heading OUT heading SCHEMAFULL;

-- Repeat on each relation table (contains, links_to, transcludes, same_page,
-- mentions, of, defines, depends_on, constrains, contradicts, supersedes):
--   source        outline | link | glossary | symbol | extract | model
--   from_doc      docId of the writer
--   indexed_sha
-- Logical tables also carry quote (option) and confidence (option).
```

The block above is the shape. The migration is one `DEFINE FIELD` per table. `source = symbol` on `defines` is the fence definition (there is no separate `defines_symbol` table). `source = glossary` on `defines` points at a `term`. `source = model` points at a heading.

No full-text index in this namespace. `SEARCH` lives on the search projection ([search-scale — schema](./search-scale/README.md#schema)). No HNSW index in this slice.

### Replace rules

| Writer | Deletes | Leaves |
|---|---|---|
| Extract pass for `docId` | That doc’s `mention` rows with `pass != page.pass`. Extractor edges (`contains`, `same_page`, `mentions`, `of`, and `defines` where `source` is `glossary` or `symbol`) whose `from_doc` is this doc and `pass` is stale. | Model edges. Other docs. |
| Direct pass | This doc’s outbound `links_to` / `transcludes` | Inbound edges from other docs (those writers refresh their own outbound) |
| Semantic pass | Outbound `depends_on`, `constrains`, `contradicts`, `supersedes`, and `defines` **where `source = model`**, for dirty headings only | `source = glossary`, `source = link`, symbols |

Implement delete-then-insert in one SurrealDB transaction per page. A crash mid-pass must not leave a page with zero edges and no `index_error`. Prefer: write the new pass, then delete the old pass, then bump `page.pass`.

Folder `contains` edges are rewritten from the catalog at this SHA (cheap, whole tree). They are not per-paragraph.

## Queries

Neighborhood of a heading (AB2 pack, spatial only):

```surql
SELECT
  id,
  ->contains->heading.{ id, text } AS children,
  ->links_to->(page, heading).{ id, git_path, text } AS out,
  <-links_to<-heading.{ id, text, doc_id } AS inbound,
  ->mentions->mention.{ kind, text, norm } AS hits,
  ->depends_on->heading.{ id, text } AS depends,
  ->constrains->heading.{ id, text } AS constrains,
  ->contradicts->heading.{ id, text } AS contradicts
FROM heading:[$doc, $block];
```

Glossary definition for a word hit on this heading:

```surql
SELECT ->mentions->mention->of->term<-defines<-heading.{ id, text, doc_id }
FROM heading:[$doc, $block];
```

Every page that cites a UUID (join on normalized mention text, no model edge):

```surql
SELECT doc_id, block_id, text FROM mention WHERE kind = 'uuid' AND norm = $uuid;
```

Broken links:

```surql
SELECT href, from_doc, indexed_sha FROM links_to WHERE resolved = false;
```

## Capacity

Dogfood is one wiki (this product’s docs) on one SurrealDB process: namespace `graph` and namespace `search`, one memory cap. The schema is heading-grained, incremental on dirty `docId`s. Many wikis are placed one database per workspace ([scale.md](./scale.md)). Do not shard one wiki. Do not add LanceDB until a measured query cannot be answered by `RELATE` plus the search projection.

Expected write size per dirty page: headings (tens), mentions (tens to low hundreds), direct edges (the page’s links). Model edges: a handful per dirty heading, not a clique.

## Do not

- Store Yjs updates or blobs in SurrealDB.
- Store the graph under `.venus/` as the primary copy (optional debug dump is not the store).
- Let the web app open the SurrealDB port.
- Key records by `gitPath`.
- Keep every historical SHA’s full edge set in v1.
- Put `crdt_*` rows and graph edges in one database so a graph migration can take down collab.
