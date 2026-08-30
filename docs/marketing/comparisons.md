# Venus comparisons

Pitch: [pitch.md](../marketing/pitch.md). Design: [venus-design.md](../design/venus-design.md), [venus-plan.md](../drafts/pre-design/venus-plan.md). Product: [product-plan](../product/product-plan.md), [pains](../product/pains.md).

## Versus PM tools (Linear, Jira, Plane, GitHub Issues)

If Venus looks like a PM tool, Linear wins in a week. Those products are first-class **work items**: states, assignees, cycles, views. Spec-driven development there is a description field or a link. An agent reads the ticket, opens a PR, and **done means the issue closed**. The spec is optional and usually stale.

Venus inverts that. **The spec is first-class; the board is a mirror.** Agentic spec-driven development is the product loop, not a Linear integration:

| | Linear / Jira / Plane / Issues | Venus |
|---|---|---|
| Unit of work | Issue / work item | Spec page (and a plan derived from it) |
| What the agent must not drift from | Ticket title + description | Published spec git + leased drafts |
| Plan / DoD | Checklist on the issue, written by whoever | Plan then **other agent** DoD, both human-accepted wiki leases |
| Parallel humans on the spec | Comments on the ticket | Live CRDT; source edits freeze under a lease |
| Story complete | Status = Done | Spec (and aims/API) **diff accepted by a human** |
| Board | The product | `*.state.yaml` + plan page; Issues/Plane optional mirrors |

Use a PM tool for intake, roadmap, and work that is not a spec (triage, sales, “fix the button”). The **spec-first loop** is still required: closing a card without accepting the spec is a bug. Linear can stay the mirror. Venus is the flow, not a worse board.

Do not build Venus cycles, points, or a kanban as the home screen. If people live on the board, you are a worse Linear. If they live on the spec and the board follows yaml, agentic SDD is first citizen and Linear stays the mirror — or stays out.

## Versus Notion (and Notion agents)

Notion is a CRDT (block graph) for humans. It does not fit **docs for agentic development**: agents cannot clone folders of markdown; they type the live workspace. Venus exists to close that gap — LLM wiki as **git markdown**, collab page for humans, same spec.

If Venus looks like Notion with a copilot, Notion still wins at ops. The job Notion cannot do is a spec that hardens Cursor: files in folders, honest export, agents do not publish by typing.

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

Notion Agent acts as you and edits in place. Custom Agents are named teammates with their own ACL, schedules, and connectors. That is the right product for triage, standups, Q&A, and keeping databases current. It is the wrong default for an LLM wiki whose second author is a model: the agent becomes the record the moment it types. There is no honest git loop, no human-language accept of the spec change, and no rule that the same model cannot write the bar and the work that “satisfies” it. Version history can revert a page; it cannot tell you what you agreed the spec now means, and why.

Use Notion when the wiki **is** the ops workspace (databases, standups, Q&A). Keep Figma for canvas. Those teams still need a **spec-first development loop** once agents implement — Notion-with-agents is not that loop. Use Venus as the flow: the artifact agents share with humans **is the spec**, writes are a checkout, a story is not done until a human accepts the **meaning** of that spec change. Notion stays ops; it does not stay the heart.

Do not build Notion databases, Slack routing, or 24/7 report bots. If people live in the live page while an agent patches it, you are a worse Notion. If they live on the spec and agent writes go through the lease, Venus is the LLM wiki; Notion stays the ops surface — or stays out.

## Versus Cursor

Cursor is the loop Venus copies for source: you ask (change or audit), the working set is exclusive, you wait, you review the diff, you accept. That is an efficient **developer-first** model, not a lockout for the person who prompted. Freeze is that contract on a wiki page.

The write loop is Cursor’s. The **accept** is CodeSpeak-shaped, aimed at spec: you are not rubber-stamping markdown hunks the way you accept a code patch. You accept human-language descriptions of what the spec now says and why, when you understand them. Hunks and the rail are how that description stays attached to the change; they are not the thing you are blessing as “the code looks fine.”

Cursor is the **code** editor. Venus is the **spec** wiki that same loop must not silently skip.

