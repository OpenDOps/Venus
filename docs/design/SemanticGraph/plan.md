# Semantic graph — first board

The first board is the search cluster: [search-scale/plan.md](./search-scale/plan.md), board [SS.state.yaml](./search-scale/SS.state.yaml).

It builds `surreal-graph` (edges, no full-text index) and `surreal-search` (full SurrealDB server, search schema only). Dev shape is two search nodes, `shard_count = 2`, `replica_count = 1`. The HA default, step 7 of that plan, is a third search node with `replica_count` still 1.

Design of the split: [scale.md](./scale.md). Design of the cluster: [search-scale/README.md](./search-scale/README.md).
