# Architecture

How Venus’s pieces connect. **Product + data design** (goal, stack, lease, git, agentic contract): [venus-design.md](./venus-design.md). **Stores:** [datamodel](./datamodel/README.md). This file is the **dataflow**: who talks to whom. Markdown hangs off the synced Store ([projection](#markdown-projection-add-here-before-coding-m2)).

Words: [glossary.md](./glossary.md). Prototype CRDT stack: [CRDT/README.md](./CRDT/README.md). Installed names: [api-map.md](./api-map.md). How we test each box: [scenarios](../scenarios/README.md).

## Layers (now vs later)

| Layer | Status | Design |
|---|---|---|
| Editor + outline | **M0 done** | [M0](./M0/README.md) |
| Sync, persist, blobs, **doc export** (keck) | **M1 done** (legacy wire proof) | [CRDT](./CRDT/README.md), [M1](./M1/README.md) |
| Markdown projection | **M2 done** | [MDGate](./MDGate/README.md), [M2/plan.md](./M2/plan.md) |
| **Venus hub** (replace keck) | **M3.0 done** (2026-09-13) | [M3.0](./M3.0/README.md), [hub](./components/backend/hub/), [hub HA](./M3.0/high-availability.md) |
| Pin + git snapshotter | **M3 done** (2026-09-14) | [M3](./M3/README.md) ([plan](./M3/plan.md)), [LiveSnapshot](./LiveSnapshot/README.md) |
| Catalog / header | **M4 not started** | [M4](./M4/README.md) ([plan](./M4/plan.md)). Tree: [CRDT tree](./components/frontend/crdt-tree/). Page map: [page-identity](./datamodel/page-identity.md). **Gated on M3 closed.** |
| Lease `T0` + freeze | **M5 not started** | [implementation plan — M5](./venus-implementation-plan.md#m5--lease--freeze-week), [lease-freeze-rationale.md](./lease-freeze-rationale.md) |
| Comment-commit apply | **M6 not started** | [implementation plan — M6](./venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks), [MDGate apply](./MDGate/apply.md) |
| Threads, alternatives, stacks | **M7 not started** | [implementation plan — M7](./venus-implementation-plan.md#m7--threads-alternatives-stacks-2-weeks) |
| Revert + agent loop | **M8 not started** | [implementation plan — M8](./venus-implementation-plan.md#m8--revert--agent-loop-week) |
| LifeIndexing | Parallel (after M3) | [Agents](./Agents/README.md) — [AB1](./Agents/agentic-binding.md#ab1--lifeindexing) / [LifeIndexing](./Agents/LifeIndexing.md) |
| Bound chat | Parallel (after AB1; **ask-only**) | [AB2](./Agents/agentic-binding.md#ab2--bound-chat) |
| Chat-edit markdown | Parallel (**after M5–M6 checkout**, not after AB2) | [AB3](./Agents/agentic-binding.md#ab3--chat-edit-markdown) |
| History / why pack | Parallel (**after M6**; needs AB1 index; not after AB1) | [AB4](./Agents/agentic-binding.md#ab4--history--why-pack) |
| Stores (CRDT + git) | Design | [datamodel](./datamodel/README.md) |

## Dataflow (M3.0)

M1 proved the wire on **keck**. Product collab is the **Venus hub** (`hub:3000`): [hub](./components/backend/hub/). Postgres tables are `crdt_*` (not `jwst` docs). The browser wire did not change (`AFFiNE` + y-protocols).

```mermaid
flowchart TB
  subgraph browsers["browsers"]
    tabA["Tab A<br/>BlockSuite Store<br/>store.spaceDoc Y.Doc"]
    tabB["Tab B<br/>BlockSuite Store<br/>store.spaceDoc Y.Doc"]
  end

  ws["SyncProvider kind octobase<br/>wire alias for the hub<br/>Yjs update v1 · y-protocols/sync<br/>WebSocket subprotocol AFFiNE"]
  hub["Venus hub<br/>Compose hub :3000<br/>Rust + y-octo<br/>/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00"]
  pg[("Postgres<br/>Compose postgres<br/>crdt_snapshot / crdt_update<br/>blob / workspace_lease / dirty<br/>volume pg-venus-data")]

  tabA --> ws
  tabB --> ws
  ws --> hub
  hub -->|"POSTGRES_HOST/USER/PASSWORD"| pg

  exportClient["sidecar / later Venus<br/>internal · gRPC"]
  exportClient -->|"Hub.ExportDoc / ListDocs<br/>Yjs update v1 · not T0, not git<br/>GET /export is advertisement JSON"| hub

  tabA -->|"POST/GET /api/blobs/77e4a2b1-8b40-5979-a73c-fd4477216d00"| hub
  tabB -->|"POST/GET /api/blobs/77e4a2b1-8b40-5979-a73c-fd4477216d00"| hub

  subgraph m2["M2 — RAM projection"]
    pane["read-only markdown pane"]
  end

  tabA -->|"MarkdownAdapter.fromDoc<br/>same CRDT · not a second replica<br/>not GET export"| pane
```

Stock **`y-websocket` is not on this diagram.** The wire is hub + `AFFiNE`. The unused `y-websocket` kind in code is not a Hocuspocus target. Persist hosted is Postgres. See [CRDT](./CRDT/README.md#seam). How the hub process works: [hub](./components/backend/hub/).

Compose **`web`** (nginx on `:8080`) is a same-origin reverse proxy in front of **`hub:3000`**. It is not a fourth store. Vite `pnpm dev` still opens WS to `:3000` and proxies `/api`. How the containers run: [devops/compose](../devops/compose.md).

## Elements

Each row is a box that later markdown (or git) can hang off. Do not put review hunks on the published tree ([datamodel](./datamodel/README.md)).

| Element | What it is | Markdown later | How we test |
|---|---|---|---|
| **Host** | Vite + React. Mounts BlockSuite web components. | Layout for a read-only md pane (M2). No caret. | [Editor host](../scenarios/editor-host.md) |
| **BlockSuite Store** | Live published page. `affine:*` tree. | Adapter input. Sidecar ids on export. | [Editor host](../scenarios/editor-host.md), [hydrate](../scenarios/hydrate-persist.md) |
| **Y.Doc** | `store.spaceDoc`. Client CRDT. | Never a second Y.Text of the `.md`. | [Hydrate](../scenarios/hydrate-persist.md), [collaboration](../scenarios/collaboration.md) |
| **SyncProvider** | Seam in the host. | Unchanged. Md pane reads the **synced** store. | [Sync seam](../scenarios/sync-seam.md) |
| **Hub** | Venus **Rust + y-octo**: apply, broadcast, persist ~1s. Compose `hub`. Wiki sticky on `workspace_id`. [hub](./components/backend/hub/). | Lease/git sidecar calls **ExportDoc** (gRPC), then adapter in the convert worker. Pin collect is SQL, not export. | [Collaboration](../scenarios/collaboration.md), [blobs](../scenarios/blobs.md), [doc export](../scenarios/doc-export.md) |
| **Postgres** | Source of truth for refresh. `crdt_*` + `blob` + `workspace_lease` + `dirty`. Volume `pg-venus-data`. | Not a markdown store. | [Hydrate](../scenarios/hydrate-persist.md), [blobs](../scenarios/blobs.md) |
| **Compose web** | nginx + static host. Proxies `/api` and `/collaboration` to the collab front. | Layout only. | [Compose stack](../scenarios/compose.md) |
| **Doc export** | gRPC `Hub.ExportDoc`. GET `/api/block/77e4a2b1-8b40-5979-a73c-fd4477216d00/export` is advertisement JSON (available docs + `?doc=` / gRPC). Current tree, no tab. | Same RPC becomes `T0` bytes (M5) and optional `.venus/snapshots/*.bin`. Envelope: [rpc.md](./rpc.md). | [Doc export](../scenarios/doc-export.md) |
| **Markdown projection** | Adapter output + RAM id sidecar. | **M2 done.** [plan](./M2/plan.md). | [Markdown projection](../scenarios/markdown-projection.md) |
| **Git** | Folders + `.md` + commits. | Snapshot vs comment-commit. Layout: [datamodel git](./datamodel/git.md). Pin then convert: [LiveSnapshot](./LiveSnapshot/README.md). M3+. | None yet. |
| **Review session** | Lease, threads, hunks. Per commit Before/After **hub CRDTs**. | Comment-commits vs `T0`. After editable; edits are the next commit. M5–M6. | None yet. |

## Markdown projection (add here before coding M2)

M2 hangs **one** new arrow off the **synced Store**, not off keck export:

```text
WYSIWYG (live CRDT)  ← aligned →  read-only markdown pane
                         │
                         MarkdownAdapter.fromDoc
                         + block-id sidecar
```

- Markdown is a **projection**, not a replica ([venus-design.md](./venus-design.md)).
- One exporter later serves the pane, git flush, and lease `T0` md ([implementation plan — adapter gate](./venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it)).
- Non-browser jobs still use **doc export** for the binary, then the same adapter. Git flush **pins** those bytes (or a sidecar replica) **before** `fromDoc`; live CRDT is not paused ([LiveSnapshot](./LiveSnapshot/README.md)).
- Spectator loop (single-flight; in-place splice or full `fromDoc`): [live-pane.md](./MDGate/live-pane.md). Git / lease convert: [pin-convert.md](./MDGate/pin-convert.md) (pin Yjs bytes, then `fromDoc` on an offline clone).

**M2 closed** when [fixtures.md](./MDGate/fixtures.md) **export** rows are green ([M2/plan.md](./M2/plan.md), 2026-08-30). `fromDoc` + pane loop: [MDGate](./MDGate/README.md). Apply (markdown vs `T0` → hunks): [apply.md](./MDGate/apply.md); **apply** fixture rows before M6.

## Cross-references

| Need | Where |
|---|---|
| Stores (CRDT + git) | [datamodel](./datamodel/README.md) |
| Goal, stack, lease, git, agentic contract | [venus-design.md](./venus-design.md) |
| Why freeze | [lease-freeze-rationale.md](./lease-freeze-rationale.md) |
| Yjs wire, hub (M1: keck), Postgres, export | [CRDT/README.md](./CRDT/README.md) |
| Pins and curl | [api-map.md](./api-map.md) |
| JSON / gRPC envelope | [rpc.md](./rpc.md) |
| M1 steps | [M1/plan.md](./M1/plan.md) |
| M2 steps | [M2/plan.md](./M2/plan.md) |
| M3.0 hub | [hub](./components/backend/hub/); [M3.0/plan.md](./M3.0/plan.md); live CRDT HA: [M3.0/high-availability.md](./M3.0/high-availability.md) |
| Compose / Kubernetes | [devops](../devops/README.md). Hub fleet (after M3.0): [hub-fleet.md](../devops/hub-fleet.md) |
| Adapter gate (M2) | [MDGate](./MDGate/README.md), [subset](./MDGate/subset.md), [fixtures](./MDGate/fixtures.md), [live pane](./MDGate/live-pane.md), [pin convert](./MDGate/pin-convert.md), [M2/plan.md](./M2/plan.md) |
| Apply (M6) | [MDGate apply](./MDGate/apply.md) |
| Pin + git snapshotter (M3) | [M3/plan.md](./M3/plan.md); [LiveSnapshot](./LiveSnapshot/README.md); **done** 2026-09-14. **Gated on M3.0 done** + [LiveSnapshot HA](./LiveSnapshot/high-availability.md) **Acceptance** |
| M4 catalog / tree / header | [M4/plan.md](./M4/plan.md); [CRDT tree](./components/frontend/crdt-tree/). **Gated on M3 closed.** |
| Milestone order (M0–M8) | [venus-implementation-plan.md](./venus-implementation-plan.md#milestone-plan) |
| LifeIndexing / bound chat | [agentic-binding](./Agents/agentic-binding.md) (AB1–AB4). Contract: [LifeIndexing](./Agents/LifeIndexing.md) (spatial + temporal). |
| Implemented tests | [scenarios](../scenarios/README.md) |
| License split | [licensing.md](../legal/licensing.md) |
