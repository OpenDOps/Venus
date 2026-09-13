# M3 — Git snapshotter

Pin Yjs bytes **beside** the hub, convert with the M2 exporter, commit **one** autocomment snapshot to `wiki/`. **No** lease, catalog CRDT, folder tree, or comment-commit. Live collab never waits on markdown or git.

**Status: in progress** — board at [step-pin](./plan.md#4-step-pin). `step-recon-snapshot`, `step-worker`, and `step-rust-adapter` are **done**. **Gate:** [M3.0](../M3.0/README.md) **closed** (2026-09-13) and [LiveSnapshot HA](../LiveSnapshot/high-availability.md) **Acceptance** accepted 2026-09-13. Do not `git init wiki/` or start the Flush UI in this step (autoinit is [step-flush](./plan.md#6-step-flush)).

This folder is the implementation contract for the M3 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). M3 is the **thin column** of [HA — M3 must keep this shape](../LiveSnapshot/high-availability.md#m3-must-keep-this-shape): read M3.0 SQL `dirty` (do not invent a second keystroke list), in-process idle/Flush, replica encode or idle GET, pin at **run** not enqueue, one process, one `wiki/`. Pin then convert: [LiveSnapshot](../LiveSnapshot/README.md). Convert helper (no git): [pin-convert.md](../MDGate/pin-convert.md). Git tree: [datamodel git](../datamodel/git.md). Live CRDT stays the hub: [M3.0 HA](../M3.0/high-availability.md).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M3.state.yaml](./M3.state.yaml) | Board: step status only |
| [convert-bench.md](./convert-bench.md) | JS CLI vs Rust `fromDoc` timings ([step-rust-adapter](./plan.md#3-step-rust-adapter)) |

Shared: [api-map.md](../api-map.md) (snapshotter Actuals filled in `step-recon-snapshot`). Words: [glossary.md](../glossary.md). Exporter: [MDGate](../MDGate/README.md).

**Depends on:** [M3.0](../M3.0/README.md) closed. Host already syncs `doc:home` on the hub; `from-doc.js` / `pin-from-doc.js` already exist. `mount-editor` must stay unaware of git.

**Exit:** clone `wiki/` elsewhere and read the page as markdown; casual WYSIWYG did not require a review comment; typing during flush still syncs between tabs.

**Next:** [M4 — Folder tree + links + product header](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks). Parallel (not this exit): [LifeIndexing](../Agents/LifeIndexing.md) (**AB1**) after the SHA.
