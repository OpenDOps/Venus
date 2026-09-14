# Components

Venus-owned pieces, split by where they run. Dataflow of how they sit together stays in [architecture.md](../architecture.md). Installed URLs stay in [api-map.md](../api-map.md). Live-page **wire** (Yjs, seam, export) stays in [CRDT](../CRDT/README.md). How containers run: [devops/compose](../../devops/compose.md).

keck (M1) was an **external** AGPL image — its routes live in api-map history, not here.

## Backend

Processes Venus runs. Compose services. No React.

| Component | Compose | Role |
|---|---|---|
| [Hub](./backend/hub/) | `hub` | Live CRDT: apply, broadcast, persist. Rust + y-octo. [Architecture](./backend/hub/architecture.md), [files](./backend/hub/files.md). |

Sidecar (git snapshotter) is Compose `sidecar`; process notes live with [M3](../M3/README.md) / [LiveSnapshot](../LiveSnapshot/README.md) until it gets its own folder here.

## Frontend

Host chrome in `apps/web`. Talks to the hub only through `SyncProvider` (Yjs update v1). `mount-editor` stays unaware.

| Component | Milestone | Role |
|---|---|---|
| [CRDT tree](./frontend/crdt-tree/) | [M4](../M4/README.md) | Collaborative wiki folder tree. Catalog Y.Doc on the hub + React tree with drag-and-drop. |
