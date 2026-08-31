# Agents

**Main product feature:** a **multidimensional spec graph** — document parts bind to each other **and** to a timeline of explained diffs. That is how an LLM wiki is agentic-native: the model (and the human who aims it) get neighborhood **and** why, not a page plus search.

This folder is the design of that feature and of bound chat on top of it. It hangs off a [LiveSnapshot](../LiveSnapshot/README.md) SHA. It is **not** the M0–M8 dual-store spine (those make git honest). It is **not** the orchestrator ([product-plan](../../product/product-plan.md) — lease MCP, runner, accept inbox). Product statement: [product-plan — Multidimensional spec graph](../../product/product-plan.md#multidimensional-spec-graph). Two gits + **spec-bound** analyzer: [code-bind](./code-bind.md) (**select** CodeGraph CLI / Aider / both), [product-plan — spec-bound review](../../product/product-plan.md#spec-bound-review). Marketing: [agentic-comparison](../../marketing/agentic-comparison.md) (graph vs Notion/Cursor, LLM wiki design). Pain: [why is it designed this way](../../product/pains.md#7-why-is-it-designed-this-way), [PR review does not know the spec](../../product/pains.md#9-pr-review-does-not-know-the-spec).

Do not sell this as shipped until **AB4**. AB4 cannot exist until **M6**. Spatial AB1 is “what else is in force,” not [pains §7](../../product/pains.md#7-why-is-it-designed-this-way).

## The graph (what we sell after AB4)

| Axis | What is linked | What you can ask |
|---|---|---|
| **Spatial** | Headings / pages at a git SHA (`defines`, `depends-on`, `constrains`, `contradicts`; plus direct links) | If I change this, what else is in force? |
| **Temporal** | Those same parts → comment-commits, review comments, supersedes / reverts | Why is it designed this way? Where did we decide badly — what SHA do we revert? |

One index, sidecar `blockId`s, clocked to published git. Snapshot autocomments are *what moved*, not why. The why is the **accepted** comment-commit (and the rail). Contract: [LifeIndexing.md](./LifeIndexing.md).

Human chrome is **user-to-agent**: point at a span, the engine expands this graph (and later drafts via lease). Not a copilot bolted onto a human wiki UI ([agentic-native](../../product/product-plan.md#agentic-native-user-to-agent)).

## Build order (not the feature list)

High-level plan: [agentic-binding.md](./agentic-binding.md).

| Milestone | Ships |
|---|---|
| **AB1** | Spatial index after [M3](../venus-implementation-plan.md#m3--git-snapshotter-week). Not pain 7 / not the main feature. |
| **AB2** | Bound chat (**ask-only**; spatial pack; no wiki write) |
| **AB3** | Chat-edit via lease (**after M5–M6 checkout**, not after AB2) |
| **AB4** | Temporal why pack (**after M6** comment-commits, not after AB1) |
| **AB5** | Two gits + **spec-bound** analyzer on `productSha` (landed ↔ plan ↔ docs). **Select** CodeGraph CLI, Aider, or both. Review after plan-step implement. After M4 record remotes; do not delay M5. [code-bind](./code-bind.md) |

Do not teach agents Yjs. Do not put this index on the pin cut or `fromDoc` path. Do not sell the graph as the spec — git + meaning-accept remain the record.

| File | Role |
|---|---|
| [agentic-binding.md](./agentic-binding.md) | Parallel milestones AB1 → AB5 |
| [LifeIndexing.md](./LifeIndexing.md) | Spatial + temporal graph contract |
| [code-bind.md](./code-bind.md) | Wiki remote ≠ product remote; **select** CodeGraph CLI / Aider / both; post-implement review |
