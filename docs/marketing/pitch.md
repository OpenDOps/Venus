# Venus pitch

**Venus is your project heart.**

The heart is where feelings and wishes live — where you want to go — and aesthetics, the feeling of beauty. It also pushes blood so the body can work.

Venus is that for a project: it **accumulates** feelings and wishes (aims, intention, the spec humans accept). It **pushes specs like blood** through the full cycle — plan, implement, test, review, iterate — so the rest of the body (agents, PRs, code, reviews, tests, publications) can work. Without that pulse, the cycle is motion without a heart.

**Humans keep heart, instinct, faith and intention, while agents do the work.**

That is the product. Venus is the heart of the project — not the board and not the agent. It is a **spec-first development flow**, enforced by the wiki, the leases, the plans, the code and Hugo publications. Those are how the pulse is kept, not what to love.

**Product bar:** that loop must be **shorter than Notion + a PR**. If it is not, Venus is those products plus freeze and yaml. Details: [product-plan](../product/product-plan.md).

The hole Notion leaves: CRDT is for humans; **agentic development wants markdown in folders**. Venus is that wiki — collab for people, git tree for agents and developers, one spec. Pains (this gap, SDD vs Notion-spec / Scrum): [pains.md](./pains.md).

## How (technically)

Humans and agents share one spec that cannot silently drift, because nothing publishes without a lease and a human, and a story is not done until the spec diff is accepted.

Agents draft specs, plans, DoD, code, tests, API markdown, and Hugo-ready pages. They do not become the published record by typing into the live tree. A human accepts the intention — and the spec diff that records it.

That accept is a **CodeSpeak-like review cycle, for spec changes**: you accept human-language descriptions of what moved and why — when you understand them — not a mute patch of markdown or code. The hunks are how the change is tracked; the description is what you are saying yes to.

## What that forbids

- An agent merging a story while the spec still describes the old world.
- The same agent writing the definition of done and the tests that “prove” it.
- Accepting a mute spec patch, or a page rewrite whose meaning was never reviewed in human language.
- Markdown on git and the live page meaning two different products.
- A docs site (Hugo or otherwise) publishing anything that was not accepted into that spec git, or outdated.



## Where it lives in the design


| Line               | Design                                                                                                                 |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| Heart / intention  | Humans accept every spec, plan, DoD, and docs lease. Aims of the software are wiki pages, not generated afterthoughts. |
| Understand, then accept | CodeSpeak-like cycle on **spec** review: human-language descriptions of the change, pinned to hunks. Not accepting code. |
| Agents do the work | Planner, DoD author, implementer, autodoc generator — leased drafts, PRs, tests.                                       |
| One spec           | BlockSuite CRDT + git markdown snapshots; markdown is not a second live replica.                                       |
| No silent drift    | [venus-design.md](../design/venus-design.md), [venus-plan.md](../drafts/pre-design/venus-plan.md)           |




## Where Venus sits

```text
                    live collab spec
                           │
     Notion / AFFiNE       │     OpenKnowledge / Stele
     (no honest git loop)  │     (markdown is the CRDT)
                           │
                           ▼
                    Venus (this design)
                           │
     GitBook / Tina        │     Cursor / Spec Kit / Devin / CodeSpeak
     (docs+git, weak agent │     (agent+PR; CodeSpeak: chat→reqs on code)
      gate)                │
```

The empty cell is multiplayer spec + file snapshot + agent writes are leased + CodeSpeak-like accept of human-language spec-change descriptions + DoD from a different agent + story closed by human spec accept. Do not sell editor, board, or agent. Sell that gate.

Comparisons: [comparisons.md](./comparisons.md) — [PM tools](./comparisons.md#versus-pm-tools-linear-jira-plane-github-issues), [Notion](./comparisons.md#versus-notion-and-notion-agents), [Cursor](./comparisons.md#versus-cursor), [CodeSpeak](./comparisons.md#versus-codespeak). First-class product (runner, MCP, shared git): [product-plan](../product/product-plan.md).

Autodocumenting (APIs, aims, Hugo) is part of the same gate: generated files are still markdown in the spec tree; Hugo only builds **accepted** git. Details: [venus-plan.md](../drafts/pre-design/venus-plan.md) (autodoc + Hugo).