# Product plan: first-class for development and agents

This is a **product** doc: what to add, in what order, and what not to build so Venus is a real development app and an agent orchestrator. It is not marketing. **Unique features** (the join neighbors do not ship): [unique-features.md](./unique-features.md). Positioning vs Notion / Cursor / CodeSpeak stays in [comparisons.md](../marketing/comparisons.md). Bound chat vs Notion Agent / Cursor ask: [agentic-comparison.md](../marketing/agentic-comparison.md). Main wiki feature (spatial + temporal graph): [Agents](../design/Agents/README.md), [below](#multidimensional-spec-graph). Workspace (two gits) + **spec-bound PR review** (CodeGraph / Aider; landed ↔ plan ↔ docs): [below](#workspace-and-aider), [code-bind](../design/Agents/code-bind.md). The pitch stays in [pitch.md](../marketing/pitch.md). Pains: [pains.md](./pains.md). Editor, lease, git, MDGate stay in [design](../design/README.md). Plan invariants stay in [venus-plan.md](../drafts/pre-design/venus-plan.md). Wiki vs product trees: [venus-plan §2](../drafts/pre-design/venus-plan.md#2-wiki-repo-vs-product-repo) (lean: separate remotes / submodules).

## Product goal

**The spec loop must be shorter than Notion + a PR.**

If it is not, Venus is the same product as those, plus freeze and yaml. Dual store (live wiki + git files) only becomes a new category when the extra gates are **fewer than the drift they prevent**.

Bar: a PM, high-level engineer, or CTO writes prose in Venus, an agent (or source write) that **changes contract** goes through one meaning-accept, implementers ship from accepted git, merge is blocked until spec is current **or** the change is marked skip. That path must beat: type in Notion, open a GitHub PR, maybe remember to update the page.

Keep until that bar is green: MDGate, git snapshot, lease on **agent** writes, meaning-accept, skip when spec did not move, status check on merge, tree + accept inbox without Cursor. **Spec-bound PR review** (landed ↔ plan ↔ docs) is how implement is judged against that bar, not a second bug-hunt.

**Main wiki feature** (agentic-native, not the loop bar): [multidimensional spec graph](#multidimensional-spec-graph) — spatial binds + temporal why. Design: [Agents](../design/Agents/README.md).

Cut from v1 (they make the loop longer than Notion + a PR): two-agent DoD as a **hard** rule (warning only), alternatives/stacks, Hugo, Linear mirrors, “every story is a plan.”

M0–M8 is the spec wiki (dual store). First-class is that **short** outer loop. Cursor (and CI, PRs) stay the inner loop.

## Agentic-native (user-to-agent)

Venus is an **agentic-native** environment. It is not a human wiki, board, or editor with an agent **adopted onto** the same UI (Notion Agent, copilots in docs, bots that click Linear).

Those products were built for humans. The agent is a guest: it searches the page, types the page, or mimics the ticket. The human tool stays the product; the model is bolted on.

Venus inverts that. Git markdown, lease, hunks, Bind pack, comment-commit why — those surfaces exist so **agents can work**. The human UI is how you **aim** them: point at a span, bless meaning, skip when spec did not move. That is a **user-to-agent tool**, not an agent-in-a-human-tool.

A **main product feature** of that environment is the **multidimensional spec graph**: parts bind to parts, and parts bind to the timeline of accepted whys ([below](#multidimensional-spec-graph)).

WYSIWYG still exists so PMs, high-level software engineers, and CTOs can write prose without Cursor. It is not the agent’s interface. Do not teach agents Yjs. Do not ship “copilot in the page” as the native path.

Positioning: [agentic-comparison.md](../marketing/agentic-comparison.md). Pains: [agents on human UIs](./pains.md#8-agents-bolted-onto-human-tools), [why is it designed this way](./pains.md#7-why-is-it-designed-this-way).

## Multidimensional spec graph

**Main feature** of the LLM wiki (with dual store and meaning-accept). Not a search index bolted on later. Do not sell it as shipped at AB1: spatial without temporal why is “what else is in force,” not [pains §7](./pains.md#7-why-is-it-designed-this-way). The temporal axis cannot exist until **AB4**, which cannot start until **M6**.

The spec is a graph with **two axes**, keyed by stable block ids at a git SHA ([LifeIndexing](../design/Agents/LifeIndexing.md), [Agents](../design/Agents/README.md)):

| Axis | Links | Product use |
|---|---|---|
| **Spatial** | Heading ↔ heading (`depends-on`, `constrains`, `contradicts`) and direct wiki links | Change this without violating that. Bound chat packs the neighborhood. |
| **Temporal** | Heading ↔ comment-commits + review comments (`decided-in`, `supersedes`, `reverts`) | “Why is it designed this way?” Full history and reasoning on the clause. Name the bad decision and revert (M8). |

Notion has page history (a blob) and `@` / search. Cursor has why in **chat**, not on the hunk. Venus already stores why on comment-commits; the graph makes that **retrievable with the selection**. Snapshot autocomments are not why. Pain this closes: [pains §7](./pains.md#7-why-is-it-designed-this-way). Comparison: [agentic-comparison — graph](../marketing/agentic-comparison.md#how-the-multidimensional-graph-compares).

This is what agents and that audience expand when they point at a span. It is not a second spec. Build: [agentic-binding](../design/Agents/agentic-binding.md) AB1 (spatial, after M3 — not pain 7). **AB4** (temporal why) **after M6**, not after AB1. Bound chat (**AB2**, ask-only) consumes the spatial pack until then; chat-edit (**AB3**) waits on **M5–M6 checkout**, not on AB2.

Cut from v1 **loop** (Notion + a PR bar) still does not include this graph — dual store + meaning-accept can ship first. Cut it from the **product story** and Venus is honest git plus a copilot. Keep it in the story from day one.

## The gap: CRDT is for humans, folders of markdown are for agents

CRDT (and Notion) was built so **humans** can type the same page at once. It is a bad agent interface: no clone, no grep, no PR, a proprietary graph, live writes. Agentic development hardens when the wiki is **markdown in folders** — ordinary git, simple for developers, what Cursor already eats.

Notion as docs for that loop **does not fit**. Export is a dump. Agents type the live graph. The spec is not files.

Venus closes that gap: humans keep a collab page (CRDT / WYSIWYG). Agents and developers get **the same spec as a git tree of `.md`**. MDGate is what makes those one spec, not two. Do not teach agents Yjs. Do not make developers live in Notion.

## Dual store is honest

Two representations of **one** published spec: the live page (BlockSuite CRDT — what PMs, high-level engineers, and CTOs type) and git markdown (what implementers and agents clone). Honest means they are the **same spec**, not two workspaces that drift.

| Honest | Dishonest (Notion + a PR, or a broken Venus) |
|---|---|
| Clone of `wiki/` reads as the page that audience saw | Export rewrites wording, drops ids, invents hunks |
| After flush / accept, git HEAD **is** that page | CRDT moved; git is last Tuesday; Cursor implements a lie |
| Opaque / lossy bits are named and not silently rewritten | Adapter jitter; agents “fix” noise |
| One exporter for pane, snapshot, lease `T0`, review Before | Three markdowns of the same doc |
| Lag during the idle snapshot window is bounded; **flush-before-lease** makes `T0` = HEAD | “Not two replicas” in the doc, two replicas in the clone |

This is [MDGate](../design/MDGate/README.md) plus [LiveSnapshot](../design/LiveSnapshot/README.md). Until the fixture suite is green, the dual store is a claim. The unification idea is useless if git is a dump.

## Fewer gates

**Notion + a PR today:** write a page (workspace A) + open a PR (workspace B) + *maybe* update the page. Three human actions, two objects, docs often skipped.

**Venus target (one human extra at most, same PR they already do):**

| Change | Gates |
|---|---|
| Prose / typo / aims narrative | **0** — WYSIWYG, snapshot git. No lease. |
| Bugfix, spec did not move | **0** Venus — skip. One PR (already had). Merge check is green on skip. |
| Contract moved (human or agent) | **1** meaning-accept in Venus + **1** PR. Merge check waits on that accept. |

That is shorter than Notion + a PR because there is **one spec**, skip is explicit, and “remember to update Notion” is replaced by a status check — not by more reviews.

**Do not add gates:** plan accept + DoD accept + spec accept on every story; two-agent DoD as a block; alternatives/stacks; a board. A plan is optional. DoD-from-another-agent is a **warning** in v1. Merge check is a machine gate, not a fourth human.

The extra gate that is allowed to exist is **only** meaning-accept when contract moves. If you need more than that to ship, the loop lost.

## Principle

Venus **orchestrates gates**. It does not host the IDE, the board, or the agent runtime.

- Humans keep intention: CodeSpeak-like accept of **spec meaning**, not mute patches.
- Agents do the work under lease; implementer lives in the product repo. Two-agent DoD is a **warning** in v1, not a hard gate.
- The hop between Venus and Cursor is **git**, not the clipboard. The runner kicks **Cursor CLI** (or SDK) to implement. **Cursor IDE is not required** for that loop. **CodeGraph CLI and/or Aider review that PR or branch against the spec** (landed ↔ plan ↔ docs) — they do not implement. **Select** the reviewer: [code-bind — select](../design/Agents/code-bind.md#select-codegraph-cli-or-aider-or-both).

Venus is not only a wiki app. It is a **spec-first development flow**: one spec instead of Notion + git + agent chat. The app enforces the short loop (lease, git, skip, merge check). Teams that keep Figma, Notion, or Linear still need this once agents write code. Those tools are surfaces. They are not the pulse.

## Who needs the flow (including small teams, Figma, Notion)

Every team that ships with agents needs a spec-first loop. Small teams need it **more**: fewer humans to notice drift, more agent output per person. “Intention lives in one head” dies the first week a composer implements from chat.

Figma is canvas, not a spec agents can clone and not silently rewrite. Notion is a living workspace; its agents type the live page. Linear closes a card. None of that is “human accepted the meaning of the spec, then Cursor ran, then the spec caught up.” A Figma+Notion shop should **keep** Figma for design and Notion for ops if they want. They should not treat either as the artifact agents must not drift from.

What they may refuse is a **heavy second wiki**, not the loop. If Venus is the flow (few gates, shared git, skip when spec does not move), those teams can accept it. If Venus is “replace Notion and Figma,” they will not — and should not. Do not sell Venus as their home screen. Sell it as the framework that makes the rest of the body honest.

**PMs, other managers, high-level software engineers, and CTOs (and people next to that seat — VP Eng, chief architect) are first-class.** They will not adopt an IDE. They adopt Venus: wiki, tree, WYSIWYG, **accept inbox**. That is their loop — write, aim, bless meaning. Implementers still clone accepted wiki git; they are not this list.

**Why they need it too** (not only PMs):

| Who | Why Venus, not Cursor / git / a board |
|---|---|
| **High-level software engineers** | They own the **constraints** agents will implement (architecture, freeze, APIs, “must not”). That is meaning-accept, the [why graph](#multidimensional-spec-graph), and **[spec-bound PR review](#spec-bound-review)** (did the landed work follow those clauses), not a PR wall or a chat thread. They still clone git when they want; they will not live in the IDE to bless contract. If they only review code, the spec rots and the next agent re-litigates the design ([pains §7](./pains.md#7-why-is-it-designed-this-way)). |
| **CTO / VP Eng / chief architect** | They are accountable for **did we ship the world we meant**, not for typing in Composer. Agent output outruns their ability to read every diff. The pulse is an **accept inbox** (contract moved / skip) plus a tree they can read, not Linear “Done” and not an IDE they will not adopt. Close-to-CTO people run the org through those gates. |

Without this audience in Venus, only PMs have a heart and the technical bar lives in chat. That is Notion + Cursor again.

**The agentic loop does not require the Cursor IDE.** Venus **controls the implementer job**: pack at `wikiSha`, kick Cursor **CLI** (`agent -p` / SDK) on the product checkout, get a branch/PR, kick Aider/CodeGraph **spec-bound review**, gates. Humans may still open the IDE to debug or type by hand. That is optional inner-loop chrome, not the product. If Venus *is* a Composer clone, Cursor already won. If Venus requires the IDE to participate, this audience has no heart and programmers are glued to a window they no longer need for the loop.

## Force these (or it is not the flow)

The app can exist as a collab wiki and still be skippable. To make Venus **the** spec-first flow, the design must refuse the skip — not as a preference, as an invariant.

| Force | If you do not |
|---|---|
| Spec / plan / DoD / docs **apply is human-only**. Agents draft; they do not publish. | Agents become the record (Notion). |
| Accept is **meaning**: human-language description of the spec change, when the human understands it. Mute apply is forbidden. | You are accepting code that happens to be markdown. |
| A story that **moved spec** is not done until that spec accept. PR merge is not done. | Linear wins; spec rots. |
| **Skip** when spec did not move. No lease theater on a bugfix. | Teams abandon the flow. |
| Planner ≠ DoD author ≠ implementer | **v1: warn**, do not block. Hard refuse is later — it lengthens the loop past Notion + a PR. |
| Agents do not type the live CRDT. Lease + hunks only. | Freeze is theater. |
| **Git is how programmers see the spec.** Shared checkout; no copy-paste. | Cursor and Venus are two products. |
| **This audience never needs Cursor** (IDE or CLI). WYSIWYG + accept inbox + tree is a complete path to write and bless intention. PMs, other managers, high-level engineers, CTOs. | Only day-to-day implementers have a heart. |
| Header shows the **gate** (who holds the lease, what is waiting to accept), not a kanban. | People live on a board. |
| **Environment is agentic-native.** Human chrome **aims** the agent (bind, accept). Agents do not fake the human UI. | You bolted a copilot onto Notion/Linear. They already won that. |
| **Cursor CLI implements. Aider / CodeGraph CLI review the PR or branch against the spec** (landed ↔ plan ↔ docs). The **IDE is optional.** | You sold a second IDE, or review is a bug-hunt with no specification. |

Data-model invariants (CRDT, freeze, two git classes) stay in [venus-design.md](../design/venus-design.md). These are **flow** invariants. Product: [below](#add-this--first-class-gap). v1 documenting the product: [implementation plan](../design/venus-implementation-plan.md#v1-dogfood--document-the-product).

## Shared git, not copy-paste

Humans and agents must not paste spec into chat or the IDE. They work from a **shared checkout** of the workspace: wiki git + product git, **two remotes or submodules, separate histories** ([venus-plan § wiki vs product](../drafts/pre-design/venus-plan.md#2-wiki-repo-vs-product-repo) — lean **C**). That split is decided. Parent-vs-nested (workspace-of-two vs product-owns-wiki) is chosen when the first product remote is wired. Do not assume the product repo is the same git as `wiki/`.

| Direction | How |
|---|---|
| **PM / high-level eng / CTO, prose / aims / narrative** | WYSIWYG → snapshot git. No ceremony. |
| **Contract** (behavior, API, DoD, must-clauses) | Human or agent: lease + meaning-accept. |
| **Agent** | Always lease. Never live CRDT. |
| **Programmer / implementer** | Read accepted **wiki** git. Product writes: Venus kicks **Cursor CLI** (or SDK) on the product repo → branch / PR. Spec writes through Venus if contract moved. **IDE is optional** (debug, hand-edit). |
| **Aider / CodeGraph CLI** | **Review only:** the agentic-loop **PR or branch** at `productSha`, **against the bound spec/plan**. Not a bug-hunt dump. **Not implementation.** Does not publish spec. **Select** before AB5. |

v1: clone-and-PR import of wiki markdown is later ([MDGate](../design/MDGate/README.md)). Until the adapter gate is green, do not tell agents “edit the `.md` in git and it will apply.” They acquire a lease (MCP or UI) and put hunks; accept still lands as a wiki commit the next `git pull` sees.

Wire the two remotes (or submodules) into the runner and MCP **before** implement starts. Design: [code-bind](../design/Agents/code-bind.md).

## Workspace and Aider

A Venus workspace **binds** the wiki remote to a Venus-controlled product (separate repo + branch). Bind object: `{ wikiSha, productSha, productBranch }`. Notion’s GitHub connector dumps sources into chat with no clock. Venus only packs code when the recipe asks, at the **pinned** product SHA.

**Aider and CodeGraph CLI are for agentic-loop PR (or branch) review, not for implementation.** **Cursor CLI** (the named implementer) writes the product branch and opens the PR — Venus kicks that job; the IDE is not required. The runner then kicks the selected analyzer on **that PR or branch**. They do not `git commit` product code, do not replace Cursor CLI, and do not type the wiki.

### Spec-bound review

This is a **product feature**, not “we also run CodeGraph.” Notion and Cursor both lack it ([comparisons](../marketing/comparisons.md#spec-bound-pr-review)).

**Landed feature bound to plan, plan bound to docs.** That chain is the review working set.

When the analyzer reviews a PR, Venus **builds a bound pack**, not a repo dump:

```text
landed feature   (diff at productSha' — the PR / branch)
      bound to
plan step        (the wiki plan that asked for that work)
      bound to
documentation    (accepted spec / aims / API at wikiSha)
```

The review **must validate code against the specification**. Ordinary bug-hunt (mistakes, blast radius, tests) is not enough. The pack exists so the reviewer can answer:

| Question | Without the bind | With the bind |
|---|---|---|
| Did we make a mistake? | GitHub / Cursor diff review | Same, plus CodeGraph impact at `productSha'` |
| Did we **follow the spec**? | Hope someone `@`’d the right README | Headings the plan was accepted against, at `wikiSha` |
| How **close to the goal**? | Ticket title, or the chat that kicked implement | The plan step’s named outcome, bound to those docs |

`implements` (heading → path/symbol at `productSha`) is how the landed feature stays tied to the clause. Drift is a **contradiction for a human** (another implement pass or a spec lease) — not CodeSpeak (update the spec from HEAD) and not Notion (agent types the page).

**Select CodeGraph CLI, Aider, or both** before that kick. Lean default (**both, split jobs**): [code-bind — select](../design/Agents/code-bind.md#select-codegraph-cli-or-aider-or-both).

| | CodeGraph CLI | Aider |
|---|---|---|
| Job on the **PR / branch** | Queryable graph at `productSha` (callers, impact, `implements`); **spec pack** on the review | LLM **review comments** on that diff against the same pack; repo map if CodeGraph is off |
| Implementation | **No** | **No** |

Neither replaces GitHub human review or meaning-accept. v1: always **run** the selected reviewer(s) after a branch/PR exists; flags are a **warning** (same softness as two-agent DoD). Later the step may fail. Merge check stays spec current or skip — not “Aider approved” and not “close enough to the spec.”

Do not enrich every wiki request with the codebase. Do not delay M5 for this. After [M4](../design/venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks) **record** the remotes; **select** the reviewer; the kick is on the runner ([AB5](../design/Agents/agentic-binding.md#ab5--code-bind--aider)). Design: [code-bind — automated review](../design/Agents/code-bind.md#automated-review-after-implement).

## Already designed — ship the spine

Without these, orchestration is theater. They are [implementation plan](../design/venus-implementation-plan.md) M2–M8, not this file’s invention.

1. **MDGate** — honest git markdown. No jitter, stable ids.
2. **Git snapshots + catalog** — cloneable spec; casual WYSIWYG without a ceremony.
3. **Lease, freeze, comment-commit** — Cursor-like checkout; CodeSpeak-like accept of spec meaning.
4. **Agent = lease holder** — same `acquire` / `putHunks` / `comment` as a human. Must be an **API**, not only a UI.

That is the spec store. It is not yet a development app.

## Add this — first-class gap

The implementation plan stops at M8 (revert + regenerate). **Plans, runner, agent protocol, and the accept inbox are not in that sequence.** They are this product.

### 1. Agent protocol (MCP + HTTP)

Lease-shaped tools. **Cursor CLI** stays the implementer (IDE optional). Venus talks git + this API + a kick (`agent -p` / SDK), not a hosted Composer.

- Read published spec / plan / DoD **at a git SHA** (disk after pull, or the same bytes via API). Product tree **at `productSha`** when the workspace has a product remote.
- `acquire` / heartbeat / release; `putHunks`; submit a **human-language description** of the spec change.
- List open leases, waiting accepts, plan state, `pr` URLs.
- **v1:** warn if `dodAgentId` is missing or equals planner/implementer. **Later:** refuse. Hard refuse in v1 makes the loop longer than Notion + a PR.

Selection-grounded **read** context (bind a span, expand LifeIndexing pack; optional CodeGraph / Aider pack at `productSha`) is a **different** tool family: [agentic-binding](../design/Agents/agentic-binding.md). Same MCP server later may host both; lease writes stay this section.

### 2. Plan runner (orchestrator)

Not a board. Not “every story is a plan.” A state machine on `*.state.yaml` **when a plan exists**:

- Optional: spec (or a section) → planner lease → human accept → implementer may run. DoD from another agent is a **warning** in v1, a hard gate later.
- Step: `implement (Cursor CLI) → PR or branch → Aider / CodeGraph **spec-bound** review (that PR/branch) → tests → breakpoint → done`. Runner kicks **external** CLIs. It does not write code. **Cursor CLI implements** (`agent -p` / SDK). **IDE is optional.** Review pack is **landed ↔ plan ↔ docs**, not a repo dump ([code-bind](../design/Agents/code-bind.md#automated-review-after-implement)). **Select** the reviewer CLIs first.
- Overlapping spec leases serialize.
- Story cannot go `done` without spec accept **if contract moved**. **Skip** if it did not.
- **Merge status check:** GitHub (or equivalent) is red until Venus says spec is current or skipped. Without this, the loop is social and Linear wins.

### 3. Human inbox of meaning-accepts

The team surface is not the page editor. It is a **short** queue — not plan + DoD + spec as three required clicks on every story:

- Spec (contract) moved — understand what the wiki now says.
- Plan ready — only if someone opened a plan.
- DoD ready — v1 optional / warn.

Each item is description + hunks as evidence. Header chrome: current plan, who holds the lease, next gate, link to PR. Not cycles, points, or kanban. If accept is buried in a wiki tab, people rubber-stamp or skip.

### 4. Agent roles as accepted wiki

Planner / DoD / implementer / autodoc are jobs with ids on the plan. Instructions for those jobs are spec pages (same lease/accept). Orchestration is who may write which lease — not a Notion teammate with Slack triggers.

### 5. Freeze as a team feature

Heartbeat, steal, expiry — already in [lease-freeze-rationale](../design/lease-freeze-rationale.md); they are product, not polish. Show who holds what, for how long. Whole-page freeze is the **current** design (Cursor-like for the prompter); extend later (section/folder leases) once the loop is boring.

### 6. Close the git hop

- Agents read accepted git from the workspace checkout; humans open Venus **at gates**.
- PR URL on the yaml; breakpoint is GitHub + Venus notify, not a second review of the same hunks.
- “Open this plan in Cursor” is a **CLI kick** (clone/SHA + `agent -p` / SDK) on the product branch, not a required IDE window.
- After Cursor CLI implements, kick Aider / CodeGraph CLI on that **PR or branch** — **spec-bound review** (landed ↔ plan ↔ docs), not bug-hunt only ([Workspace and Aider](#workspace-and-aider)).

Spatial + temporal graph is a **main feature**, not this gap list: [Multidimensional spec graph](#multidimensional-spec-graph). Two gits + **spec-bound PR review** is this gap: [Workspace and Aider](#workspace-and-aider) / [spec-bound review](#spec-bound-review).

## Do not add

| Temptation | Why it kills Venus |
|---|---|
| In-wiki coding / Cursor **IDE** clone | Inner loop already won. Venus kicks **CLI**, it does not host Composer. |
| Venus “chat control” as the spec | Transcript is not the spec. Venus owns the **job** (kick, SHA, PR URL, next gate), not a second Cursor thread that becomes the record. |
| Copy-paste spec into the IDE | Shared git is the hop |
| 24/7 Custom Agents that write the live page | Notion; silent spec |
| Linear-like home board | People live on the board; spec dies |
| Linear mirrors, Hugo, alternatives/stacks in v1 | Loop longer than Notion + a PR |
| Every story is a plan | Same |
| Two-agent DoD as a hard v1 block | Same |
| Chat as the spec | CodeSpeak’s object, opposite pulse |
| Mute apply of markdown | Accepting code, not meaning |
| Cursor required to bless spec | PMs, high-level engineers, and CTOs cannot adopt the flow |
| Bolting an agent onto a human wiki/board as the product | They already won “copilot in the tool.” Venus is user-to-agent ([agentic-native](#agentic-native-user-to-agent)). |
| Aider or CodeGraph dumped into every wiki ask | Notion connector. Pack code only on recipe, at `productSha`. |
| Aider or CodeGraph as **implementer** | Cursor writes the branch/PR. These CLIs **review** that PR or branch **against the spec**. Selling them as coding agents kills the loop. |
| CodeGraph / Aider as a GitHub bug-hunt bot | Without **landed ↔ plan ↔ docs**, it is Cursor’s review. Notion and Cursor already do mistakes. The bind is the feature. |
| Shipping AB5 with no `codeAnalyzer` pick | Must select CodeGraph CLI, Aider, or both ([code-bind](../design/Agents/code-bind.md#select-codegraph-cli-or-aider-or-both)). |

## Order

Judge each slice against the goal: **shorter than Notion + a PR**.

1. MDGate + git snapshot + tree (dual store works; this audience writes prose).
2. Lease on **agent** (and contract) writes + meaning-accept + skip.
3. **Status check on merge** (otherwise the loop is a lecture).
4. MCP + accept inbox (shared git; no clipboard; PMs, high-level engineers, and CTOs never need Cursor).
5. Plan runner **optional** (not every story). Two-agent DoD warn, then later refuse. Kick **Cursor CLI** to implement (IDE optional). **Aider / CodeGraph review the PR against the spec** (landed ↔ plan ↔ docs). Select reviewer CLIs **before** that kick.
6. Hugo / autodoc / Linear mirrors — only after the bar is green.

Until the dual store is honest and a contract change is **one** meaning-accept plus a PR (or skip), Venus is AFFiNE + a GitHub workflow spec, aimed at a Notion user, justified by a Cursor metaphor. That can become the flow. The product goal is to **make the loop shorter**, not to add gates.
