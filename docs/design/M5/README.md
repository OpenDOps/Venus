# M5 — Semantic graph backend: SurrealDB + surrealastic

**Status:** in progress — steps 1–7 done (step 7 on 2026-10-08). Next is [step 8](../SemanticGraph/search-scale/plan.md#8-step-query). Plan slice: [venus-implementation-plan — M5](../venus-implementation-plan.md#m5--semantic-graph-backend-surrealdb--surrealastic-6-weeks).

M5 is board 1 of the [semantic graph](../SemanticGraph/README.md#boards): the [search cluster](../SemanticGraph/search-scale/README.md). It has no plan of its own. The step-by-step is that board’s.

| | |
| --- | --- |
| **Plan** | [SemanticGraph/search-scale/plan.md](../SemanticGraph/search-scale/plan.md) (`ss-cluster`) |
| **Board** | [SS.state.yaml](../SemanticGraph/search-scale/SS.state.yaml) |
| **Gate** | [Search-scale — gate](../SemanticGraph/search-scale/plan.md#gate). Step 7 hooks after `last_flushed`; the [M4](../M4/README.md) person pass does not block it. |
| **Exit** | [Search-scale — exit](../SemanticGraph/search-scale/plan.md#exit) |
| **Design** | [search-scale](../SemanticGraph/search-scale/README.md), [scale.md](../SemanticGraph/scale.md), [store.md](../SemanticGraph/store.md), [semantic graph README](../SemanticGraph/README.md) |

Fixture docs only. No extractor, no LLM, no graph UI. Does not run CodeGraph or Aider on the wiki. Does not change Flush. Does not replace [M7](../venus-implementation-plan.md#m7--lease--freeze-week).

Extraction, the graph view, vectors, and the semantic model are [M6](../M6/README.md).
