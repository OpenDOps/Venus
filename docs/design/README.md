# Design

| File / folder | Role |
|---|---|
| [venus-design.md](./venus-design.md) | Product + data design: goal, hub stack, dual store, lease/git/views, agentic contract |
| [datamodel](./datamodel/README.md) | **What is stored where:** [CRDT spaces](./datamodel/crdt.md), [git](./datamodel/git.md) |
| [architecture.md](./architecture.md) | Dataflow (hub + Postgres + host) |
| [components](./components/README.md) | Venus-owned services. Hub: [hub](./components/hub/) |
| [glossary.md](./glossary.md) | Doc export vs `T0` vs git snapshot vs markdown projection |
| [CRDT](./CRDT/README.md) | Prototype CRDT stack (Yjs, M1 keck, M3.0 hub, Postgres, export). No client Rust: [CRDT/wasm.md](./CRDT/wasm.md). |
| [LiveSnapshot](./LiveSnapshot/README.md) | Pin (copy, do not stall CRDT) + git snapshotter. Implement: [M3](./M3/README.md). **Gated on** [M3.0](./M3.0/README.md) **closed** and [high-availability.md](./LiveSnapshot/high-availability.md) **Acceptance**. Live CRDT HA: [M3.0/high-availability.md](./M3.0/high-availability.md). |
| [Agents](./Agents/README.md) | **Main product feature:** multidimensional spec graph (spatial binds + temporal why). Plan: [agentic-binding.md](./Agents/agentic-binding.md). Contract: [LifeIndexing](./Agents/LifeIndexing.md). Two gits + analyzer (select CodeGraph CLI / Aider / both): [code-bind](./Agents/code-bind.md). |
| [MDGate](./MDGate/README.md) | Adapter gate. `fromDoc`, [live pane](./MDGate/live-pane.md), [apply](./MDGate/apply.md), [subset](./MDGate/subset.md), [fixtures](./MDGate/fixtures.md). |
| [api-map.md](./api-map.md) | Installed symbols (Actual column) |
| [scenarios](../scenarios/README.md) | Implemented tests, grouped by feature |
| [venus-implementation-plan.md](./venus-implementation-plan.md) | Tools, bindings, milestone order |
| [lease-freeze-rationale.md](./lease-freeze-rationale.md) | Why freeze on lease |
| [M0](./M0/README.md) | Empty host (done 2026-08-29) |
| [M1](./M1/README.md) | OctoBase loop (done 2026-08-30) |
| [M2](./M2/README.md) | Markdown projection (done 2026-08-30) |
| [M3.0](./M3.0/README.md) | Venus hub replace keck (**in progress**). Plan: [M3.0/plan.md](./M3.0/plan.md). Process: [hub](./components/hub/). Wire: [CRDT](./CRDT/README.md). HA: [M3.0/high-availability.md](./M3.0/high-availability.md). |
| [M3](./M3/README.md) | Git snapshotter (**not started**; gated on M3.0 closed + [LiveSnapshot HA](./LiveSnapshot/high-availability.md) Acceptance). Plan: [M3/plan.md](./M3/plan.md). Pin: [LiveSnapshot](./LiveSnapshot/README.md). |

Product (what to ship for orchestration, after the wiki spine): [product-plan](../product/product-plan.md). Operators: [runbook](../runbook.md). Deploy: [devops](../devops/README.md) ([hub fleet](../devops/hub-fleet.md) after M3.0). Tests: [scenarios](../scenarios/README.md).
