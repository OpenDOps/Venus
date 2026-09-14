# M3 — Git snapshotter

Pin Yjs bytes **beside** the hub, convert with the M2 exporter, commit **one** autocomment snapshot to `wiki/`. **No** lease, catalog CRDT, folder tree, or comment-commit. Live collab never waits on markdown or git.

**Status: closed** (2026-09-14) — board steps 1–9 `done`. Host Flush + git-log chrome are on; `mount-editor` stays unaware. Clone `wiki/` elsewhere and read markdown. **Gate was:** [M3.0](../M3.0/README.md) **closed** (2026-09-13) and [LiveSnapshot HA](../LiveSnapshot/high-availability.md) **Acceptance** accepted 2026-09-13.

This folder is the implementation contract for the M3 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). M3 **implements** the [HA snapshotter](../LiveSnapshot/high-availability.md) at one-wiki load: SQL trigger → `dirty` / `dirty_wiki` → observer `jobs` → `SKIP LOCKED` consumers → MVCC cut → pin Map → convert → git. Do not ship idle GET / in-process idle as the product queue. Pin then convert: [LiveSnapshot](../LiveSnapshot/README.md). Convert helper (no git): [pin-convert.md](../MDGate/pin-convert.md). Git tree: [datamodel git](../datamodel/git.md). Live CRDT stays the hub: [M3.0 HA](../M3.0/high-availability.md).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M3.state.yaml](./M3.state.yaml) | Board: step status only |
| [convert-bench.md](./convert-bench.md) | JS CLI vs Rust `fromDoc` timings ([step-rust-adapter](./plan.md#3-step-rust-adapter)) |

Shared: [api-map.md](../api-map.md) (snapshotter Actuals filled in `step-recon-snapshot`). Words: [glossary.md](../glossary.md). Exporter: [MDGate](../MDGate/README.md). Close-out: [runbook M3](../../runbook.md#manual-testing-m3-close-out).

**Depends on:** [M3.0](../M3.0/README.md) closed. Host already syncs `doc:home` on the hub; `from-doc.js` / `pin-from-doc.js` already exist. `mount-editor` must stay unaware of git.

**Exit:** clone `wiki/` elsewhere and read the page as markdown; casual WYSIWYG did not require a review comment; typing during flush still syncs between tabs.

**Next:** [M4 — Folder tree + links + product header](../M4/README.md) ([plan](../M4/plan.md)). Parallel (not this exit): [LifeIndexing](../Agents/LifeIndexing.md) (**AB1**) after the SHA.
