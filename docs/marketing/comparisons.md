# Venus comparisons

Pitch: [pitch.md](./pitch.md). Design: [venus-design.md](../design/venus-design.md), [venus-plan.md](../drafts/pre-design/venus-plan.md).

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

Use a PM tool for intake, roadmap, and work that is not a spec (triage, sales, “fix the button”). Use Venus when the artifact agents share with humans **is the spec**, and closing a card without accepting that spec is a bug.

Do not build Venus cycles, points, or a kanban as the home screen. If people live on the board, you are a worse Linear. If they live on the spec and the board follows yaml, agentic SDD is first citizen and Linear stays the mirror — or stays out.

## Versus CodeSpeak

[CodeSpeak](https://codespeak.dev/) says **human intent is all that matters**. Venus says the same in other words. The fork is **where intent lives** and **what is allowed to update it**.

CodeSpeak is an agentic toolkit: capture intent **from agent chats**, turn it into structured requirements **mapped to code**, enforce them on every change, and **update those requirements as the system changes** so the spec “always reflects reality.” You review meaning, not the full diff. Specs that go stale, become slop, or get ignored after the first feature are the enemies they name.

Venus is the **project heart**: a collab spec humans accept. Agents do the work; they do not become the record by chatting. The spec is a document you read (and Hugo can publish), not a requirement that surfaces only when code is touched.

| | CodeSpeak | Venus |
|---|---|---|
| Source of intent | Extracted from agent chats | Written and **human-accepted** spec / aims (lease) |
| Spec shape | Structured requirements, shown when relevant | Wiki pages + git markdown (heart you can read front to back) |
| Bound to | Code (mapping, drift check) | Published spec git; code must catch up, then spec diff is accepted |
| Stale spec | They auto-update reqs to match the system | Forbidden without a human: story not done until spec accept |
| If code and intent diverge | Prefer **reality of the code**; spec follows | Prefer **intention**; human decides whether the spec (heart) changes |
| Collab on the spec | Not the product (toolkit on chats/code) | Live CRDT + freeze on source lease |
| DoD | Confirm requirements were implemented | Other agent writes DoD; implementer cannot; human accept |
| Site / clone | Recover chats as docs/context | Ordinary markdown, Hugo from **accepted** git |

Same enemy (intent lost, spec ignored). Opposite pulse: CodeSpeak **follows the body** (code + chats) so the shadow spec stays true. Venus **pumps from the heart** (accepted spec) through plan → implement → test → review → iterate. If CodeSpeak wins, you never needed a wiki. If Venus wins, a chat log is not a spec.
