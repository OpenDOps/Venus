# M3.0 — Venus hub (replace keck)

A **standalone merge buffer**: apply Yjs, broadcast to the room, persist to **Postgres**. Same browser wire as M1 (`y-protocols/sync`, subprotocol `AFFiNE`). **No** OctoBase keck, JWST Block REST, markdown, or git.

**Status: in progress** — board at [step-compose-hub](./plan.md#4-step-compose-hub). Product apply is **Rust + y-octo** (MIT). Store is **Postgres** `crdt_*` / `blob`. AFFiNE WS apply / broadcast / persist ~1s is in the crate. Later steps (Compose, provider, parity, lease, dirty) are **not** claimed. How the crate is meant to run: [hub](../components/hub/). Internals: [architecture](../components/hub/architecture.md), [files](../components/hub/files.md).

This folder is the implementation contract for the M3.0 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). Live CRDT HA: [high-availability.md](./high-availability.md). Dataflow: [architecture.md](../architecture.md). CRDT stack: [CRDT/README.md](../CRDT/README.md). M1 keck recon (legacy): [LiveSnapshot/octobase.md](../LiveSnapshot/octobase.md).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M3.0.state.yaml](./M3.0.state.yaml) | Board: step status only |
| [high-availability.md](./high-availability.md) | Wiki sticky per `workspace_id`, Rust + y-octo merge, persist pipes, dirty trigger, drain. Snapshotter / `toDoc` worker stays in [LiveSnapshot/high-availability.md](../LiveSnapshot/high-availability.md). Hub HPA / gateway: [hub-fleet.md](../../devops/hub-fleet.md) (later). |
| [steps-1-3-review.md](./steps-1-3-review.md) | Review of recon / store / WS crate: logic, performance, fix proposals. Auth deferred to a separate module. Does not reopen those steps. |

Shared: [api-map.md](../api-map.md) (Chosen backend = Venus hub). Words: [glossary.md](../glossary.md).

**Depends on:** [M2](../M2/README.md) closed (2026-08-30). Host keeps `OctoBaseKeckProvider` as a wire alias (`kind: 'octobase'`). `mount-editor` stays unaware of the server.

**Exit (held):** Compose is **`postgres` + `hub` + `web`**. Hub is **Rust + y-octo**. Same `AFFiNE` wire. Export is Yjs update v1 from **Venus tables**. One live owner per wiki (`workspace_lease`). Dirty upsert on persist (no `jobs`). keck is **not** a product service.

**Next:** [M3 — Git snapshotter](../venus-implementation-plan.md#m3--git-snapshotter-week), gated on [LiveSnapshot HA](../LiveSnapshot/high-availability.md) **Acceptance** (snapshotter beside the hub). Convert helper: [pin-convert.md](../MDGate/pin-convert.md).
