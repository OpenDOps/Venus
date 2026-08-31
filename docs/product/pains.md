# Pains Venus closes

Product problem list — not the pitch, not the milestone plan. What is broken in the world that Venus is for. Goal and keep/cut: [product-plan.md](./product-plan.md). Unique join: [unique-features.md](./unique-features.md).

Two loads: **Notion is not an agent wiki**, and **Notion-like spec management is not spec-driven development**. The **category** is the successor to Scrum: spec-driven development in the agentic era; Venus is the tool (Jira+Confluence for that work). A third: **you cannot recover why a design is this way** — history is a blob, chat is gone, the clause is silent. A fourth: **PR review does not know the spec**. A fifth: **the spec was born in one engineer’s IDE** — other departments never met on a document that can become software. A sixth: **the hop is copy-paste** — import to markdown, shape in Cursor, and people who do not use Cursor or markdown cannot edit; or Word / Google Docs into ChatGPT and back. **Largest for non-Notion shops:** Confluence is the usual technical-design wiki and is not lightweight like Notion. A seventh: **the live call left no measurable artifact** — no session-bound spec diff of what was consensused. An eighth: **agents never get set up** — Notion / Confluence / Linear have AI as optional chrome; most people never turn it on. A ninth: **Cursor is good at markdown docs, bad at accepting them** — the diff is only in source; preview has no marks; you flip preview/source to read and to accept. Venus is an LLM-wiki **framework** with the agentic process on by default. Brainstorming on the collab wiki, then agents fitting LLM-wiki structure, is meant to close that. In the agentic era (from 2026), spec-driven development is the framework that actually binds the work. Scrum, Agile-as-ceremony, and ticket-first process are leftover human choreography; they do not tell an agent what must not drift.

## 1. CRDT wiki vs markdown folders (the gap)

**Pain:** Docs for humans and docs for agents are different objects.

CRDT products (Notion, AFFiNE, BlockSuite live) were built so **people** can type the same page at once. That is a bad agent interface: no clone, no grep, no PR, a proprietary graph, live writes. Agents that write software work on **files and git**.

Notion as the spec for agentic development **does not fit**. Export is a dump. Custom Agents type the live graph. The spec is not folders of `.md`. Developers already leave Notion and put truth in the repo — or they implement from a stale page.

