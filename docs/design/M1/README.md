# M1 — OctoBase loop

Sync the M0 page over **OctoBase keck**. Persist is **Postgres in Docker**; keck is a **second Docker**. One workspace, one doc, WebSocket (`AFFiNE` subprotocol). Two tabs. One image blob. keck export snapshot. **No git, lease, catalog, or markdown pane.**

This folder is the implementation contract for the M1 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). The wiki plan runner does not exist yet, so the plan lives here (not under `wiki/plans/`).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M1.state.yaml](./M1.state.yaml) | Board: step status only |

Shared: [api-map.md](../api-map.md) (sync / blob / snapshot Actuals filled in `step-recon-sync`; kind **octobase**).

**Depends on:** [M0](../M0/README.md) closed (2026-08-29). Host already has `SyncProvider` + `MemoryNoopProvider`; `mount-editor` must stay unaware of the server.

**Exit:** refresh and a second client see the same page (including one uploaded image). **Postgres in Docker** (via keck in a **second** container) is the refresh source; IndexedDB is optional and not required to close M1. SQLite is not the product store.

**Next:** [M2 — Markdown projection](../venus-implementation-plan.md#m2--markdown-projection-week).
