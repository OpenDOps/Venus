# Product plan: first-class for development and agents

This is a **product** doc: what to add, in what order, and what not to build so Venus is a real development app and an agent orchestrator. It is not marketing. **Category** (method first, then tool): [below](#category-the-way-of-work-then-the-tool). **Unique features** (the join neighbors do not ship): [unique-features.md](./unique-features.md) — including **team brainstorming** ([below](#product-use-brainstorming)), **share while Cursor stays the IDE** ([below](#product-use-share-cursor-stays-the-ide)), and **session-bound commit** ([below](#product-feature-session-bound-commit)). Positioning vs Notion / Cursor / CodeSpeak stays in [comparisons.md](../marketing/comparisons.md). Bound chat vs Notion Agent / Cursor ask: [agentic-comparison.md](../marketing/agentic-comparison.md). Main wiki feature (spatial + temporal graph): [Agents](../design/Agents/README.md), [below](#multidimensional-spec-graph). Workspace (two gits) + **spec-bound PR review** (CodeGraph / Aider; landed ↔ plan ↔ docs): [below](#workspace-and-aider), [code-bind](../design/Agents/code-bind.md). The pitch stays in [pitch.md](../marketing/pitch.md). Pains: [pains.md](./pains.md). Editor, lease, git, MDGate stay in [design](../design/README.md). Plan invariants stay in [venus-plan.md](../drafts/pre-design/venus-plan.md). Wiki vs product trees: [venus-plan §2](../drafts/pre-design/venus-plan.md#2-wiki-repo-vs-product-repo) (lean: separate remotes / submodules).

## Product goal

**The spec loop must be shorter than Notion + a PR.**

If it is not, Venus is the same product as those, plus freeze and yaml. Dual store (live wiki + git files) only becomes a new category when the extra gates are **fewer than the drift they prevent**.

Bar: a PM, high-level engineer, or CTO writes prose in Venus, an agent (or source write) that **changes contract** goes through one meaning-accept, implementers ship from accepted git, merge is blocked until spec is current **or** the change is marked skip. That path must beat: type in Notion, open a GitHub PR, maybe remember to update the page.

Keep until that bar is green: MDGate, git snapshot, lease on **agent** writes, meaning-accept, skip when spec did not move, status check on merge, tree + accept inbox without Cursor. **Spec-bound PR review** (landed ↔ plan ↔ docs) is how implement is judged against that bar, not a second bug-hunt.

**Main wiki feature** (agentic-native, not the loop bar): [multidimensional spec graph](#multidimensional-spec-graph) — spatial binds + temporal why. Design: [Agents](../design/Agents/README.md).

**Use that produces the spec** (not the loop bar): [brainstorming](#product-use-brainstorming) — a team call on collaborative docs; agents fit it to LLM-wiki structure. **Use that shares it while Cursor stays the IDE:** [share](#product-use-share-cursor-stays-the-ide) — commit markdown, team edits WYSIWYG, pull back. **Feature of live sessions:** [session-bound commit](#product-feature-session-bound-commit) — the call leaves a measurable spec diff.

Cut from v1 (they make the loop longer than Notion + a PR): two-agent DoD as a **hard** rule (warning only), alternatives/stacks, Hugo, Linear mirrors, “every story is a plan.”

M0–M8 is the spec wiki (dual store). First-class is that **short** outer loop. Cursor (and CI, PRs) stay the inner loop.

## Category: the way of work, then the tool

**Promote spec-driven development in the agentic era first** — the successor to Scrum/Agile-as-ceremony. Then Venus is the **tool for that work**. Do not lead with “LLM wiki” or “for teams that already keep spec in `docs/`.” Those people are a beachhead, not the market.

Scrum optimized **human coordination** (sprint, points, Jira ticket, Confluence page). Agents do not pull from a standup. The scarce question is **did we ship the world we meant**. The framework is: one accepted spec, humans keep intention, agents do the work, a story is not done until spec is current or skipped.

**Jira + Confluence was the agile pair** (board + wiki). Venus **replaces that pair for software development** — not a worse Jira (no cycles/kanban as the home screen). For **technical design specifications**, Venus **replaces Confluence**. Confluence is not lightweight like Notion: spaces, macros, permissions, a dump to markdown. Notion-like chrome is that lightness; the object is still a cloneable spec, not a Confluence clone and not an ops wiki. Intake, sales, and triage can stay in a ticket tool. Company ops wiki can stay Notion or leftover Confluence. **How the product is designed, accepted, and implemented** lives in Venus.

Do not sell “replace Notion and Figma.” Do not sell “everyone who has Jira.” Sell: teams that still run **agile-era process** while **agents write the code** need a new OS. The default spec wiki in that world is **Confluence**, not Notion. The **tool** is an **LLM-wiki framework**: agentic integrations are the process, on by default, and they frame how you work — that is what differs from Notion, Confluence, and Linear (optional AI most people never set up). Pain: [pains.md](./pains.md) (§3, §11, §13). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus PM tools, Versus Confluence, Versus Notion).

## Product use: brainstorming

An LLM wiki is usually created by **one engineer in a personal IDE**. Venus is a **team session**: people on a call write **together** in collaborative docs (live CRDT, prose, no lease).

The **editor is Notion-like on purpose.** WYSIWYG, live cursors, headings, pages — so product, marketing, and CTOs show up without learning git, Cursor, or a new docs OS. That is the learning-curve cut, and the **lightness Confluence does not have**. It is not “we are a Notion clone.” The wiki Venus replaces for **technical design** is Confluence.

During that session or immediately after, the same team **aims agents** to fit the notes into **LLM wiki structure** — folders, headings, marketing and product goals as pages, binds to constraints. Agents draft; humans meaning-accept. Do not implement this as Notion Agent (tokens on the live page). Bound chat (ask) can run during the call; structure that changes contract still goes through lease.

That is where **all directions meet**: product, project, marketing, development. They adjust the **end-product shape** by a specification that has to meet every department’s needs — not a deck in one tool, tickets in another, and `docs/` in Cursor.

### Vs Notion (same gestures, different object)

| | Notion (the call they already know) | Venus (same room, different heart) |
|---|---|---|
| What you type | A living page. That page **is** the product. | Familiar collab page. The **spec** is that page **plus** git markdown after flush/accept. |
| After the call | History slider. Export is a dump. Agents clone nothing honest. **The page moved; nobody has “what this call decided.”** | Clone of `wiki/` **is** what the room wrote. A **session-bound commit** is the measurable diff of consensus. |
| Agent in the session | Teammate **types the live page**. The model becomes the record. | Humans type the room. Agents **fit structure** under lease; you accept meaning. Ask-only chat may pack context. |
| Marketing / product goals | A page, a DB, a deck — usually not what Cursor implements. | Goals are **spec pages** on the same tree. The next agent must not drift from them. |
| Departments | Ops wiki they already live in. Spec for software is optional. | The meeting **is** software design. All directions stay on one specification. |
| Done | The page moved. | Meaning-accept when contract moved; skip if it did not; merge waits on that. |

Do not hide Venus behind a foreign UI (markdown-only, IDE-only) or the call never happens. Do not copy Notion Agent into that familiar UI or the call is a worse Notion. Familiar chrome; agentic-native gate.

The call is not done when the page moved. A **session-bound commit** is the measurable artifact ([below](#product-feature-session-bound-commit)).

The [loop bar](#product-goal) is how that spec then ships. Unique feature: [unique-features.md](./unique-features.md) (Collaborative brainstorming, Session-bound commit). Pain: [pains.md](./pains.md) (§10, §12). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus Notion).

## Product use: share (Cursor stays the IDE)

**Even if people keep Cursor as the IDE**, Venus is where the spec is **shared**. You are not donating a dump and losing the editors.

```text
Cursor (markdown)  --commit-->  Venus WYSIWYG  --team edits-->  git  --pull-->  Cursor
```

You write or shape markdown in Cursor (or any git client). You **commit**. The team sees that spec as a **WYSIWYG page** — they do not need Cursor or markdown. They change it in the collab page. You **`git pull`** and you are back in Cursor on the same spec. No export, no paste into chat, no one-way door ([pains §11](./pains.md#11-clipboard-import-export)).

This is a **second product use**, next to [brainstorming](#product-use-brainstorming). Brainstorming is how the room starts a spec. Share is how an engineer who still lives in the IDE **keeps that room on the same object**. Do not sell “replace Cursor as the editor.” Sell Venus as the **share surface**. Cursor stays inner-loop chrome for people who want it.

Prose can land by snapshot. **Contract** still takes lease + meaning-accept — a commit that changes must-clauses is not a mute apply. Agents never skip that by typing git. Humans who write markdown in Cursor are still on **one spec**, not a private `docs/` copy.

**Shipped vs story:** team WYSIWYG → git (you pull) is **M3**. Cursor commit → page (they see WYSIWYG) is **M6 apply**. Until the adapter gate is green, do not tell anyone “edit the `.md` in git and it will apply.” Lease + hunks still land as a wiki commit the next pull sees ([below](#shared-git-not-copy-paste)). Keep the use in the story from day one; do not sell the round-trip as shipped at M3.

Unique feature: [unique-features.md](./unique-features.md) (Honest dual store). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus Cursor).

## Product feature: session-bound commit

**Brainstorms and other live sessions stay a meaningful artifact.** Not a recording, not a history slider, not “the page moved.”

A **background agent** sees that there was session activity and **unites** that activity into a **commit bound to the session**. Everyone can open a **measurable document diff** and know what was achieved or consensused on that live call.

```text
live session (humans type, no lease)
      →  activity
      →  background agent unites (lease, not live type)
      →  commit bound to the session
      →  spec diff anyone can read
```

Idle snapshot autocomments (`snapshot: <title>`) are **not** this. They say the page flushed, not “this call agreed that.” Notion/Confluence activity is a blob. A Zoom recap in chat is gone next week.

The agent does **not** type the live page during the call ([brainstorming](#product-use-brainstorming)). It drafts after (or at session close) under lease. **Contract** still takes meaning-accept. Prose-only sessions still get a named session commit so the room has a diff — that is the artifact, not a fourth human gate on every typo.

**Shipped vs story:** needs git (**M3**) for the commit to exist, then agent lease to unite. Keep it in the story from day one. Do not sell it as shipped at M3 idle flush. Do not delay M3 for the agent. Product gap: [below](#7-session-bound-commit). Unique feature: [unique-features.md](./unique-features.md) (Session-bound commit). Pain: [pains.md](./pains.md) (§12).

## Agentic-native (user-to-agent)

Venus is an **agentic-native** environment. It is not a human wiki, board, or editor with an agent **adopted onto** the same UI (Notion Agent, copilots in docs, bots that click Linear).

Those products were built for humans. The agent is a guest: it searches the page, types the page, or mimics the ticket. The human tool stays the product; the model is bolted on.

Venus inverts that. Git markdown, lease, hunks, Bind pack, comment-commit why — those surfaces exist so **agents can work**. The human UI is how you **aim** them: point at a span, bless meaning, skip when spec did not move. That is a **user-to-agent tool**, not an agent-in-a-human-tool.

A **main product feature** of that environment is the **multidimensional spec graph**: parts bind to parts, and parts bind to the timeline of accepted whys ([below](#multidimensional-spec-graph)).

WYSIWYG still exists so PMs, high-level software engineers, and CTOs can write prose without Cursor. It is not the agent’s interface. Do not teach agents Yjs. Do not ship “copilot in the page” as the native path.

**Agents are first-class, not a connector.** Venus is a **framework for an LLM wiki**: agentic integrations are **part of the process**, **on by default**, and they **frame how you work** (lease, index, graph, session-bound commit, meaning-accept). Newcomers are not staring at an empty wiki plus “set up an agent.” A lot of people **never use Notion Agent** because it has to be set up first (and paid for). Same for Confluence and Linear: AI is optional chrome on a human tool. That gap is a **strong product difference**, not a pricing footnote.

They are not “connected to the doc.” They **are** the environment: index, build the semantic graph in the background, write commit comments, help edit the wiki under lease ([session-bound commit](#product-feature-session-bound-commit), [graph](#multidimensional-spec-graph)). Venus is **Gonka-native**: Kimi and MiniMax tokens ship **in the subscription** — how the default process actually runs, not the category. Do not lead with “cheap tokens.” Do not sell a cheaper Notion AI. Category: [above](#category-the-way-of-work-then-the-tool). Unique feature: [unique-features.md](./unique-features.md) (LLM wiki framework). Marketing: [comparisons.md](../marketing/comparisons.md) (Versus Notion, Versus Confluence, Versus PM tools), [pitch.md](../marketing/pitch.md).

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

**Accept is on the rendered page.** Cursor is good at markdown **source**. Its preview has **no diff**; commit marks live only in source. Humans understand the spec as WYSIWYG. Meaning-accept is After / Before / Diff on that page ([pains §14](./pains.md#14-cursor-docs-preview-has-no-diff)). M6. Do not ship a better markdown preview inside Cursor.

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

Venus is not only a wiki app. It is a **spec-first development flow** — the agentic-era replacement for Scrum as the binding OS, and for **Jira+Confluence** as the development pair. The app enforces the short loop (lease, git, skip, merge check). Cursor stays the inner loop. Ops Notion and intake tickets can remain surfaces. They are not the pulse.

## Who needs the flow (including small teams, Figma, Notion)

**Not only people who already keep an LLM wiki.** Every team that still runs Scrum/Jira+Confluence (or Linear + Notion) while agents implement needs a spec-first OS. The wiki-in-`docs/` shop is the easy yes; the **default agile shop** is the market.

Every team that ships with agents needs a spec-first loop. Small teams need it **more**: fewer humans to notice drift, more agent output per person. “Intention lives in one head” dies the first week a composer implements from chat.

Figma is canvas, not a spec agents can clone and not silently rewrite. **Technical design specs leave Confluence.** Company ops wiki (HR, runbooks, Notion databases) can stay. Jira/Linear as **intake** (sales, triage, “fix the button”) can stay. None of that is “human accepted the meaning of the spec, then Cursor ran, then the spec caught up.” They should not treat the ticket or the ops page as the artifact agents must not drift from.

What they may refuse is a **heavy second wiki** *and* a second board. Confluence already is the heavy wiki. Venus is **lightweight** (Notion-like chrome) and replaces **Confluence for technical design** plus Jira as the development heart. If Venus is “replace Notion and Figma” or “a worse Jira” or “another Confluence,” they will not — and should not. Sell the **framework** first; the app is how you run it. Do not sell Venus as the company home screen. Sell it as the OS for agentic software development.

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

The same rule kills the **Word / Google Docs → ChatGPT → paste back** hop and the **import to markdown, shape in Cursor, nobody else can edit** hop ([pains §11](./pains.md#11-clipboard-import-export)). WYSIWYG is how those people write the spec. Git markdown is the projection, not a private Cursor file. **Share use:** commit in Cursor → team WYSIWYG → pull back ([above](#product-use-share-cursor-stays-the-ide)).

| Direction | How |
|---|---|
| **PM / high-level eng / CTO, prose / aims / narrative** | WYSIWYG → snapshot git. No ceremony. |
| **Contract** (behavior, API, DoD, must-clauses) | Human or agent: lease + meaning-accept. |
| **Agent** | Always lease. Never live CRDT. |
| **Programmer / implementer** | Read accepted **wiki** git. May **write spec markdown in Cursor** and share through Venus (commit → team WYSIWYG → pull). Product writes: Venus kicks **Cursor CLI** (or SDK) on the product repo → branch / PR. Spec **contract** still through Venus. **IDE is optional** (debug, hand-edit, this share loop). |
| **Aider / CodeGraph CLI** | **Review only:** the agentic-loop **PR or branch** at `productSha`, **against the bound spec/plan**. Not a bug-hunt dump. **Not implementation.** Does not publish spec. **Select** before AB5. |

v1: clone-and-PR import of wiki markdown is later ([MDGate](../design/MDGate/README.md)). Until the adapter gate is green, do not tell agents “edit the `.md` in git and it will apply.” They acquire a lease (MCP or UI) and put hunks; accept still lands as a wiki commit the next `git pull` sees.

Wire the two remotes (or submodules) into the runner and MCP **before** implement starts. Design: [code-bind](../design/Agents/code-bind.md).

## Workspace and Aider

A Venus workspace **binds** the wiki remote to a Venus-controlled product (separate repo + branch). Bind object: `{ wikiSha, productSha, productBranch }`. Notion’s GitHub connector dumps sources into chat with no clock. Venus only packs code when the recipe asks, at the **pinned** product SHA.

**Aider and CodeGraph CLI are for agentic-loop PR (or branch) review, not for implementation.** **Cursor CLI** (the named implementer) writes the product branch and opens the PR — Venus kicks that job; the IDE is not required. The runner then kicks the selected analyzer on **that PR or branch**. They do not `git commit` product code, do not replace Cursor CLI, and do not type the wiki.

### Spec-bound review

This is a **product feature**, not “we also run CodeGraph.” Same as the [LLM-wiki framework](#agentic-native-user-to-agent): **already automated and set up**. You do not install a GitHub review bot or wire CodeGraph yourself. Notion and Cursor both lack this bind ([comparisons](../marketing/comparisons.md#spec-bound-pr-review)).

**Landed feature bound to plan, plan bound to docs.** That chain is the review working set.

**CodeGraph CLI enriches the PR’s context** (callers, impact, `implements` at `productSha'`). System prompts then validate the same PR from **several sides** — security, performance, product design, code style, and the rest — **and** whether the PR **meets the spec**, **and** whether it brings **related new documentation** that should ship in the **wiki** (or Hugo, when that surface exists). Review **flags** missing docs; it does **not** publish. Agents still cannot type the live wiki. Hugo stays cut from the v1 **loop bar**; the **check** is in the story from day one.

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
| Security / performance / design / style | Hope a human remembered the checklist | **System prompts** on those lenses, on the enriched pack |
| Did we **follow the spec**? | Hope someone `@`’d the right README | Headings the plan was accepted against, at `wikiSha` |
| How **close to the goal**? | Ticket title, or the chat that kicked implement | The plan step’s named outcome, bound to those docs |
| Should related **docs** ship (wiki / Hugo)? | “Update Confluence later” | Flag: new behavior with no spec/wiki (or Hugo) page to publish |

`implements` (heading → path/symbol at `productSha`) is how the landed feature stays tied to the clause. Drift is a **contradiction for a human** (another implement pass or a spec lease) — not CodeSpeak (update the spec from HEAD) and not Notion (agent types the page).

**Select CodeGraph CLI, Aider, or both** before that kick. Lean default (**both, split jobs**): [code-bind — select](../design/Agents/code-bind.md#select-codegraph-cli-or-aider-or-both).

| | CodeGraph CLI | Aider |
|---|---|---|
| Job on the **PR / branch** | Queryable graph at `productSha` (callers, impact, `implements`); **enriches PR context** for the pack | LLM **review comments** on that diff against the same pack — **lenses** (security, performance, product design, style) **plus spec plus docs-to-ship**; repo map if CodeGraph is off |
| Implementation | **No** | **No** |

Neither replaces GitHub human review or meaning-accept. v1: always **run** the selected reviewer(s) after a branch/PR exists; flags are a **warning** (same softness as two-agent DoD). Later the step may fail. Merge check stays spec current or skip — not “Aider approved” and not “close enough to the spec.”

Do not enrich every wiki request with the codebase. Do not delay M5 for this. After [M4](../design/venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks) **record** the remotes; **select** the reviewer; the kick is on the runner ([AB5](../design/Agents/agentic-binding.md#ab5--code-bind--aider)). Design: [code-bind — automated review](../design/Agents/code-bind.md#automated-review-after-implement).

## Already designed — ship the spine

Without these, orchestration is theater. They are [implementation plan](../design/venus-implementation-plan.md) M2 → **M3.0** → M3–M8, not this file’s invention. Rebuild the collab front (**hub**) **before** git snapshotter.

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
- Step: `implement (Cursor CLI) → PR or branch → Aider / CodeGraph **spec-bound** review (that PR/branch; CodeGraph-enriched context; lenses + spec + docs-to-ship) → tests → breakpoint → done`. Runner kicks **external** CLIs. It does not write code. **Cursor CLI implements** (`agent -p` / SDK). **IDE is optional.** Review pack is **landed ↔ plan ↔ docs**, not a repo dump ([code-bind](../design/Agents/code-bind.md#automated-review-after-implement)). **Select** the reviewer CLIs first. Review is **on by default** in this loop — not a GitHub Action the team forgot.
- Overlapping spec leases serialize.
- Story cannot go `done` without spec accept **if contract moved**. **Skip** if it did not.
- **Merge status check:** GitHub (or equivalent) is red until Venus says spec is current or skipped. Without this, the loop is social and Linear wins.

### 3. Human inbox of meaning-accepts

The team surface is not the page editor. It is a **short** queue — not plan + DoD + spec as three required clicks on every story:

- Spec (contract) moved — understand what the wiki now says.
- Plan ready — only if someone opened a plan.
- DoD ready — v1 optional / warn.

Each item is description + hunks as evidence. Review those hunks on **WYSIWYG** After / Before / Diff — not source-only marks ([pains §14](./pains.md#14-cursor-docs-preview-has-no-diff)). Header chrome: current plan, who holds the lease, next gate, link to PR. Not cycles, points, or kanban. If accept is buried in a wiki tab, people rubber-stamp or skip.

### 4. Agent roles as accepted wiki

Planner / DoD / implementer / autodoc are jobs with ids on the plan. Instructions for those jobs are spec pages (same lease/accept). Orchestration is who may write which lease — not a Notion teammate with Slack triggers.

### 5. Freeze as a team feature

Heartbeat, steal, expiry — already in [lease-freeze-rationale](../design/lease-freeze-rationale.md); they are product, not polish. Show who holds what, for how long. Whole-page freeze is the **current** design (Cursor-like for the prompter); extend later (section/folder leases) once the loop is boring.

### 6. Close the git hop

- Agents read accepted git from the workspace checkout; humans open Venus **at gates**.
- PR URL on the yaml; breakpoint is GitHub + Venus notify, not a second review of the same hunks.
- “Open this plan in Cursor” is a **CLI kick** (clone/SHA + `agent -p` / SDK) on the product branch, not a required IDE window.
- After Cursor CLI implements, kick Aider / CodeGraph CLI on that **PR or branch** — **spec-bound review** (landed ↔ plan ↔ docs), not bug-hunt only ([Workspace and Aider](#workspace-and-aider)).

### 7. Session-bound commit

After a brainstorm or other **live session**, a background agent sees activity and unites it into a **commit bound to that session** — a document diff of what was achieved or consensused. Not idle snapshot autocomment. Not Notion page history. Agent drafts under lease; does not type the live room. Contract still meaning-accept. Detail: [above](#product-feature-session-bound-commit).

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
| Brainstorm as Notion Agent (session teammate types the live page) | The call is then a worse Notion. Structure the session via lease; humans still type the room. |
| “Cheaper Notion AI” / lead with Kimi tokens | Category is spec-driven development. Included Gonka runtime is how newcomers get first-class agents, not the product. |
| Empty wiki until they paste an OpenAI key | Then agents are a connector, not citizens. BYO model may exist later; it is not day-one. |
| Idle snapshot autocomment as “what the call decided” | Flush is not consensus. Session-bound commit is the artifact. |
| Aider or CodeGraph dumped into every wiki ask | Notion connector. Pack code only on recipe, at `productSha`. |
| Aider or CodeGraph as **implementer** | Cursor writes the branch/PR. These CLIs **review** that PR or branch **against the spec**. Selling them as coding agents kills the loop. |
| CodeGraph / Aider as a GitHub bug-hunt bot | Without **landed ↔ plan ↔ docs**, it is Cursor’s review. Notion and Cursor already do mistakes. The bind is the feature. |
| Review that **publishes** wiki or Hugo | Review **flags** docs that should ship. Meaning-accept still publishes spec. Hugo still after the loop bar. |
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
