# M3.0 — Venus hub (replace keck)

A **standalone merge buffer**: apply Yjs, broadcast to the room, persist to **Postgres**. Same browser wire as M1 (`y-protocols/sync`, subprotocol `AFFiNE`). **No** OctoBase keck, JWST Block REST, markdown, or git.

This folder is the implementation contract for the M3.0 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). The wiki plan runner does not exist yet, so the plan lives here (not under `wiki/plans/`). Live CRDT HA: [high-availability.md](./high-availability.md). Dataflow: [architecture.md](../architecture.md). CRDT stack: [CRDT/README.md](../CRDT/README.md). M1 keck recon (legacy): [LiveSnapshot/octobase.md](../LiveSnapshot/octobase.md).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M3.0.state.yaml](./M3.0.state.yaml) | Board: step status only |
| [high-availability.md](./high-availability.md) | Sticky owner per `workspace_id`, persist pipes, dirty trigger, drain. **Landed here.** Snapshotter fleet stays in [LiveSnapshot/high-availability.md](../LiveSnapshot/high-availability.md). |

Shared: [api-map.md](../api-map.md) (Chosen backend filled in `step-recon-hub`). Words: [glossary.md](../glossary.md).

**Depends on:** [M2](../M2/README.md) closed (2026-08-30). Host already has `OctoBaseKeckProvider` + M1 e2e against keck. `mount-editor` must stay unaware of the server.

**Exit:** Compose is **`postgres` + `hub` + `web`**. Refresh and a second tab still share `doc:home` (including one image). Export is Yjs update v1 from **Venus tables**, not `jwst`. One live owner per wiki (lease). Dirty upsert on persist (no `jobs` yet). keck is **not** a product service.

**Next:** [M3 — Git snapshotter](../venus-implementation-plan.md#m3--git-snapshotter-week), gated on **this milestone closed** and [LiveSnapshot HA](../LiveSnapshot/high-availability.md) **Acceptance** (snapshotter beside the hub). Convert helper: [pin-convert.md](../MDGate/pin-convert.md).