**Confluence is the usual technical-design wiki** (next to Jira) and is worse for agents than Notion: not lightweight collab, macros, a dump if you want markdown. That shop lives the clipboard hop ([§11](#11-clipboard-import-export)).

**Venus:** humans keep a collab page. Agents and developers get the **same** spec as a git tree of markdown. MDGate makes those one spec. Do not teach agents Yjs.

Detail: [product-plan — The gap](./product-plan.md#the-gap-crdt-is-for-humans-folders-of-markdown-are-for-agents).

Git `docs/` already closes the **agent** half. The remaining pain is PMs who will not live in git, plus agents that must not silently publish. Venus is for that join, not for inventing files.

## 2. Spec management is not spec-driven development

**Pain:** Teams store product spec in a Notion-like wiki (or a ticket description) and call that “spec-driven.” It is **document storage**. Closing the card, updating the page, or letting an agent rewrite the wiki is not a development loop.

Spec-driven development in the agentic era: the **accepted spec** is what agents must not drift from; a story is not done until that spec is current (or explicitly skipped); humans bless **meaning** when contract moves. The wiki is the heart. The board, the sprint, and the chat are not.

Notion-like spec management fails that:

| What they do | Why it is not SDD |
|---|---|
| Spec lives in a page, work lives in Linear/Jira | Two objects. Done = ticket status. Spec optional, usually stale. |
| Agent edits the live page | The model becomes the record. |
| Version history / activity log | Undo a blob. Not “we accepted this meaning.” |
| Sprint, standup, points | Human ceremony. Agents do not pull from a sprint; they pull from nearest text. |
| PR merges, wiki “later” | Intention never catches up. Next agent reads a lie. |

**Venus:** one spec, file-shaped for agents, collab for PMs, meaning-accept on contract change, skip when spec did not move, merge check so GitHub cannot pretend the story is done. Loop must stay [shorter than Notion + a PR](./product-plan.md#product-goal).

## 3. Agentic era vs Scrum / Agile-as-ceremony

**Pain:** Process from a human-only era is the default OS for teams that now ship with agents. It is outdated **as the binding constraint**, not as a calendar.

Scrum/Agile (standups, sprints, story points, a backlog of tickets) optimized **human coordination**. From 2026 the scarce thing is not “did we meet” — it is **did the agent implement the world we meant**. Tickets and ceremonies do not survive contact with a composer: the model reads whatever is in context (chat, a Confluence dump, last week’s ticket). The most powerful framework is **spec-driven**: shared accepted markdown, short loop, humans keep intention, agents do the work.

**Promote that framework first.** Venus is the **tool for it** — not “an LLM wiki for people who already have one,” and not a worse Jira. The agile-era pair was **Jira + Confluence** (ticket + wiki). Venus replaces **that pair for software development**. For **technical design specifications**, the wiki it replaces is **Confluence** — not as lightweight as Notion (macros, spaces, dump-to-markdown). Intake and sales tickets can stay in Jira. Company ops wiki can stay leftover Confluence or Notion. How you **design and ship the product** does not.

If people live on the board, Scrum won and the spec died. Detail: [product-plan — Category](./product-plan.md#category-the-way-of-work-then-the-tool). Non-Notion shops feel the clipboard hop harder ([§11](#11-clipboard-import-export)).

## 4. Three workspaces, three specs

**Pain:** PMs write in **Confluence** (the default technical-design wiki next to Jira) or Notion. Programmers write (or ignore) spec in git. Agents write in Cursor chat or the live wiki. Drift is the default. The hop between those rooms is usually **clipboard** ([§11](#11-clipboard-import-export)).

**Venus:** one spec; [prose vs contract](./product-plan.md#shared-git-not-copy-paste); agents always lease; implementers pull git; PMs, high-level engineers, and CTOs never need Cursor. The implementer job is **Cursor CLI**; the IDE is optional.

## 5. Done means the ticket closed

**Pain:** Linear/Jira “Done” while the spec still describes the old world. Agents merge a story; the wiki is leftover.

**Venus:** if contract moved, not done until meaning-accept. If it did not, skip. Status check on merge — or the invariant is a lecture. **PR review is spec-bound** (landed ↔ plan ↔ docs): not “the ticket closed and the diff looks fine” ([product-plan — spec-bound review](./product-plan.md#spec-bound-review)).

## 6. Chat is not a spec

**Pain:** Intent lives in agent transcripts. Requirements are extracted after the fact (CodeSpeak’s object) or lost. Next session has no cloneable heart. The cheap version is the same hop without git: paste a Word or Google Doc into ChatGPT, paste the answer back ([§11](#11-clipboard-import-export)).

**Venus:** the spec is a document you can read front to back, in git, accepted by a human. Chat may draft; it must not become the record.

## 7. Why is it designed this way

**Pain:** A spec page says *what*. Almost nothing says *why it is this way* **on the clause**. New PMs, new agents, and you-in-six-months re-litigate freeze, lease, `T0`. You cannot ask an LLM for design evolution. You cannot find the **bad decision** to roll back — only a history slider or a dead chat.

Where the why goes today, and dies:

| Place | Why it fails |
|---|---|
| Confluence page history | A **blob**. Undo the page. Macros, not a clause. |
| Notion version history / activity | A **blob**. Undo the page. Not “this accept, this comment, this heading.” |
| Linear / Jira ticket | Another object. Spec paragraph does not point at it. Ticket closed; rationale gone. |
| Cursor (or any) chat | Why is in the **thread**. Next session, next person, next model: empty. |
| `git log` on snapshots | Autocomment `snapshot: <title>` — *what* moved, not why. |
| The paragraph itself | Silent. No bind to the decision that produced it. |

This is a **wiki-design** pain, not a missing chatbot. Agents that implement from the clone re-invent policy. Humans argue the current text as if it had no past.

**Venus:** comment-commit **why** and review **comments** are pinned to hunks (sidecar ids). The life index is **spatial + temporal**: what else this clause binds to, **and** the explained diffs that made it so (`decided-in`, `supersedes`, `reverts`). Bound chat packs both when you point at the heading. Revert is still a lease ([M8](../design/venus-implementation-plan.md#m8--revert--agent-loop-week)), with a **named SHA**, not a live undo.

The timegraph **with comments** is what closes the pain (**AB4**, after **M6** — not after AB1). Spatial binds alone answer “what else is in force.” Without the temporal axis you still cannot understand the design. Snapshot autocomments are not why — if contract never takes comment-commit, this pain stays open.

Detail: [product-plan — graph](./product-plan.md#multidimensional-spec-graph), [agentic-comparison](../marketing/agentic-comparison.md#how-the-multidimensional-graph-compares).

## 8. Agents bolted onto human tools

**Pain:** The default “AI product” is a human tool with an agent **adopted** onto it — copilot in Notion, agent in Linear, bot that types the same page the PM types. The environment is still human-native. The model is a guest in someone else’s UI. You get a slower, worse version of a product that already won (the wiki, the board).

The same guest is usually an **extra SKU, a pasted API key, or a setup wizard**. A lot of people **never use Notion Agent** because it has to be set up first. Linear and Confluence AI are the same: optional, after the human tool. The tool you bought is not agentic; the connector is.

**Venus:** the environment is **agentic-native** — a **framework for an LLM wiki**. Agentic integrations are **part of the process**, **on by default**, and they **frame how you work**. Lease, git markdown, Bind pack, hunks, meaning-accept exist so agents can work. Human chrome is **user-to-agent**: point at a clause, accept why, skip when spec did not move. WYSIWYG is for humans to write; it is not what the agent drives. Agents **index, graph, comment, edit under lease** as jobs in that OS. Newcomers get that **in the subscription** (Gonka-native: Kimi, MiniMax). Detail: [product-plan — Agentic-native](./product-plan.md#agentic-native-user-to-agent). Pain: [§13](#13-agents-never-get-set-up).

## 9. PR review does not know the spec

**Pain:** GitHub review (and Cursor’s own review) hunts **mistakes** — bugs, style, blast radius. You must **set up** the bot, the CodeGraph Action, the checklist. Most teams never do, or they get a generic linter dump. Notion has no product PR; a GitHub connector dumps sources into chat. The landed feature is not bound to the plan; the plan is not bound to the docs. Nobody is forced to ask: did we follow the specification? Did this change need **new wiki (or Hugo) docs**? How close are we to the goal?

**Venus:** PR review is **already in the process**, like the rest of the LLM-wiki framework. **CodeGraph CLI enriches the PR’s context.** System prompts validate from **several sides** (security, performance, product design, code style, …) **and** against the **spec** **and** whether related documentation should ship in the wiki (or Hugo). Bound pack: landed feature ↔ plan step ↔ accepted documentation. Analyzer does not implement or publish. Detail: [product-plan — spec-bound review](./product-plan.md#spec-bound-review), [comparisons](../marketing/comparisons.md#spec-bound-pr-review).

## 10. The spec was born in one person’s IDE

**Pain:** An “LLM wiki” is usually one engineer writing `docs/` in Cursor (or dumping a Notion export). Product, marketing, and project were never in that file. The call happened in a deck or a Miro board. Departments do not meet on a specification that can become software.

**Venus:** a **team call** on collaborative docs with a **Notion-like UI** so product and marketing actually show up (no IDE learning curve). During or after, agents are aimed to fit that session into LLM-wiki structure (tree, product and marketing goals as spec pages). Humans accept. The agent does not type the live page. All directions — product, project, marketing, development — adjust the end-product shape on **one** spec. The call leaves a **session-bound commit** — a measurable diff of what was consensused ([§12](#12-the-call-left-no-measurable-artifact), [product-plan — session-bound commit](./product-plan.md#product-feature-session-bound-commit)). Detail: [product-plan — brainstorming](./product-plan.md#product-use-brainstorming), [unique-features](./unique-features.md). People who **keep Cursor** still share through Venus: commit, team WYSIWYG, pull ([product-plan — share](./product-plan.md#product-use-share-cursor-stays-the-ide), [§11](#11-clipboard-import-export)).

## 11. Clipboard, import, export

**Pain:** The spec hop is copy-paste. You import or copy a doc into markdown, shape it with an LLM in Cursor, and then **people who do not use Cursor or markdown cannot edit it**. Next round: export, paste, import again. Some people never reach git: **Word or Google Docs → paste into ChatGPT → paste back into the page**. The document and the model never share a place. The engineer who can use the IDE owns the only copy that is still “for agents.”

This is a **large problem for non-Notion users.** Notion is already lightweight collab (wrong object for a spec, but people can type together). **Confluence is not.** It is the usual home of **technical design specifications** next to Jira: heavy (spaces, macros, permissions), a dump to markdown, not a room you brainstorm in. Those teams live the roundtrip: Confluence / Word / Google Docs → chat → `docs/` in Cursor → nobody else can edit.

That is the daily work of an LLM wiki without a product. Import is a dump. Export is a dump. Chat is a clipboard with a model. Shaping in Cursor is a **one-way door**.

**Venus:** replaces **Confluence for technical design specs** with one page both audiences type. Chrome is **Notion-like on purpose** (the lightness Confluence lacks). WYSIWYG for people who will not live in markdown or an IDE. Git markdown is the **same** spec (clocked projection), not an export you reshape in Cursor and cannot give back.

**Second use:** people may **keep Cursor as the IDE**. Venus is the share place: you commit, the team sees WYSIWYG and can change it, you pull back ([product-plan — share](./product-plan.md#product-use-share-cursor-stays-the-ide)). Agents fit structure **under lease** in that wiki; they are not the paste buffer between Confluence and `docs/`. Humans and agents do not hop spec through chat or the clipboard. Word, Google Docs, and leftover Confluence can stay for letters and company ops; they are not the spec agents implement.

Do not ship “better import from Word / Confluence” as the product. The product is **stop needing the roundtrip**. Dual store: [unique-features](./unique-features.md). Gap: [§1](#1-crdt-wiki-vs-markdown-folders-the-gap). Comparison: [comparisons — Confluence](../marketing/comparisons.md).

## 12. The call left no measurable artifact

**Pain:** You had a live brainstorm (or any design call). Afterwards nobody can point at **what that call decided**. Notion/Confluence: the page moved; history is a blob. Zoom: a recording and a chat recap. Miro: a board. Idle git snapshot: the wiki flushed sometime. There is no **document diff bound to the session**, so “what we consensused on Tuesday” is folklore.

**Venus:** brainstorms and other live sessions stay a **meaningful artifact**. A background agent sees that there was activity and unites it into a **commit bound to the session**. Everyone reads a measurable spec diff: what was achieved on that call. Humans still type the room; the agent does not become the record by typing live. Contract still meaning-accept. Idle snapshot autocomment is not this pain’s close. Detail: [product-plan — session-bound commit](./product-plan.md#product-feature-session-bound-commit), [unique-features](./unique-features.md).

## 13. Agents never get set up

**Pain:** Notion, Confluence, and Linear **have** AI. Most people do not use it. Agents are extra payment, a connector, or a setup step **after** you already live in the human tool. The wiki or the board stays the product; the agent stays optional. “We have Notion Agent” is not “we work agentically.”

**Venus:** a **framework for an LLM wiki**. Agentic integrations are **already the process** — default, not a wizard. They **frame how you work** (lease, accept, graph, session-bound commit). **Gonka-native**; Kimi and MiniMax tokens are **in the subscription** so that process runs on day one. That is a **strong product difference** vs Notion / Confluence / Linear, not a cheaper AI SKU. Do not confuse this with “we are an LLM vendor.” Detail: [product-plan — Agentic-native](./product-plan.md#agentic-native-user-to-agent), [unique-features](./unique-features.md) (§13).

## 14. Cursor docs: preview has no diff

**Pain:** Cursor is **excellent with markdown**. The worst part is **docs**, not code. The markdown preview does **not show the diff**. Changes and accept marks live only in **source**. To understand the document you switch to preview; to accept the commit you switch back to source. Humans read a **rendered page**. Cursor makes you bless a **source hunk**. Flip, flip, flip.

That is the daily pain of “LLM wiki in Cursor.” Writing `.md` is solved. **Reviewing and accepting** the page as a page is not.

**Venus:** meaning-accept on **WYSIWYG After / Before / Diff** — the same rendered page the team reads, with hunks overlaid. You do not accept markdown syntax; you accept **what the spec now says**, with the description in human language. Source markdown remains the agent/git form (dual store). Cursor stays the **code** IDE (and optional markdown write). It does not have to be the spec-accept UI. Design: [MDGate apply](../design/MDGate/apply.md) (humans review on WYSIWYG After/Before). Shipped vs story: **M6**. Unique feature: [unique-features](./unique-features.md) (§4). Comparison: [comparisons — Cursor](../marketing/comparisons.md).

## What Venus does not claim to close

- Figma as visual spec (canvas stays canvas).
- Ops wiki, databases, Slack routing (Notion can keep those). Leftover **Confluence as company intranet** can stay. **Technical design specs** do not.
- Inner-loop **implementation** (Cursor already won). Cursor also already won **writing markdown**. It did **not** win **accepting a rendered doc** ([§14](#14-cursor-docs-preview-has-no-diff)). Aider / CodeGraph CLI are **spec-bound PR review** in the Venus loop (landed ↔ plan ↔ docs), not a second implementer ([product-plan](./product-plan.md#spec-bound-review)).
- “Only we have markdown in folders” — git already does. We close **that plus** a human wiki **plus** agents cannot publish **plus** SDD as the loop, in one object.
