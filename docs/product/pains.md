# Pains Venus closes

Product problem list — not the pitch, not the milestone plan. What is broken in the world that Venus is for. Goal and keep/cut: [product-plan.md](./product-plan.md).

Two loads: **Notion is not an agent wiki**, and **Notion-like spec management is not spec-driven development**. A third: **you cannot recover why a design is this way** — history is a blob, chat is gone, the clause is silent. The temporal graph plus comments is meant to close that. In the agentic era (from 2026), spec-driven development is the framework that actually binds the work. Scrum, Agile-as-ceremony, and ticket-first process are leftover human choreography; they do not tell an agent what must not drift.

## 1. CRDT wiki vs markdown folders (the gap)

**Pain:** Docs for humans and docs for agents are different objects.

CRDT products (Notion, AFFiNE, BlockSuite live) were built so **people** can type the same page at once. That is a bad agent interface: no clone, no grep, no PR, a proprietary graph, live writes. Agents that write software work on **files and git**.

Notion as the spec for agentic development **does not fit**. Export is a dump. Custom Agents type the live graph. The spec is not folders of `.md`. Developers already leave Notion and put truth in the repo — or they implement from a stale page.

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

Scrum/Agile (standups, sprints, story points, a backlog of tickets) optimized **human coordination**. From 2026 the scarce thing is not “did we meet” — it is **did the agent implement the world we meant**. Tickets and ceremonies do not survive contact with a composer: the model reads whatever is in context (chat, a Notion dump, last week’s ticket). The most powerful framework is **spec-driven**: shared accepted markdown, short loop, humans keep intention, agents do the work.

Venus is that framework as a product, not a worse Jira and not a standup tool. Intake and roadmap can stay in a PM tool. If people live on the board, Scrum won and the spec died.

## 4. Three workspaces, three specs

**Pain:** PMs write in Notion. Programmers write (or ignore) spec in git. Agents write in Cursor chat or the live wiki. Drift is the default.

**Venus:** one spec; [prose vs contract](./product-plan.md#shared-git-not-copy-paste); agents always lease; programmers pull git; PMs never need Cursor. The implementer job is **Cursor CLI**; the IDE is optional.

## 5. Done means the ticket closed

**Pain:** Linear/Jira “Done” while the spec still describes the old world. Agents merge a story; the wiki is leftover.

**Venus:** if contract moved, not done until meaning-accept. If it did not, skip. Status check on merge — or the invariant is a lecture.

## 6. Chat is not a spec

**Pain:** Intent lives in agent transcripts. Requirements are extracted after the fact (CodeSpeak’s object) or lost. Next session has no cloneable heart.

**Venus:** the spec is a document you can read front to back, in git, accepted by a human. Chat may draft; it must not become the record.

## 7. Why is it designed this way

**Pain:** A spec page says *what*. Almost nothing says *why it is this way* **on the clause**. New PMs, new agents, and you-in-six-months re-litigate freeze, lease, `T0`. You cannot ask an LLM for design evolution. You cannot find the **bad decision** to roll back — only a history slider or a dead chat.

Where the why goes today, and dies:

| Place | Why it fails |
|---|---|
| Notion version history / activity | A **blob**. Undo the page. Not “this accept, this comment, this heading.” |
| Linear / Jira ticket | Another object. Spec paragraph does not point at it. Ticket closed; rationale gone. |
| Cursor (or any) chat | Why is in the **thread**. Next session, next person, next model: empty. |
| `git log` on snapshots | Autocomment `snapshot: <title>` — *what* moved, not why. |
| The paragraph itself | Silent. No bind to the decision that produced it. |

This is a **wiki-design** pain, not a missing chatbot. Agents that implement from the clone re-invent policy. Humans argue the current text as if it had no past.

**Venus:** comment-commit **why** and review **comments** are pinned to hunks (sidecar ids). The life index is **spatial + temporal**: what else this clause binds to, **and** the explained diffs that made it so (`decided-in`, `supersedes`, `reverts`). Bound chat packs both when you point at the heading. Revert is still a lease ([M8](../design/venus-implementation-plan.md#m8--revert--agent-loop-week)), with a **named SHA**, not a live undo.

The timegraph **with comments** is what closes the pain. Spatial binds alone answer “what else is in force.” Without the temporal axis you still cannot understand the design. Snapshot autocomments are not why — if contract never takes comment-commit, this pain stays open.

Detail: [product-plan — graph](./product-plan.md#multidimensional-spec-graph), [agentic-comparison](../marketing/agentic-comparison.md#how-the-multidimensional-graph-compares).

## 8. Agents bolted onto human tools

**Pain:** The default “AI product” is a human tool with an agent **adopted** onto it — copilot in Notion, agent in Linear, bot that types the same page the PM types. The environment is still human-native. The model is a guest in someone else’s UI. You get a slower, worse version of a product that already won (the wiki, the board).

**Venus:** the environment is **agentic-native**. Lease, git markdown, Bind pack, hunks, meaning-accept exist so agents can work. Human chrome is **user-to-agent**: point at a clause, accept why, skip when spec did not move. WYSIWYG is for humans to write; it is not what the agent drives. Detail: [product-plan — Agentic-native](./product-plan.md#agentic-native-user-to-agent).

## What Venus does not claim to close

- Figma as visual spec (canvas stays canvas).
- Ops wiki, databases, Slack routing (Notion can keep those).
- Inner-loop **implementation** (Cursor already won). Aider / CodeGraph CLI are **PR or branch review** in the Venus loop, not a second implementer ([product-plan](./product-plan.md#workspace-and-aider)).
- “Only we have markdown in folders” — git already does. We close **that plus** a human wiki **plus** agents cannot publish **plus** SDD as the loop, in one object.
