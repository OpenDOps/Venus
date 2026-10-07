# Design

| File / folder | Role |
|---|---|
| [venus-design.md](./venus-design.md) | Product + data design: goal, hub stack, dual store, lease/git/views, agentic contract |
| [datamodel](./datamodel/README.md) | **What is stored where:** [CRDT spaces](./datamodel/crdt.md), [git](./datamodel/git.md) |
| [architecture.md](./architecture.md) | Dataflow (hub + Postgres + host) |
| [components](./components/README.md) | Backend ([hub](./components/backend/hub/)) and frontend ([CRDT tree](./components/frontend/crdt-tree/)). |
| [glossary.md](./glossary.md) | Doc export vs `T0` vs git snapshot vs markdown projection |
| [CRDT](./CRDT/README.md) | Prototype CRDT stack (Yjs, M1 keck, M3.0 hub, Postgres, export). No client Rust: [CRDT/wasm.md](./CRDT/wasm.md). |
| [LiveSnapshot](./LiveSnapshot/README.md) | Pin (copy, do not stall CRDT) + git snapshotter. **Implemented** in [M3](./M3/README.md) (closed 2026-09-14). Gate was [M3.0](./M3.0/README.md) **closed** and [high-availability.md](./LiveSnapshot/high-availability.md) **Acceptance**. Live CRDT HA: [M3.0/high-availability.md](./M3.0/high-availability.md). |
| [Agents](./Agents/README.md) | **Main product feature:** multidimensional spec graph (spatial binds + temporal why). Plan: [agentic-binding.md](./Agents/agentic-binding.md). Contract: [LifeIndexing](./Agents/LifeIndexing.md). Two gits + analyzer (select CodeGraph CLI / Aider / both): [code-bind](./Agents/code-bind.md). |
| [SemanticGraph](./SemanticGraph/README.md) | AB1 **store** (after M4): SurrealDB. In-document extract (tree-sitter, Aho–Corasick, regex, optional ONNX NER), direct `venus:doc:` links, background semantic binds. [extract](./SemanticGraph/extract.md), [connect](./SemanticGraph/connect.md), [store](./SemanticGraph/store.md). Graph vs search namespaces: [scale](./SemanticGraph/scale.md). Search cluster: [search-scale](./SemanticGraph/search-scale/README.md), plan [search-scale/plan.md](./SemanticGraph/search-scale/plan.md). |
| [MDGate](./MDGate/README.md) | Adapter gate. `fromDoc`, [live pane](./MDGate/live-pane.md), [apply](./MDGate/apply.md), [subset](./MDGate/subset.md), [fixtures](./MDGate/fixtures.md). |
| [api-map.md](./api-map.md) | Installed symbols (Actual column) |
| [rpc.md](./rpc.md) | Project-wide JSON/gRPC envelope (`error` / advertisement / data). Internal RPC is gRPC. |
| [scenarios](../scenarios/README.md) | Implemented tests, grouped by feature |
| [venus-implementation-plan.md](./venus-implementation-plan.md) | Tools, bindings, milestone order |
| [lease-freeze-rationale.md](./lease-freeze-rationale.md) | Why freeze on lease |
| [M0](./M0/README.md) | Empty host (done 2026-08-29) |
| [M1](./M1/README.md) | OctoBase loop (done 2026-08-30) |
| [M2](./M2/README.md) | Markdown projection (done 2026-08-30) |
| [M3.0](./M3.0/README.md) | Venus hub replace keck (**done** 2026-09-13). Plan: [M3.0/plan.md](./M3.0/plan.md). Process: [hub](./components/backend/hub/). Wire: [CRDT](./CRDT/README.md). HA: [M3.0/high-availability.md](./M3.0/high-availability.md). |
| [M3](./M3/README.md) | Git snapshotter (**done** 2026-09-14). Plan: [M3/plan.md](./M3/plan.md). Pin: [LiveSnapshot](./LiveSnapshot/README.md). |
| [M4](./M4/README.md) | Folder tree + links + product header (**in progress** — steps 1–8 done; [step-verify](./M4/plan.md#9-step-verify) automated pass 2026-10-06, person pass open). Plan: [M4/plan.md](./M4/plan.md). |
| [M5](./venus-implementation-plan.md#m5--semantic-graph-backend-surrealdb--surrealastic-6-weeks) | Semantic graph backend: SurrealDB + surrealastic (**in progress** — [search-scale](./SemanticGraph/search-scale/plan.md) steps 1–4 done) |
| [M6](./M6/README.md) | Venus docs in the wiki, indexed (**not started**). Dictionary (`daachorse`), tree-sitter, regex, optional ONNX NER, full text, vectors |
| [M7](./venus-implementation-plan.md#m7--lease--freeze-week) | Lease + freeze (**not started**) |
| [M8](./venus-implementation-plan.md#m8--comment-commit-markdown-only-2-weeks) | Comment-commit, markdown apply (**not started**). Apply: [MDGate apply](./MDGate/apply.md) |
| [M9](./venus-implementation-plan.md#m9--threads-alternatives-stacks-2-weeks) | Threads, alternatives, stacks (**not started**) |
| [M10](./venus-implementation-plan.md#m10--revert--agent-loop-week) | Revert + agent loop (**not started**) |

Product (what to ship for orchestration, after the wiki spine): [product-plan](../product/product-plan.md). Operators: [runbook](../runbook.md). Deploy: [devops](../devops/README.md) ([hub fleet](../devops/hub-fleet.md) after M3.0). Tests: [scenarios](../scenarios/README.md).
