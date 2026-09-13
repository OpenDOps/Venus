# Components

Venus-owned services (process, API, lifecycle). Dataflow of how they sit together stays in [architecture.md](../architecture.md). Installed URLs stay in [api-map.md](../api-map.md). Live-page **wire** (Yjs, seam, export) stays in [CRDT](../CRDT/README.md). How containers run: [devops/compose](../../devops/compose.md).

keck (M1) was an **external** AGPL image — its routes live in api-map history, not here.

| Component | Compose | Role |
|---|---|---|
| [Hub](./hub/) | `hub` | Live CRDT: apply, broadcast, persist. Rust + y-octo. [Architecture](./hub/architecture.md), [files](./hub/files.md). |
