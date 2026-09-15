# RPC envelope

Project-wide contract for **JSON HTTP GET** and **gRPC**. Clients decide the outcome by inspecting a root **`error`** field (HTTP JSON body, or `venus.rpc.v1.Error` unpacked from gRPC `Status.details`). Do not infer success from “some payload is present.”

**gRPC is the product RPC.** Every **internal** hop (sidecar → hub, hub → other Venus processes, tests that are not a browser) uses gRPC. The browser keeps **AFFiNE WebSocket** for collab and **HTTP** for blobs and `page_identity` (the tab cannot speak our gRPC). Do not add grpc-web for M4.

IDL: [`proto/venus/rpc/v1/error.proto`](../../proto/venus/rpc/v1/error.proto), [`proto/venus/hub/v1/hub.proto`](../../proto/venus/hub/v1/hub.proto). Actuals: [api-map](./api-map.md). Hub process: [hub](./components/backend/hub/).

## Three shapes

Parse the JSON object. Then:

1. **If `error` is present and not `null` → failure.** Stop. Do not read `data`. `advertisement` on the same object is a hint only.
2. **Else if `advertisement` is present and you were not fetching a resource → discovery.** How to call the real RPC, which docs exist, which `doc` / `?doc=` form is valid.
3. **Else → success.** HTTP JSON wraps the resource in **`data`**. gRPC success is the typed response message (no `error` field). Binary fields stay bytes (`yjs_update_v1`), not a JSON string.

Never send `"error": null` on success. Omit the key.

| Shape | HTTP | gRPC |
|---|---|---|
| Error | 4xx/5xx + `{ "error": { "code", "message", … } }` | Non-OK `Status`. Pack `venus.rpc.v1.Error` in `details`. Do **not** return the success message with an `error` field. |
| Advertisement | **200** `{ "advertisement": { … } }` | `ListDocs` (typed docs + the same bind facts). Not a substitute for `ExportDoc`. |
| Data | **200** `{ "data": { … } }` | RPC response fields (`ExportDocResponse.yjs_update_v1`, …). |

`code` is a stable snake_case token. `message` is for humans and tests. Extra keys on `error` (e.g. `workspace_id`) are allowed; clients still key off `error` existing.

## Transport split

| Who | Collab | Doc export | Blobs | Page identity |
|---|---|---|---|---|
| Browser tab | WS `/collaboration/:workspace_id` (`?doc=` SQL uuid) | Does not export | HTTP `/api/blobs/…` | HTTP `POST`/`DELETE` `/api/pages/…` |
| Sidecar / curl-from-a-service / later Venus | — | **gRPC `Hub.ExportDoc` / `ListDocs`** | gRPC when a Blob RPC exists; HTTP is the browser path | Flush **SELECT**s SQL. gRPC later if sidecar must stop reading the table |
| Human debug | POST/GET health JSON | GET `/api/block/:workspace/export` = **advertisement only** | HTTP | HTTP `/api/pages/…` |

Doc export is **internal**. It is not a public REST resource. **Do not enable GET as the Yjs export.** There is no `GET /api/block/:workspace/:doc/export` and no `application/octet-stream` on `/export`.

## GET `/api/block/:workspace/export`

| Request | Status | Body |
|---|---|---|
| Bare GET (valid workspace UUID) | **200** | `{ "advertisement": { … } }` — no `error`. Lists home + catalog (and how to pass `doc`). Points at gRPC. |
| `?doc=<sql uuid>` (even home / catalog) | **400** | `{ "error": { "code": "export_http_disabled", "message": "…" }, "advertisement": { … } }` |
| `?doc=` empty, guid (`doc:home`, `venus:catalog`), or non-uuid | **400** | `{ "error": { "code": "invalid_doc", … }, "advertisement": { … } }` |
| Invalid `:workspace` | **400** | `{ "error": { "code": "invalid_workspace", … } }` |

