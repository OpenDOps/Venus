# M5 — plan

The first board is the search cluster: [search-scale/plan.md](./search-scale/plan.md), board [SS.state.yaml](./search-scale/SS.state.yaml). What is already running, which layer each step builds, and the remaining steps: [where the board is](./search-scale/plan.md#where-the-board-is).

Design of the layers: [scale.md](./scale.md). Design of the cluster: [search-scale/README.md](./search-scale/README.md).

Steps 1–4 are done. Replication is the shipped layer: `crates/surrealastic` writes a fenced log to equal copies, and Compose runs one graph node plus two search nodes. The layout, Venus writes, the router, recovery, the monitor, the third search node, graph copies, backups, and shard splits are steps 5–14.
