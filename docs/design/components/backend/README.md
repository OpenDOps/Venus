# Backend components

Venus-owned **processes**. They apply, persist, or convert. They do not render the editor.

| Component | Compose | Source | Role |
|---|---|---|---|
| [Hub](./hub/) | `hub` | `crates/venus-hub` | Live CRDT merge buffer: y-octo apply, AFFiNE WS broadcast, Postgres persist. Wiki sticky on `workspace_id`. |

Hub is **not** BlockSuite, not `fromDoc`, not git, not `jobs`. Many pages (and the catalog tree) are extra `doc_id`s **inside** that one wiki owner — [M4](../../M4/README.md), [CRDT tree](../frontend/crdt-tree/).