| | Cursor | Venus |
|---|---|---|
| Surface | Product repo (files, PR) | Spec wiki (CRDT + git markdown) |
| Agent loop | Prompt → dirty buffer → review → accept | Same loop: lease → private markdown → After/Before/Diff → human accept |
| What you accept | Code diff; why is in the chat | Human-language description of the **spec** change, once you understand it (hunks are the evidence) |
| Tracking | Chat + git; why is not on the hunks | Descriptions pinned to hunks; git message is that why |
| Multiplayer during the loop | You and the agent; other files stay yours | Page freeze for everyone on that `docId`; other pages stay live; comment rail stays |
| Story complete | PR merged / tests green | Spec diff **accepted**; PR is the implementer path, not done |
| Spec | README, rules, chat — optional and drift-prone | First-class; agents do not become the record by typing |
| Role | Implementer (and local review) | Heart: intention accepted, then Cursor may run |

**Freeze is the current design**, tuned for this Cursor-like loop: one source writer, stable `T0`, then a CodeSpeak-like spec accept. It can be **extended later** (narrower leases, other exclusive modes) once the loop is boring. Do not treat whole-page freeze as the forever product, and do not treat it as a bug. Primary design is: you asked, you wait, you accept **the meaning of the spec change** — Cursor’s checkout, CodeSpeak’s review, on the wiki. The extra wiki cost is only the person who did **not** prompt: they review or they open another page. That is checkout, not Cursor; v1 accepts it.

Use Cursor to write product code against a **published** contract on a **shared git checkout** — not copy-paste. Use Venus when the spec must survive that run. The accept on that spec is not a mute apply.

**Bar:** the whole loop must stay **shorter than Notion + a PR** ([product-plan](../product/product-plan.md)). If it does not, Venus is AFFiNE + a GitHub workflow, aimed at a Notion user, justified by a Cursor metaphor.

## Versus CodeSpeak

[CodeSpeak](https://codespeak.dev/) says **human intent is all that matters**. Venus says the same in other words. The fork is **where intent lives** and **what is allowed to update it**.

CodeSpeak is an agentic toolkit: capture intent **from agent chats**, turn it into structured requirements **mapped to code**, enforce them on every change, and **update those requirements as the system changes** so the spec “always reflects reality.” You review meaning, not the full diff. Specs that go stale, become slop, or get ignored after the first feature are the enemies they name.

The **review cycle** is the kinship. CodeSpeak: you accept human-language descriptions of what the **code** does — not the code itself. Venus uses that same accept, aimed at **spec change review**: you accept human-language descriptions of what the spec now says and why, when you understand them. Hunks are the tracking, not the blessing. You are not accepting a Cursor-style code patch that happens to be markdown.

Venus is the **project heart**: a collab spec humans accept. Agents do the work; they do not become the record by chatting. The spec is a document you read (and Hugo can publish), not a requirement that surfaces only when code is touched.

| | CodeSpeak | Venus |
|---|---|---|
| Source of intent | Extracted from agent chats | Written and **human-accepted** spec / aims (lease) |
| Spec shape | Structured requirements, shown when relevant | Wiki pages + git markdown (heart you can read front to back) |
| Bound to | Code (mapping, drift check) | Published spec git; code must catch up, then spec change is accepted |
| What you accept | Human-language meaning of the **code** change | Human-language meaning of the **spec** change (comment-commit) |
| Stale spec | They auto-update reqs to match the system | Forbidden without a human: story not done until spec accept |
| If code and intent diverge | Prefer **reality of the code**; spec follows | Prefer **intention**; human decides whether the spec (heart) changes |
| Collab on the spec | Not the product (toolkit on chats/code) | Live CRDT + freeze on source lease |
| DoD | Confirm requirements were implemented | Other agent writes DoD; implementer cannot; human accept |
| Site / clone | Recover chats as docs/context | Ordinary markdown, Hugo from **accepted** git |

Same enemy (intent lost, spec ignored). Same *shape* of review (accept descriptions, not mute diffs). Opposite pulse and opposite object: CodeSpeak **follows the body** (code + chats) so the shadow spec stays true. Venus **pumps from the heart** (accepted spec) through plan → implement → test → review → iterate. If CodeSpeak wins, you never needed a wiki. If Venus wins, a chat log is not a spec — and a spec accept is not a code review.
