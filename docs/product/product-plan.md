# Product plan: first-class for development and agents

This is a **product** doc: what to add, in what order, and what not to build so Venus is a real development app and an agent orchestrator. It is not marketing. Positioning vs Notion / Cursor / CodeSpeak stays in [comparisons.md](../marketing/comparisons.md). The pitch stays in [pitch.md](../marketing/pitch.md). Pains: [pains.md](./pains.md). Editor, lease, git, MDGate stay in [design](../design/README.md). Plan invariants and open git decisions stay in [venus-plan.md](../drafts/pre-design/venus-plan.md).

## Product goal

**The spec loop must be shorter than Notion + a PR.**

If it is not, Venus is the same product as those, plus freeze and yaml. Dual store (live wiki + git files) only becomes a new category when the extra gates are **fewer than the drift they prevent**.

Bar: a PM writes prose in Venus, an agent (or source write) that **changes contract** goes through one meaning-accept, programmers implement from accepted git, merge is blocked until spec is current **or** the change is marked skip. That path must beat: type in Notion, open a GitHub PR, maybe remember to update the page.

Keep until that bar is green: MDGate, git snapshot, lease on **agent** writes, meaning-accept, skip when spec did not move, status check on merge, PM tree without Cursor.

Cut from v1 (they make the loop longer than Notion + a PR): two-agent DoD as a **hard** rule (warning only), alternatives/stacks, Hugo, Linear mirrors, “every story is a plan.”

M0–M8 is the spec wiki (dual store). First-class is that **short** outer loop. Cursor (and CI, PRs) stay the inner loop.

## The gap: CRDT is for humans, folders of markdown are for agents

CRDT (and Notion) was built so **humans** can type the same page at once. It is a bad agent interface: no clone, no grep, no PR, a proprietary graph, live writes. Agentic development hardens when the wiki is **markdown in folders** — ordinary git, simple for developers, what Cursor already eats.

Notion as docs for that loop **does not fit**. Export is a dump. Agents type the live graph. The spec is not files.

Venus closes that gap: humans keep a collab page (CRDT / WYSIWYG). Agents and developers get **the same spec as a git tree of `.md`**. MDGate is what makes those one spec, not two. Do not teach agents Yjs. Do not make developers live in Notion.

## Dual store is honest

Two representations of **one** published spec: the live page (BlockSuite CRDT — what PMs type) and git markdown (what programmers and agents clone). Honest means they are the **same spec**, not two workspaces that drift.

| Honest | Dishonest (Notion + a PR, or a broken Venus) |
|---|---|
| Clone of `wiki/` reads as the page the PM saw | Export rewrites wording, drops ids, invents hunks |
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
- The hop between Venus and Cursor is **git**, not the clipboard.

Venus is not only a wiki app. It is a **spec-first development flow**: one spec instead of Notion + git + agent chat. The app enforces the short loop (lease, git, skip, merge check). Teams that keep Figma, Notion, or Linear still need this once agents write code. Those tools are surfaces. They are not the pulse.

## Who needs the flow (including small teams, Figma, Notion)

Every team that ships with agents needs a spec-first loop. Small teams need it **more**: fewer humans to notice drift, more agent output per person. “Intention lives in one head” dies the first week a composer implements from chat.

Figma is canvas, not a spec agents can clone and not silently rewrite. Notion is a living workspace; its agents type the live page. Linear closes a card. None of that is “human accepted the meaning of the spec, then Cursor ran, then the spec caught up.” A Figma+Notion shop should **keep** Figma for design and Notion for ops if they want. They should not treat either as the artifact agents must not drift from.

What they may refuse is a **heavy second wiki**, not the loop. If Venus is the flow (few gates, shared git, skip when spec does not move), those teams can accept it. If Venus is “replace Notion and Figma,” they will not — and should not. Do not sell Venus as their home screen. Sell it as the framework that makes the rest of the body honest.

**PMs and other managers are first-class.** Only programmers live in Cursor. Product managers, design leads, and other managers will not adopt an IDE. They can adopt Venus: the wiki, the tree, WYSIWYG, and the **accept inbox** (understand this spec change). That is their Cursor loop. Programmers pull accepted git. If Venus requires Cursor to participate in the flow, the flow is only for engineers and the heart is empty.

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
| **Managers never need Cursor.** WYSIWYG + accept inbox + tree is a complete path to write and bless intention. | Only programmers have a heart. |
| Header shows the **gate** (who holds the lease, what is waiting to accept), not a kanban. | People live on a board. |

