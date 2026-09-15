# Doc export

**Feature:** a non-browser client reads the **current** Y.Doc from the hub over **gRPC**. GET `/export` is advertisement JSON, not Yjs. Not markdown, not `T0`. [glossary](../design/glossary.md). [rpc.md](../design/rpc.md). [CRDT doc export](../design/CRDT/README.md#doc-export-internal-grpc).

**Boxes:** gRPC `Hub.ExportDoc` → hub encode ([architecture](../design/architecture.md#elements)). Human debug: GET advertisement.

## Run

```bash
pnpm test
pnpm sync:up          # then hydrate once (app or pnpm test:e2e:m1)
pnpm test             # advertisement GET runs when hub is on :3000
```

Exact GET: api-map **Export advertisement**. Product bytes: **Export RPC** (`Hub.ExportDoc`). [runbook](../runbook.md#sync-m1).

## Vitest

`snapshot.test.ts` checks the api-map GET string and, when `:3000` is up, that the body has **no** root `error` and lists home + catalog. Decode of Yjs is `ExportDoc` (step-spaces). There is no editor export button.

| Spec | Proves |
|---|---|
| `snapshot.test.ts` — advertisement GET string | `docs/design/api-map.md` still contains the exact `curl …/export` (no `-o` Yjs file) and `venus.hub.v1.Hub/ExportDoc` |
| `snapshot.test.ts` — GET is not live_export | `http.rs` returns advertisement; does not call `live_export` |
| `snapshot.test.ts` — not in the editor UI | `mount-editor.js` / `editor-container.js` / `boot.js` / `App.tsx` do not call `/api/block/…/export` |
| `snapshot.test.ts` — Reachable | GET `/export` **200** JSON; `advertisement.kind === doc_export`; `http_export === false` |
| `snapshot.test.ts` — `?doc=` | **400** `error.code === export_http_disabled` with `advertisement` |

Reachable **skips** when nothing listens on `127.0.0.1:3000` so `pnpm test` stays Docker-free. If the hub is up and GET is empty, not JSON, or has `error` on the bare path, they **fail**.
