# Semantic graph

**Status:** not started. Step-by-step: [plan.md](./plan.md). Parent: [M5 plan](../plan.md) board 6. Rules: [connect.md — semantic](../connect.md#semantic--background-model).

An LLM reads dirty headings and writes typed edges the static passes cannot see: `defines`, `depends_on`, `constrains`, `contradicts`, `supersedes`. Each edge carries a quote that is a substring of the source heading. Edges without that quote are dropped. `links_to` stays the edge for a real href. A cosine neighbor stays a candidate, not an edge.

The call is one JSON completion per dirty page, on a Venus model key. A normal flush does not rescan the wiki. A full rescan runs when the edge types or the schema change, when gist drift trips the limit, or on the nightly safety net.

This board starts after [extraction](../extraction/plan.md) has `links_to`. It uses [vector](../vectors/plan.md) neighbors as a candidate source when that index exists. The sequence from that index to a typed edge is [board 5 to board 6](../plan.md#from-board-5-to-board-6). It does not wait on Tantivy or the graph view.