Data-model invariants (CRDT, freeze, two git classes) stay in [venus-design.md](../design/venus-design.md). These are **flow** invariants. Product: [below](#add-this--first-class-gap). v1 documenting the product: [implementation plan](../design/venus-implementation-plan.md#v1-dogfood--document-the-product).

## Shared git, not copy-paste

Humans and agents must not paste spec into chat or the IDE. They work from a **shared checkout** of the same git (workspace branch, or parent repo with wiki + product as submodules — [venus-plan § wiki vs product](../drafts/pre-design/venus-plan.md#2-wiki-repo-vs-product-repo)).

| Direction | How |
|---|---|
| **PM, prose / aims / narrative** | WYSIWYG → snapshot git. No ceremony. |
| **Contract** (behavior, API, DoD, must-clauses) | Human or agent: lease + meaning-accept. |
| **Agent** | Always lease. Never live CRDT. |
| **Programmer** | Read accepted git; write product code in Cursor; spec writes through Venus if contract moved. |

v1: clone-and-PR import of wiki markdown is later ([MDGate](../design/MDGate/README.md)). Until the adapter gate is green, do not tell agents “edit the `.md` in git and it will apply.” They acquire a lease (MCP or UI) and put hunks; accept still lands as a wiki commit the next `git pull` sees.

Decide submodule vs monorepo **before** the runner. One clone for agents; two histories if possible (lean: submodules).

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

Lease-shaped tools. Cursor stays in the IDE and talks git + this API.

- Read published spec / plan / DoD **at a git SHA** (disk after pull, or the same bytes via API).
- `acquire` / heartbeat / release; `putHunks`; submit a **human-language description** of the spec change.
- List open leases, waiting accepts, plan state, `pr` URLs.
- **v1:** warn if `dodAgentId` is missing or equals planner/implementer. **Later:** refuse. Hard refuse in v1 makes the loop longer than Notion + a PR.

### 2. Plan runner (orchestrator)

Not a board. Not “every story is a plan.” A state machine on `*.state.yaml` **when a plan exists**:

- Optional: spec (or a section) → planner lease → human accept → implementer may run. DoD from another agent is a **warning** in v1, a hard gate later.
- Step: `implement → tests → PR → breakpoint → done`. Runner kicks **external** agents (Cursor, CI, PR review). It does not write code.
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
- “Open this plan in Cursor” is a clone/SHA on the shared branch, not a new editor.

## Do not add

| Temptation | Why it kills Venus |
|---|---|
| In-wiki coding / Cursor clone | Inner loop already won |
| Copy-paste spec into the IDE | Shared git is the hop |
| 24/7 Custom Agents that write the live page | Notion; silent spec |
| Linear-like home board | People live on the board; spec dies |
| Linear mirrors, Hugo, alternatives/stacks in v1 | Loop longer than Notion + a PR |
| Every story is a plan | Same |
| Two-agent DoD as a hard v1 block | Same |
| Chat as the spec | CodeSpeak’s object, opposite pulse |
| Mute apply of markdown | Accepting code, not meaning |
| Cursor required to bless spec | PMs cannot adopt the flow |

## Order

Judge each slice against the goal: **shorter than Notion + a PR**.

1. MDGate + git snapshot + PM tree (dual store works; managers write prose).
2. Lease on **agent** (and contract) writes + meaning-accept + skip.
3. **Status check on merge** (otherwise the loop is a lecture).
4. MCP + accept inbox (shared git; no clipboard; PMs never need Cursor).
5. Plan runner **optional** (not every story). Two-agent DoD warn, then later refuse.
6. Hugo / autodoc / Linear mirrors — only after the bar is green.

Until the dual store is honest and a contract change is **one** meaning-accept plus a PR (or skip), Venus is AFFiNE + a GitHub workflow spec, aimed at a Notion user, justified by a Cursor metaphor. That can become the flow. The product goal is to **make the loop shorter**, not to add gates.
