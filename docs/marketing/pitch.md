# Venus pitch

**Venus is your project heart.**

The heart is where feelings and wishes live — where you want to go — and aesthetics, the feeling of beauty. It also pushes blood so the body can work.

Venus is that for a project: it **accumulates** feelings and wishes (aims, intention, the spec humans accept). It **pushes specs like blood** through the full cycle — plan, implement, test, review, iterate — so the rest of the body (agents, PRs, code, reviews, tests, publications) can work. Without that pulse, the cycle is motion without a heart.

**Humans keep heart, instinct, faith and intention, while agents do the work.**

**Promote that way of managing software first** — spec-driven development as the successor to Scrum/Agile-as-ceremony. Then Venus is the **tool**: it replaces **Jira + Confluence** for how you design and ship the product. For **technical design specifications**, it replaces **Confluence** (not a worse board, not a second company wiki). Confluence is not lightweight like Notion; that heaviness is the clipboard hop. The market is teams that still run agile-era process while agents write code — not only people who already keep an LLM wiki or already live in Notion ([product-plan — Category](../product/product-plan.md#category-the-way-of-work-then-the-tool)).

That is the product. Venus is the heart of the project — not the board and not the agent. It is a **spec-first development flow**, enforced by the wiki, the leases, the plans, the code and Hugo publications. Those are how the pulse is kept, not what to love.

**Product bar:** that loop must be **shorter than Notion + a PR**. If it is not, Venus is those products plus freeze and yaml. Details: [product-plan](../product/product-plan.md).

**Product use:** (1) a **team call** to brainstorm software design on collaborative docs — not one engineer writing an LLM wiki in a personal IDE. During or after, agents fit that session into wiki structure. The call leaves a **session-bound commit** — a document diff of what was consensused ([product-plan](../product/product-plan.md), Session-bound commit). Product, project, marketing, and development meet on one spec that has to satisfy every department ([product-plan](../product/product-plan.md), Product use: brainstorming). (2) **Share while Cursor stays the IDE:** commit markdown, the team edits WYSIWYG, you pull back. Venus is the share surface, not a replacement for the editor ([product-plan](../product/product-plan.md), Product use: share).

The hole Notion leaves: CRDT is for humans; **agentic development wants markdown in folders**. Venus is a **framework for that LLM wiki** — collab for people, git tree for agents, one spec, **agentic process on by default** so you do not have to set up Notion Agent (which most people never do). It is **agentic-native**: a **user-to-agent** tool, not a human wiki with a copilot bolted on ([product-plan](../product/product-plan.md#agentic-native-user-to-agent)). Gonka-native Kimi and MiniMax are **in the subscription** so that framework runs on day one. Do not lead with the tokens. Lead with the way of work.

**Main feature of that wiki:** a **multidimensional spec graph** — document parts bind to each other, and to the timeline of accepted whys (design evolution, “why is it this way?”, which SHA to revert). Design: [Agents](../design/Agents/README.md). Marketing: [agentic-comparison](./agentic-comparison.md#why--timeline-main-feature). Pains: [pains.md](./pains.md).

## How (technically)

Humans and agents share one spec that cannot silently drift, because nothing publishes without a lease and a human, and a story is not done until the spec diff is accepted.

Agents draft specs, plans, DoD, code, tests, API markdown, and Hugo-ready pages. They do not become the published record by typing into the live tree. A human accepts the intention — and the spec diff that records it. **Cursor implements** the product via **CLI** (branch / PR); the IDE is not required for that loop. **Aider and CodeGraph CLI review that PR or branch against the spec** — landed feature bound to plan, plan bound to docs: not only mistakes, but did we follow the specification and how close are we to the goal ([product-plan](../product/product-plan.md#spec-bound-review)). They do not implement.

That accept is a **CodeSpeak-like review cycle, for spec changes**: you accept human-language descriptions of what moved and why — when you understand them — not a mute patch of markdown or code. The hunks are how the change is tracked; the description is what you are saying yes to.

## What that forbids

- An agent merging a story while the spec still describes the old world.
- The same agent writing the definition of done and the tests that “prove” it.
- Aider or CodeGraph CLI as the **implementer** (they review the PR or branch against the spec; Cursor writes the code).
- PR review that only hunts mistakes (no bind from landed feature to plan to docs).
- Accepting a mute spec patch, or a page rewrite whose meaning was never reviewed in human language.
- Markdown on git and the live page meaning two different products.
- A docs site (Hugo or otherwise) publishing anything that was not accepted into that spec git, or outdated.



## Where it lives in the design


| Line               | Design                                                                                                                 |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------- |
| Heart / intention  | Humans accept every spec, plan, DoD, and docs lease. Aims of the software are wiki pages, not generated afterthoughts. |
| Brainstorm         | **Team call** on collab docs — not one engineer’s IDE. Agents fit LLM-wiki structure. Product, marketing, development meet on one spec. |
| Session artifact   | Background agent unites live-session activity into a **commit bound to the call** — a measurable spec diff of consensus. Not a history slider. |
| Share (Cursor stays) | Commit markdown → team WYSIWYG → they edit → you `git pull`. Venus is the share surface, not a second IDE. |
| Understand, then accept | CodeSpeak-like cycle on **spec** review: human-language descriptions of the change, pinned to hunks. Not accepting code. |
| Agents do the work | Planner, DoD author, **implementer (Cursor)**, autodoc — leased drafts, PRs, tests. **Aider / CodeGraph CLI:** spec-bound review of that PR (landed ↔ plan ↔ docs), not implement. |
| Included runtime   | **Framework, on by default.** Frames how you work. Gonka-native Kimi/MiniMax in the subscription. Not a Notion Agent people never set up. |
| Spec-bound review | **Already in the loop.** CodeGraph enriches the PR. Lenses (security, perf, design, style) **plus spec plus docs-to-ship** (wiki / Hugo). Not a GitHub bot you forgot to install ([comparisons](./comparisons.md#spec-bound-pr-review)). |
| One spec           | BlockSuite CRDT + git markdown snapshots; markdown is not a second live replica.                                       |
| Spec graph         | **Main feature:** spatial binds + temporal why ([Agents](../design/Agents/README.md)). Not a copilot search box.         |
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

The empty cell is multiplayer spec + file snapshot + **team brainstorm that becomes LLM-wiki structure** + agent writes are leased + CodeSpeak-like accept of human-language spec-change descriptions + DoD from a different agent + story closed by human spec accept + **PR review bound to plan bound to docs**. Do not sell editor, board, or agent. Sell that gate.

Comparisons: [comparisons.md](./comparisons.md) — [PM tools](./comparisons.md#versus-pm-tools-linear-jira-plane-github-issues), [Confluence](./comparisons.md#versus-confluence), [Notion](./comparisons.md#versus-notion-and-notion-agents), [Cursor](./comparisons.md#versus-cursor), [spec-bound PR review](./comparisons.md#spec-bound-pr-review), [CodeSpeak](./comparisons.md#versus-codespeak). Unique join: [unique-features](../product/unique-features.md). Bound chat (ask, write, **why/history**): [agentic-comparison.md](./agentic-comparison.md). First-class product (runner, MCP, shared git): [product-plan](../product/product-plan.md).

Autodocumenting (APIs, aims, Hugo) is part of the same gate: generated files are still markdown in the spec tree; Hugo only builds **accepted** git. Details: [venus-plan.md](../drafts/pre-design/venus-plan.md) (autodoc + Hugo).