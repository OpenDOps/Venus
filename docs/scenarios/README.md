# Scenarios

Implemented tests, grouped by the **feature** they prove (the boxes on [architecture](../design/architecture.md)), not by `m0-` / `m1-` filenames.

Commands assume the **repo root**. Machine setup (Docker, `.env`, ports): [runbook](../runbook.md). This folder is **what we assert**.

M1/M2 e2e need Compose **hub**. Kind stays `'octobase'` (wire alias). Internals: [hub](../design/components/hub/).

## Runners

| Command | Runner | Needs | Matches |
|---|---|---|---|
| `pnpm test` | Vitest (Node) | nothing (doc-export Reachable/Decodes skip if hub is down) | `apps/web/src/host/**/*.test.ts` |
| `pnpm test:e2e` | Playwright | Vite `:5173` (started by the config) | `e2e/m0-*.spec.ts`, `e2e/m2-*.spec.ts` |
| `pnpm test:e2e:m1` | Playwright | Compose `postgres` + `hub` already up; Vite `:5174` + `VITE_SYNC_URL` | `e2e/m1-*.spec.ts` |
| `PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080` + `PLAYWRIGHT_M1=1` | Playwright | Compose **web** healthy (`pnpm compose:up`) | same `m1-*.spec.ts` against nginx `:8080` |
| Manual | Person in Chrome/Firefox | see each group | M0 close-out; [M1 close-out](../runbook.md#manual-testing-m1-close-out); [M2 close-out](../runbook.md#manual-testing-m2-close-out) |

```bash
pnpm test
pnpm test:e2e
pnpm sync:up          # then wait for hub :3000
pnpm test:e2e:m1
pnpm compose:up       # postgres + hub + web :8080
```

Do not `docker compose down -v` between M1 specs. One Playwright worker for M1.

## Groups

| Feature | Architecture boxes | File |
|---|---|---|
| [Editor host](./editor-host.md) | Host, Store, outline | M0 editor, seed, outline |
| [Sync seam](./sync-seam.md) | SyncProvider | memory vs octobase alias; no server in `mount-editor` |
| [Hydrate and persist](./hydrate-persist.md) | Store, hub, Postgres | seed-once; refresh drops (memory) vs keeps (sync) |
| [Collaboration](./collaboration.md) | Tab A / Tab B, WS | two tabs, no reload |
| [Blobs](./blobs.md) | Blob HTTP, Postgres | image upload, second tab, reload |
| [Doc export](./doc-export.md) | curl → hub export | Vitest `snapshot.test.ts` (skip Reachable/Decodes if hub is down) |
| [Compose stack](./compose.md) | postgres, hub, web | Vitest `compose.test.ts`; persist DoD `pnpm compose:dod`; HA DoD `pnpm compose:ha`; A→B on `:8080` optional |
| [Markdown projection](./markdown-projection.md) | Host pane, Store `fromDoc` | Vitest `mdgate/*.test.ts`; Playwright `e2e/m2-pane.spec.ts` |

Milestone DoD prose stays in [M0/plan](../design/M0/plan.md), [M1/plan](../design/M1/plan.md), and [M2/plan](../design/M2/plan.md). When a spec and the plan disagree, the **spec file** is what CI runs. Markdown projection: [markdown-projection.md](./markdown-projection.md).

## Docs shape (not a folder per component)

Keep **one** architecture map. Do **not** split `docs/design/` into `postgres/`, `export/` just because export is an HTTP GET.

| Layer | Lives in | Grows when |
|---|---|---|
| Product + data design | [venus-design.md](../design/venus-design.md) | goal, hub, lease, git, agentic |
| Dataflow | [architecture.md](../design/architecture.md) | a **new box** on the diagram |
| Owned service (API + lifecycle) | [components](../design/components/README.md) | Venus runs the process ([hub](../design/components/hub/)) |
| Slice internals | [CRDT](../design/CRDT/README.md), later MDGate / git sidecar | that slice is being built |
| Installed names / URLs | [api-map.md](../design/api-map.md) | recon fills Actual |
| Deploy | [devops](../devops/README.md) | Compose now; Kubernetes later |
| Proof | **this folder** | a test exists |
