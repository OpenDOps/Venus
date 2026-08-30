# Scenarios

Implemented tests, grouped by the **feature** they prove (the boxes on [architecture](../design/architecture.md)), not by `m0-` / `m1-` filenames.

Commands assume the **repo root**. Machine setup (Docker, `.env`, ports): [runbook](../runbook.md). This folder is **what we assert**.

## Runners

| Command | Runner | Needs | Matches |
|---|---|---|---|
| `pnpm test` | Vitest (Node) | nothing (doc-export Reachable/Decodes skip if keck is down) | `apps/web/src/host/**/*.test.ts` |
| `pnpm test:e2e` | Playwright | Vite `:5173` (started by the config) | `e2e/m0-*.spec.ts` |
| `pnpm test:e2e:m1` | Playwright | Compose `postgres` + `octobase` already up; Vite `:5174` + `VITE_SYNC_URL` | `e2e/m1-*.spec.ts` |
| `PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080` + `PLAYWRIGHT_M1=1` | Playwright | Compose **web** healthy (`pnpm compose:up`) | same `m1-*.spec.ts` against nginx `:8080` |
| Manual | Person in Chrome/Firefox | see each group | M0 close-out; [M1 close-out](../runbook.md#manual-testing-m1-close-out) |

```bash
pnpm test
pnpm test:e2e
pnpm sync:up          # then wait for keck :3000
pnpm test:e2e:m1
pnpm compose:up       # postgres + octobase + web :8080
```

Do not `docker compose down -v` between M1 specs. One Playwright worker for M1.

## Groups

| Feature | Architecture boxes | File |
|---|---|---|
| [Editor host](./editor-host.md) | Host, Store, outline | M0 editor, seed, outline |
| [Sync seam](./sync-seam.md) | SyncProvider | memory vs octobase; no server in `mount-editor` |
| [Hydrate and persist](./hydrate-persist.md) | Store, keck, Postgres | seed-once; refresh drops (memory) vs keeps (sync) |
| [Collaboration](./collaboration.md) | Tab A / Tab B, WS | two tabs, no reload |
| [Blobs](./blobs.md) | Blob HTTP, Postgres | image upload, second tab, reload |
| [Doc export](./doc-export.md) | curl → keck export | Vitest `snapshot.test.ts` (skip Reachable/Decodes if keck is down) |
| [Compose stack](./compose.md) | postgres, octobase, web | Vitest `compose.test.ts`; A→B on `:8080` optional |

Milestone DoD prose stays in [M0/plan](../design/M0/plan.md) and [M1/plan](../design/M1/plan.md). When a spec and the plan disagree, the **spec file** is what CI runs.

## Docs shape (not a folder per component)

Keep **one** architecture map. Do **not** split `docs/design/` into `keck/`, `postgres/`, `export/` just because step 7 is an HTTP GET.

| Layer | Lives in | Grows when |
|---|---|---|
| Product rules | [venus-design.md](../design/venus-design.md) | lease, git classes |
| Dataflow | [architecture.md](../design/architecture.md) | a **new box** on the diagram |
| Slice internals | [CRDT](../design/CRDT/README.md), later MDGate / git sidecar | that slice is being built |
| Installed names / URLs | [api-map.md](../design/api-map.md) | recon fills Actual |
| Deploy | [devops](../devops/README.md) | Compose now; Kubernetes later |
| Proof | **this folder** | a test exists |

Add a **component** page under `docs/design/` only when Venus **owns** a service with its own API and lifecycle (lease, git sidecar, catalog). keck is an external AGPL container; its routes belong in api-map + CRDT, not a second OpenAPI tree.
