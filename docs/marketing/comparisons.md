# Venus comparisons

Pitch: [pitch.md](./pitch.md). Design: [venus-design.md](../design/venus-design.md), [venus-plan.md](../drafts/pre-design/venus-plan.md). Product: [product-plan](../product/product-plan.md), [pains](../product/pains.md), [unique features](../product/unique-features.md). Bound chat vs Notion Agent / Cursor **ask**: [agentic-comparison.md](./agentic-comparison.md). **Spec-bound PR review** (landed ↔ plan ↔ docs; Notion and Cursor lack it): [below](#spec-bound-pr-review).

## Versus PM tools (Linear, Jira, Plane, GitHub Issues)

**Category first:** spec-driven development is the successor to Scrum/Agile-as-ceremony. Venus is the **tool of that method** ([product-plan — Category](../product/product-plan.md#category-the-way-of-work-then-the-tool)). The market is every team still running **agile-era process** while agents write code — not only shops that already keep an LLM wiki in `docs/`.

**Jira + Confluence was the agile pair** (tickets + wiki). Venus **replaces that pair for software development**: spec and accept are the heart; the board is a mirror or gone. For **technical design specifications**, Venus **replaces Confluence** ([below](#versus-confluence)). It does not replace Jira for sales/triage, and it does not replace leftover Confluence or Notion as the company **ops** wiki. It replaces them as **how you design and ship the product**.

If Venus looks like a PM tool, Linear wins in a week. Those products are first-class **work items**: states, assignees, cycles, views. Spec-driven development there is a description field or a link. An agent reads the ticket, opens a PR, and **done means the issue closed**. The spec is optional and usually stale.

Venus inverts that. **The spec is first-class; the board is a mirror.** Agentic spec-driven development is the product loop, not a Linear integration:

| | Linear / Jira / Plane / Issues | Venus |
|---|---|---|
| Unit of work | Issue / work item | Spec page (and a plan derived from it) |
| What the agent must not drift from | Ticket title + description | Published spec git + leased drafts |
| Plan / DoD | Checklist on the issue, written by whoever | Plan then **other agent** DoD, both human-accepted wiki leases |
| Parallel humans on the spec | Comments on the ticket | Live CRDT; source edits freeze under a lease |
| Story complete | Status = Done | Spec (and aims/API) **diff accepted by a human** |
| PR vs spec | Review hunts bugs; ticket is a link if someone pasted it | **Landed ↔ plan ↔ docs.** Validate code against the spec ([spec-bound PR review](#spec-bound-pr-review)) |
| Brainstorm / who writes the spec | Ticket comments, or a meeting that never becomes spec | **Team call** on collab docs; agents fit LLM-wiki structure; all departments on one spec |
| Board | The product | `*.state.yaml` + plan page; Issues/Plane optional mirrors |
| Wiki (Confluence for technical design) | Page next to the ticket; dump for agents; **not lightweight** | Same spec as git markdown; meaning-accept; leftover Confluence can stay **ops** |
| Agents | Optional AI on the board. **Setup. Most never use it.** | **LLM-wiki framework.** Agentic process **on by default**; it **frames the work** |

Use a PM tool for intake, roadmap, and work that is not a spec (triage, sales, “fix the button”). The **spec-first loop** is still required: closing a card without accepting the spec is a bug. For **development**, Venus is the replacement for Jira+Confluence — not an integration that leaves the ticket as truth. Linear/Jira can stay the **mirror** (or stay out). Venus is the flow, not a worse board.

Do not build Venus cycles, points, or a kanban as the home screen. If people live on the board, Scrum won. If they live on the spec and the board follows yaml, agentic SDD is first citizen.

## Versus Confluence

**Mostly this:** Venus replaces Confluence for **technical design specifications**. The chrome can look like Notion because Confluence is **not that lightweight** — and that heaviness is why Confluence shops dump pages, paste into ChatGPT, and shape markdown in Cursor ([pains §11](../product/pains.md#11-clipboard-import-export)).

Confluence is the wiki half of Jira. Architecture pages, ADRs, API notes, “how this service works” live there. Export is a dump. Agents cannot clone an honest tree. People who do not use markdown cannot edit the Cursor copy. Notion users already have a light room (wrong object). Confluence users do not.

| | Confluence | Venus |
|---|---|---|
| What it is | Heavy wiki next to Jira (spaces, macros, permissions) | Lightweight collab (Notion-like chrome) + git markdown, one spec |
| Technical design | The usual home. Dump to shape in Cursor. | **The product.** Page **is** the spec agents clone. |
| Who can edit after an LLM pass | Engineer with markdown / IDE | Anyone on the WYSIWYG page |
| Agent | Marketplace apps, paste into chat. **Optional. Most never set it up.** | **Framework default:** lease, graph, session commit, meaning-accept. Not a plugin. |
| Done | Page updated sometime; ticket Done | Spec current or skip |
| Company ops (HR, runbooks) | Fine to keep | Not the job. Do not become a second Confluence. |

Do not clone Confluence (macros, space admin, a slower wiki). Do not sell Venus as the company intranet. Sell it as the spec wiki for software in the agentic era. Pain: [pains.md](../product/pains.md) (§3, §11). Category: [product-plan](../product/product-plan.md).

## Versus Notion (and Notion agents)

Notion is a CRDT (block graph) for humans. It does not fit **docs for agentic development**: agents cannot clone folders of markdown; they type the live workspace. Venus exists to close that gap — LLM wiki as **git markdown**, collab page for humans, same spec.

If Venus looks like Notion with a copilot, Notion still wins at ops. The job Notion cannot do is a spec that hardens Cursor: files in folders, honest export, agents do not publish by typing. Notion **adopts** an agent onto a human workspace. Venus is the other direction: **agentic-native** surfaces, human UI to **aim** them ([product-plan — Agentic-native](../product/product-plan.md#agentic-native-user-to-agent)).

| | Notion / Notion Agent / Custom Agents | Venus |
|---|---|---|
| What the agent is | Teammate inside the workspace | Lease holder at the spec gate |
| Where it writes | Live pages and databases | Private markdown buffer; published page frozen |
| When it becomes truth | On the write (undo later) | On **human accept** of a comment-commit |
| What you review | Activity log + page history (the page moved) | Commented hunks on a frozen `T0` (After / Before / Diff) |
| What you accept | The new page, or undo the blob | Human-language description of the **spec** change (comment-commit) — only once you understand it |
| Share format for agents | Notion graph + connectors | Git folders + `.md` (clone, grep, Hugo) |
| Agent loop | Prompt or 24/7 trigger → mutate the wiki | Prompt → exclusive working set → diff + comments → **accept** |
| Story complete | Workflow finished (row, message, page) | Spec (and aims/API) **diff accepted** |
| DoD | Whoever wrote the page or the ticket | Other agent writes DoD; implementer cannot; human accept |
| Best at | Ops on a living workspace | Spec-driven development, LLM wiki, Cursor-like source writes |
| Brainstorm | The call is the page. Agent may type it. Marketing stays ops. **After: history blob, not “this call’s consensus.”** | **Same Notion-like gestures** (type together) so the room shows up. Then **structured LLM wiki** (lease, not live type). **Session-bound commit** = measurable spec diff of what the call decided. Product + marketing goals are spec pages |
| How you get an agent | **Must set up** (and often **pay extra**). Most people never use Notion Agent. | **Default process.** LLM-wiki **framework**; agentic jobs on by default; they **frame how you work**. Gonka-native (Kimi, MiniMax) in the subscription |
| PR / code vs spec | No product PR. GitHub connector dumps sources into chat | **Landed feature bound to plan, plan bound to docs.** Review validates code against the spec — not only mistakes ([spec-bound PR review](#spec-bound-pr-review)) |

The brainstorming **chrome** is allowed to look like Notion — that is how product and marketing join the call. The **object** must not: cloneable git spec, agents do not publish by typing, meaning-accept, departments stay on that specification. Copy Notion Agent into the familiar UI and you donated the session back. Detail: [product-plan](../product/product-plan.md) (Vs Notion).

Notion Agent is also a **second purchase** and a **setup step**. A lot of people never use it. Venus is an **LLM-wiki framework**: agentic integrations are **the process**, on by default, and they **frame how you work**. Gonka-native Kimi and MiniMax in the subscription is how that runs — not a cheaper Notion AI. Product: [product-plan — Agentic-native](../product/product-plan.md#agentic-native-user-to-agent).

Notion Agent acts as you and edits in place. Custom Agents are named teammates with their own ACL, schedules, and connectors. That is the right product for triage, standups, Q&A, and keeping databases current. It is the wrong default for an LLM wiki whose second author is a model: the agent becomes the record the moment it types. There is no honest git loop, no human-language accept of the spec change, and no rule that the same model cannot write the bar and the work that “satisfies” it. Version history can revert a page; it cannot tell you what you agreed the spec now means, and why.

Use Notion when the wiki **is** the ops workspace (databases, standups, Q&A). Keep Figma for canvas. Those teams still need a **spec-first development loop** once agents implement — Notion-with-agents is not that loop. **Most teams’ technical design wiki is Confluence, not Notion.** Venus replaces **that** for software development; Notion can stay ops. Use Venus as the flow: the artifact agents share with humans **is the spec**, writes are a checkout, a story is not done until a human accepts the **meaning** of that spec change. Notion stays ops; it does not stay the heart.

Do not build Notion databases, Slack routing, or 24/7 report bots. If people live in the live page while an agent patches it, you are a worse Notion. If they live on the spec and agent writes go through the lease, Venus is the LLM wiki; Notion stays the ops surface — or stays out.

**Ask path (not this write table):** selection → graph pack → chat must not feel like Notion-with-a-copilot. Gesture is Cursor add-to-chat. [agentic-comparison.md](./agentic-comparison.md).

## Versus Cursor

Cursor is the loop Venus copies for source: you ask, the working set is exclusive, you wait, you review the diff, you accept. In the **agentic loop** that job is **Cursor CLI** (`agent -p` / SDK), kicked by Venus. The **IDE is optional** — debug, hand-type, local review. Venus does not host Composer. If the loop requires the IDE, you glued the product to a window the CLI already replaced.

The write loop is Cursor’s (CLI). The **accept** is CodeSpeak-shaped, aimed at spec: you are not rubber-stamping markdown hunks the way you accept a code patch. You accept human-language descriptions of what the spec now says and why, when you understand them. Hunks and the rail are how that description stays attached to the change; they are not the thing you are blessing as “the code looks fine.”

Cursor is the **code** implementer (CLI first; IDE optional). Venus is the **spec** wiki that loop must not silently skip — agentic-native for the contract, **user-to-agent** chrome for PMs. Venus **controls the job** (kick CLI, pin SHAs, next gate). It does not own Cursor’s chat as a second spec.

| | Cursor | Venus |
|---|---|---|
| Surface | Product repo (files, PR). **CLI** is enough for the loop; IDE is optional | Spec wiki (CRDT + git markdown) + runner that **kicks** CLI |
| Agent loop | Prompt → dirty buffer → review → accept | Same loop: lease → private markdown → After/Before/Diff → human accept |
| What you accept | Code diff; why is in the chat | Human-language description of the **spec** change, once you understand it (hunks are the evidence) |
| Tracking | Chat + git; why is not on the hunks | Descriptions pinned to hunks; git message is that why. Index packs that timeline with the clause ([agentic-comparison — why](./agentic-comparison.md#why--timeline-main-feature)) |
| Multiplayer during the loop | You and the agent; other files stay yours | Page freeze for everyone on that `docId`; other pages stay live; comment rail stays |
| Story complete | PR merged / tests green | Spec diff **accepted**; PR is the implementer path, not done |
| Spec | README, rules, chat — optional and drift-prone | First-class; agents do not become the record by typing |
| Who writes the spec | One engineer in a **personal IDE** (`docs/`); export is a dump the team cannot edit | **Team call** on collab docs; **or** engineer commits from Cursor → team WYSIWYG → pull back ([product-plan — share](../product/product-plan.md#product-use-share-cursor-stays-the-ide)) |
| PR review | Diff + whatever you `@`’d. Bug-hunt. Spec is optional context | **Landed ↔ plan ↔ docs.** Validates **against the spec**: mistakes, did we follow it, how close to the goal ([spec-bound PR review](#spec-bound-pr-review)) |
| Role | **Implementer** (and local review). Aider / CodeGraph CLI are **not** this column — they review the Venus-loop **PR or branch** | Heart: intention accepted, then Cursor may run; then those CLIs **spec-bound review** |

**Freeze is the current design**, tuned for this Cursor-like loop: one source writer, stable `T0`, then a CodeSpeak-like spec accept. It can be **extended later** (narrower leases, other exclusive modes) once the loop is boring. Do not treat whole-page freeze as the forever product, and do not treat it as a bug. Primary design is: you asked, you wait, you accept **the meaning of the spec change** — Cursor’s checkout, CodeSpeak’s review, on the wiki. The extra wiki cost is only the person who did **not** prompt: they review or they open another page. That is checkout, not Cursor; v1 accepts it.

Use **Cursor CLI** (or SDK) to write product code against a **published** contract on a **shared git checkout** — Venus kicks that. Use the IDE if you want to; the loop does not. **Keeping the IDE for spec markdown is a product use:** commit, team sees WYSIWYG, you pull ([product-plan — share](../product/product-plan.md#product-use-share-cursor-stays-the-ide)). Use Venus when the spec must survive that run. The accept on that spec is not a mute apply.

**Aider / CodeGraph CLI** are kicked in the Venus **agentic loop** to **review the PR or the implement branch against the specification** — not only mistakes: closeness to the plan’s goal, and whether the code followed the docs. They are not a second Cursor and not how Venus implements. Product: [Workspace and Aider](../product/product-plan.md#workspace-and-aider), [spec-bound PR review](#spec-bound-pr-review).

Bound **chat** on the wiki copies Cursor’s **add selection to chat**, not Cursor’s apply. [agentic-comparison.md](./agentic-comparison.md).

**Bar:** the whole loop must stay **shorter than Notion + a PR** ([product-plan](../product/product-plan.md)). If it does not, Venus is AFFiNE + a GitHub workflow, aimed at a Notion user, justified by a Cursor metaphor.

## Spec-bound PR review

**Strong point Notion and Cursor both lack.** Product: [spec-bound review](../product/product-plan.md#spec-bound-review).

When Venus reviews a PR with CodeGraph CLI (and/or Aider), it **builds a bound pack**. CodeGraph **enriches the PR’s context**. The landed feature is bound to the plan; the plan is bound to the documentation. Review is **already set up** in the loop (not a GitHub Action you forgot to install). It is forced to validate **code against the specification** — not only “did we make a mistake?” — and to ask whether **related docs** should ship in the **wiki** (or Hugo). System prompts cover **several lenses** (security, performance, product design, code style, and spec).

```text
PR / branch at productSha'     ← what landed
        bound to
plan step (accepted wiki)      ← what we said we would do
        bound to
spec / aims / API at wikiSha   ← the documentation
```

| Question | Notion | Cursor | Venus |
|---|---|---|---|
| Did we make a mistake? | No product PR. Ops agents type pages. | Yes — diff review, tests, linters | Yes — same, plus CodeGraph impact at the pin |
| Security / perf / design / style | Hope a Custom Agent was set up | Whatever you `@`’d this time | **System prompts** on those lenses, on the pack |
| Did we **follow the spec**? | Spec is the live page; no cloneable SHA the PR is judged against | If you `@` a README. Next session, gone | **Required.** Pack is the headings the plan was accepted against |
| How **close to the goal**? | Ticket or database row, if you linked one | The chat that kicked Composer | The **plan step’s named outcome**, bound to those docs |
| Docs that should ship (wiki / Hugo)? | Update Confluence later | Maybe a README in the diff | **Flag** related spec/wiki (or Hugo) pages that must publish with the change |

Notion’s GitHub connector dumps sources into chat with **no clock** and **no plan object**. Cursor’s review is the **diff plus whatever made it into context**. Neither has landed-feature → plan → documentation as the review working set. That chain is why Venus can ask “are we done with what we meant?” instead of “does this PR look fine?”

Do not sell CodeGraph as a better GitHub bot. The bot without the bind is Cursor’s bug-hunt. The bind without a product pin is Notion’s dump.

## Versus CodeSpeak

[CodeSpeak](https://codespeak.dev/) says **human intent is all that matters**. Venus says the same in other words. The fork is **where intent lives** and **what is allowed to update it**.

CodeSpeak is an agentic toolkit: capture intent **from agent chats**, turn it into structured requirements **mapped to code**, enforce them on every change, and **update those requirements as the system changes** so the spec “always reflects reality.” You review meaning, not the full diff. Specs that go stale, become slop, or get ignored after the first feature are the enemies they name.

The **review cycle** is the kinship. CodeSpeak: you accept human-language descriptions of what the **code** does — not the code itself. Venus uses that same accept, aimed at **spec change review**: you accept human-language descriptions of what the spec now says and why, when you understand them. Hunks are the tracking, not the blessing. You are not accepting a Cursor-style code patch that happens to be markdown.

Venus is the **project heart**: a collab spec humans accept. Agents do the work; they do not become the record by chatting. The spec is a document you read (and Hugo can publish), not a requirement that surfaces only when code is touched.

| | CodeSpeak | Venus |
|---|---|---|
| Source of intent | Extracted from agent chats | Written and **human-accepted** spec / aims (lease) |
| Spec shape | Structured requirements, shown when relevant | Wiki pages + git markdown (heart you can read front to back) |
| Bound to | Code (mapping, drift check) | Published spec git; **landed feature ↔ plan ↔ docs**; code must catch up, then spec change is accepted |
| PR review | Reqs mapped to code; spec follows the body | **Validate code against accepted spec** (closeness to goal, follow the docs) — not update spec from HEAD |
| What you accept | Human-language meaning of the **code** change | Human-language meaning of the **spec** change (comment-commit) |
| Stale spec | They auto-update reqs to match the system | Forbidden without a human: story not done until spec accept |
| If code and intent diverge | Prefer **reality of the code**; spec follows | Prefer **intention**; human decides whether the spec (heart) changes |
| Collab on the spec | Not the product (toolkit on chats/code) | Live CRDT + freeze on source lease |
| DoD | Confirm requirements were implemented | Other agent writes DoD; implementer cannot; human accept |
| Site / clone | Recover chats as docs/context | Ordinary markdown, Hugo from **accepted** git |

Same enemy (intent lost, spec ignored). Same *shape* of review (accept descriptions, not mute diffs). Opposite pulse and opposite object: CodeSpeak **follows the body** (code + chats) so the shadow spec stays true. Venus **pumps from the heart** (accepted spec) through plan → implement → test → review → iterate. If CodeSpeak wins, you never needed a wiki. If Venus wins, a chat log is not a spec — and a spec accept is not a code review.