`?doc=` on **collaboration WS** is unchanged (omit = home, guid = 400). That query is **not** an HTTP export switch.

Advertisement `docs` always includes:

| role | guid | sql_id |
|---|---|---|
| `home` | `doc:home` | `395cd07b-bdb1-5f54-ada8-e9a3fabb6a20` |
| `catalog` | `venus:catalog` | `4fe5c16e-4be3-5700-a456-ecc8e86cdf1a` |

`ListDocs` (gRPC) may add other SQL `doc_id`s that already have rows. The hub does not parse the catalog Y.Doc to name them.

## gRPC `venus.hub.v1.Hub`

Listen: `HUB_GRPC_LISTEN`, default **`0.0.0.0:3100`**. Compose host bind `127.0.0.1:3100` (hub-b: `127.0.0.1:3101` → container `3100`). HTTP/WS stays `:3000`.

| RPC | Request | Success |
|---|---|---|
| `ListDocs` | `workspace_id` | docs + bind facts |
| `ExportDoc` | `workspace_id`, optional `doc_id` | `yjs_update_v1` + the SQL `doc_id` used |

`ExportDoc`: omit `doc_id` = home (`PAGE_DOC_ID`). Value must be a hyphenated SQL uuid (same rule as WS `?doc=`). Guid → `INVALID_ARGUMENT` / `invalid_doc`. Same encode as today’s `Hub::live_export`, but **per `doc_id`** (step-spaces). Current tree, not `T0`, not git.

grpcurl (after the server exists):

```bash
grpcurl -plaintext -d '{"workspace_id":"77e4a2b1-8b40-5979-a73c-fd4477216d00"}' \
  127.0.0.1:3100 venus.hub.v1.Hub/ExportDoc
```

With a second page: set `doc_id` to that SQL uuid.

## Error codes

| `error.code` | HTTP | gRPC status | When |
|---|---|---|---|
| `invalid_workspace` | 400 | `INVALID_ARGUMENT` | `:workspace` / `workspace_id` is not a hyphenated UUID |
| `invalid_doc` | 400 | `INVALID_ARGUMENT` | `doc` / `doc_id` empty, guid, or not a UUID |
| `export_http_disabled` | 400 | — (HTTP only) | GET `/export?doc=` — Yjs is not on GET |
| `lease_held` | 503 | `UNAVAILABLE` | another hub owns `workspace_id` |
| `node_not_empty` | 409 | `FAILED_PRECONDITION` | `deleteNode` when any catalog node has `parentId === id` |
| `home_protected` | 409 | `FAILED_PRECONDITION` | `deleteNode` of home (`doc:home`) |
| `store_failed` | 500 | `INTERNAL` | Postgres / hydrate / encode |

Collab upgrade JSON uses the same `error` object (`invalid_workspace`, `invalid_doc`, `lease_held`, `store_failed`). Blob **bytes** GET/HEAD stay binary (304/404); that is not this envelope.

## What this replaces

M1 / M3.0 leftover: `GET /api/block/:workspace/export` returned `application/octet-stream` (home Yjs). That silent home-only GET is **not** the multi-doc story. Pin collect stays SQL MVCC ([M3](./M3/plan.md)); it never was this GET.

`Hub::live_export` remains the RAM/SQL encode used by **`ExportDoc`**. HTTP does not call it. Rebuild Compose `hub` after this change (`docker compose up --build postgres hub`) so GET is JSON, not leftover Yjs bytes.

## Sidecar / M4

Sidecar pin does not call export (grep still fails closed on `/export` as collect). When something needs **live** Yjs from the owner (debug, later `T0` replica encode), it uses **gRPC**, not GET. Browser chrome never calls export.

Page identity for the **web host** is HTTP `POST`/`DELETE` `/api/pages/…` ([hub page-identity](./components/backend/hub/page-identity.md)). Sidecar Flush reads SQL. Do not add a Hub proto RPC for this table until the sidecar needs it.
