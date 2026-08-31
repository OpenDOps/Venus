# Design

| File / folder | Role |
|---|---|
| [venus-design.md](./venus-design.md) | Product rules (lease, freeze, two git classes, views) |
| [datamodel](./datamodel/README.md) | **What is stored where:** [CRDT spaces](./datamodel/crdt.md), [git](./datamodel/git.md) |
| [architecture.md](./architecture.md) | Dataflow (M0–M2 on keck; M3.0+ on hub) |
| [glossary.md](./glossary.md) | Doc export vs `T0` vs git snapshot vs markdown projection |
| [CRDT](./CRDT/README.md) | Prototype CRDT stack (Yjs, M1 keck, M3.0 hub, Postgres, export) |
| [LiveSnapshot](./LiveSnapshot/README.md) | Pin (copy, do not stall CRDT) + git snapshotter. **M3 gated on** [M3.0](./M3.0/README.md) **closed** and [high-availability.md](./LiveSnapshot/high-availability.md) **Acceptance**. Live CRDT HA: [M3.0/high-availability.md](./M3.0/high-availability.md). |
| [Agents](./Agents/README.md) | **Main product feature:** multidimensional spec graph (spatial binds + temporal why). Plan: [agentic-binding.md](./Agents/agentic-binding.md). Contract: [LifeIndexing](./Agents/LifeIndexing.md). Two gits + analyzer (select CodeGraph CLI / Aider / both): [code-bind](./Agents/code-bind.md). |
| [MDGate](./MDGate/README.md) | Adapter gate. `fromDoc`, [live pane](./MDGate/live-pane.md), [apply](./MDGate/apply.md), [subset](./MDGate/subset.md), [fixtures](./MDGate/fixtures.md). |
| [api-map.md](./api-map.md) | Installed symbols (Actual column) |
| [scenarios](../scenarios/README.md) | Implemented tests, grouped by feature |
| [venus-implementation-plan.md](./venus-implementation-plan.md) | Tools, bindings, milestone order |
| [lease-freeze-rationale.md](./lease-freeze-rationale.md) | Why freeze on lease |
| [M0](./M0/README.md) | Empty host (done 2026-08-29) |
| [M1](./M1/README.md) | OctoBase loop (done 2026-08-30) |
| [M2](./M2/README.md) | Markdown projection (done 2026-08-30) |
| [M3.0](./M3.0/README.md) | Venus hub replace keck (next). Plan: [M3.0/plan.md](./M3.0/plan.md). HA: [M3.0/high-availability.md](./M3.0/high-availability.md). |

Product (what to ship for orchestration, after the wiki spine): [product-plan](../product/product-plan.md). Operators: [runbook](../runbook.md). Deploy: [devops](../devops/README.md). Tests: [scenarios](../scenarios/README.md).
