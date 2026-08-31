# Unique features of Venus

What Venus **is** that neighbors are not. Not the pitch ([pitch.md](../marketing/pitch.md)), not order/keep-cut ([product-plan.md](./product-plan.md)), not vs-whom tables ([comparisons.md](../marketing/comparisons.md)). Pains these close: [pains.md](./pains.md).

**Promote the way of work first** (spec-driven development in the agentic era — successor to Scrum), then this join as the **tool** that replaces Jira+Confluence for that work ([product-plan.md](./product-plan.md), Category). Not only for teams that already keep an LLM wiki.

Pieces exist elsewhere (CRDT wikis, git markdown, PR review, CodeGraph). Uniqueness is the **join**: **LLM wiki as a framework** (agentic process on by default, frames how you work — including **automated spec-bound PR review**), team brainstorm on one collab spec, agents fit it to LLM-wiki structure, **share while Cursor stays the IDE**, **live sessions leave a session-bound commit**, two honest stores, agents cannot publish, review is against that spec. Sell the join. Do not sell the editor, the board, a copilot, or cheap tokens.

**Bar:** the loop must stay shorter than Notion + a PR ([product-plan.md](./product-plan.md), Product goal). A unique gate that makes the loop longer is a bug.

**Use:** (1) a team call to brainstorm software design on collaborative docs ([product-plan.md](./product-plan.md), Product use: brainstorming). (2) **Share while Cursor stays the IDE** — commit markdown, team edits WYSIWYG, pull back ([product-plan.md](./product-plan.md), Product use: share). Live sessions leave a **session-bound commit** — a measurable spec diff of what the call consensused ([product-plan.md](./product-plan.md), Session-bound commit).

**Shipped vs story:** dual store is the next spine (M3). Cursor-commit → WYSIWYG apply is **M6**. Session-bound commit needs M3 git, then agent lease — do not sell idle flush as “what the call decided.” Graph, bound chat, spec-bound PR review are in the story from day one; do not sell them as shipped before their gates ([product-plan.md](./product-plan.md)).

## 1. Collaborative brainstorming (cross-department spec)

An **LLM wiki** is usually something **one engineer** wrote in a **personal IDE**. Venus is the other start: a **team call** on collaborative docs. Product, project, marketing, and development type together (live CRDT, no lease — same as prose). During the session or right after, they **aim agents** to fit that prose into **LLM-wiki structure**: tree, headings, marketing and product goals as spec pages, binds. Agents draft that structure under lease; humans meaning-accept. The agent does **not** become the record by typing the live page.

The **UI is Notion-like on purpose** — to remove the learning curve so that audience actually joins the call. WYSIWYG and live collab are familiar. What is not Notion: the session becomes a **cloneable spec**, agents **structure** it under lease, marketing and product goals stay on that tree, and shipping still hits the loop bar.

That room is where **all directions meet**. The end-product shape is adjusted by a specification that has to satisfy every department — not a marketing deck, not a ticket, not a Cursor chat that nobody else can join.

| Neighbor | What they do instead |
|---|---|
| Cursor / personal IDE | One implementer writes `docs/` alone **or** dumps a page the team cannot edit. **Share use closes the dump:** commit → Venus WYSIWYG → pull. Learning curve of the IDE stays optional, not required. |
| Confluence | Usual home of **technical design specs** next to Jira. **Not lightweight.** Dump → chat → markdown. **Learning curve is the wiki; the hop is the clipboard.** |
| Notion / Google Docs | Same *gestures* (type together). The page **is** the product; Agent may type it; export is a dump; marketing stays ops. **Learning curve is low; the object is wrong for software spec.** |
| Miro / FigJam | Canvas. Not a spec agents clone. |

Do not sell “we also have multiplayer.” Notion won that. Do not sell a foreign editor and lose the room. The unique join is **familiar collab → structured LLM wiki → accepted spec**, with every department still on that object. The call itself leaves a **session-bound commit** (§12). Detail: [product-plan.md](./product-plan.md) (Product use: brainstorming, Vs Notion). Pain: [pains.md](./pains.md) (§10, §12).

## 2. Honest dual store (block CRDT + git markdown)

Live collab is a **block CRDT** (WYSIWYG for PMs, high-level engineers, CTOs). Agents and implementers clone **the same spec** as folders of `.md`. Markdown is a **clocked projection** (pin → convert → git SHA), not a second live replica and not an export dump. That is what closes **import → shape in Cursor → people who do not use markdown cannot edit**, **Confluence dump**, and **Word / Google Docs ↔ ChatGPT paste** ([pains.md](./pains.md) §11). Do not sell a better importer.

| Neighbor | What they do instead |
|---|---|
| Notion / AFFiNE | CRDT is truth. Markdown is dump or AI context. No cloneable `wiki/` as the page. Lightweight collab; **wrong object.** |
| Confluence | Heavy wiki. Technical design lives here. Dump to markdown. No live collab room. |
| OpenKnowledge / Stele / Muesli | Markdown **is** the CRDT (Y.Text / dual-observer). Agents type the live doc or the file. |
| GitBook / Tina | Git markdown is truth. No live block-CRDT wiki. Collab is branches. |
| HedgeDoc | CRDT over markdown in a database. Git is not the product. |
| Word / Google Docs + ChatGPT | Clipboard is the hop. Paste in, paste back. No shared spec; no clone. |

MDGate + snapshotter make the two stores **one spec**. Do not teach agents Yjs.

**Use of that store:** people may keep Cursor as the IDE. Venus is the **share place**: commit → team sees WYSIWYG and can change it → you pull back. Not a dump. Not “replace the IDE.” Cursor is still good at **writing** markdown; it is bad at **accepting** a doc (preview has no diff — [pains.md](./pains.md) §14). Detail: [product-plan.md](./product-plan.md) (Dual store is honest, Product use: share). Pain: [pains.md](./pains.md) (§1, §11, §14).

## 3. Agentic-native (user-to-agent)

Surfaces exist so **agents can work** (git, lease, hunks, Bind pack, comment-commit). Human chrome **aims** them: point at a span, bless meaning, skip. Not a human wiki/board with a copilot adopted onto the same UI.

Notion Agent types the live page. Linear bots click tickets. That category already won. Venus is the other direction.

**Included, not connected:** Venus is a **framework for an LLM wiki**. Agentic jobs (index, background semantic graph, commit comments, leased edit) are **the process**, **on by default**, and they **frame how users work**. A lot of people never use Notion Agent because it must be **set up first**. Confluence and Linear are the same: human tool, optional AI. That is a **strong product difference**, not an add-on SKU. Gonka-native Kimi and MiniMax in the subscription is how that default runs. Detail: [product-plan.md](./product-plan.md) (Agentic-native). Pain: [pains.md](./pains.md) (§8, §13). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus Notion).

## 4. Agents cannot publish

Spec / plan / DoD / docs **apply is human-only**. Agents draft under a **lease**; the published page is frozen. They never type the live CRDT. Truth is a **meaning-accept** (human-language description of the spec change, once the human understands it) — not mute apply, not page-history undo.

That is the CodeSpeak-shaped review cycle aimed at **spec**, not code. Humans review on the **rendered** After / Before / Diff — not source-only marks. Cursor’s markdown preview has **no diff**; accept lives in source. That is a **real pain Venus must close** ([pains.md](./pains.md) §14). Detail: [product-plan.md](./product-plan.md) (Force these). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus Cursor).

## 5. Skip + merge check (spec is the done bar)

If contract **did not** move: **skip**. No lease theater on a bugfix. If it **did**: story is not done until meaning-accept. GitHub (or equivalent) stays red until Venus says spec is current **or** skipped.

Linear “Done” and “PR merged” are not the pulse. Pain: [pains.md](./pains.md) (§5).

## 6. Accept inbox without Cursor

PMs, other managers, high-level software engineers, and CTOs adopt Venus: tree, WYSIWYG, **accept inbox**. They will not adopt an IDE. Implementers clone accepted wiki git. The IDE is optional inner-loop chrome; Venus kicks **Cursor CLI**.

Without this audience in Venus, only day-to-day implementers have a heart. Detail: [product-plan.md](./product-plan.md) (Who needs the flow).

## 7. Multidimensional spec graph (spatial + temporal)

The **main wiki feature**. Headings bind to headings **and** to the timeline of accepted whys, keyed by sidecar ids at a git SHA.

| Axis | Ask |
|---|---|
| **Spatial** | If I change this, what else is in force? |
| **Temporal** | Why is it designed this way? Which SHA do we revert? |

Notion: `@` / search / page-history **blob**. Cursor: why in **chat**. Snapshot autocomments are not why.

**Do not sell at AB1.** Spatial without temporal is “what else is in force,” not pain 7. **AB4 cannot exist until M6.** Detail: [product-plan.md](./product-plan.md) (Multidimensional spec graph), [Agents README](../design/Agents/README.md). Pain: [pains.md](./pains.md) (§7).

## 8. Bound chat (Cursor gesture, spec object)

Point at a span → Bind `{ sha, docId, blockIds }` → pack from the graph. Composer shows the **quote**, not the dump. **AB2 is ask-only** (no wiki write). **AB3** chat-edit is lease + hunks after **M5–M6 checkout**, not Notion typing the live page, not “AB2 next.”

That is Cursor **add-to-chat** on a spec SHA — not Notion-with-a-copilot. Detail: [agentic-comparison.md](../marketing/agentic-comparison.md), [agentic-binding.md](../design/Agents/agentic-binding.md).

## 9. Spec-bound PR review (landed ↔ plan ↔ docs)

**Strong vs Notion and Cursor. Same default as the LLM-wiki framework:** review is **already automated and set up**. You do not wire a GitHub bot.

**CodeGraph CLI enriches the PR’s context.** System prompts then look at the same PR from **several sides** (security, performance, product design, code style, …) **and** whether it **meets the spec** **and** whether related **new documentation** should ship in the **wiki** (or Hugo when that surface exists). Flags missing docs; does not publish.

When CodeGraph CLI (and/or Aider) reviews a PR, Venus builds a **bound pack**, not a repo dump:

```text
landed feature   (PR / branch at productSha')
      bound to
plan step        (accepted wiki plan that asked for that work)
      bound to
documentation    (spec / aims / API at wikiSha)
```

Review **validates code against the specification**: mistakes, **did we follow the spec**, **how close to the goal**, **did docs that belong in the wiki (or Hugo) come with the change**. Not a GitHub bug-hunt bot. Analyzer does **not** implement. v1: warning; merge green stays spec current or skip.

Detail: [product-plan.md](./product-plan.md) (Spec-bound review). Comparison: [comparisons.md](../marketing/comparisons.md) (Spec-bound PR review). Pain: [pains.md](./pains.md) (§9).

## 10. Two gits, two clocks

Wiki remote (spec / plan / DoD) ≠ product remote (application code). Pins `{ wikiSha, productSha }`. Code is packed only when the recipe asks, at the product pin — not Notion’s GitHub connector dump.

Cursor remains implementer. Aider / CodeGraph **review** the step’s PR or branch. Detail: [product-plan.md](./product-plan.md) (Workspace and Aider), [code-bind.md](../design/Agents/code-bind.md).

## 11. Plan and DoD are spec (not a ticket checklist)

When a plan exists, it is a **wiki page** on the same lease/accept path. DoD is written by a **different agent** than implementer (v1: warn, not a hard block). The board is yaml + the plan page; Linear stays a mirror.

Intention lives in accepted spec, not in chats (CodeSpeak’s object, opposite pulse). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus CodeSpeak). Pain: [pains.md](./pains.md) (§2, §6).

## 12. Session-bound commit (live call artifact)

**Brainstorms and other live sessions stay a meaningful artifact.** A background agent sees session activity and unites it into a **commit bound to that session**. Everyone opens a **document diff** and knows what was achieved or consensused on that call.

Notion/Confluence: the page moved, or a history blob, or notes in another doc. Idle git snapshot: the wiki flushed, not “this call.” Chat recap: gone.

Humans type the room (no lease). The agent does **not** type the live page. It unites after, under lease. Contract still meaning-accept. Detail: [product-plan.md](./product-plan.md) (Session-bound commit). Pain: [pains.md](./pains.md) (§12).

## 13. LLM wiki framework (agentic process by default)

**Product (strong vs Notion, Confluence, Linear):** Venus is a **framework for an LLM wiki**. Agentic integrations are **already the process** — set up by default — and they **frame how you work**. Index, background semantic graph, commit comments, leased wiki edit, session-bound commit, **automated PR review** (CodeGraph-enriched pack, spec + docs-to-ship): not optional chrome you configure later. Neighbors are human tools (wiki, board, GitHub) with an agent or review bot you must **turn on**. Most people never do.

**Marketing:** Gonka-native. Kimi and MiniMax tokens in the subscription so that default actually runs. Notion Agent is extra payment **and** setup. Confluence / Linear: paste a key or install an app. Cursor already includes models for **code**; Venus includes them so the **spec framework** is live on day one. Do not lead the pitch with model names. Do not sell cheaper Notion AI.

Detail: [product-plan.md](./product-plan.md) (Agentic-native). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus Notion, Versus Confluence, Versus PM tools). Pain: [pains.md](./pains.md) (§13).

## Not unique — do not sell these as Venus

| Piece | Who already won |
|---|---|
| Live collab typing | Notion, Google Docs. Venus is **brainstorm → session-bound commit → LLM wiki structure**, not a second Docs. |
| WYSIWYG / CRDT editor | Notion, AFFiNE, BlockSuite |
| Markdown in git | Git, Tina, GitBook, every `docs/` folder. Cursor **writes** this well. **Accept in preview** is Venus (Cursor preview has no diff). |
| Implement code from a prompt | Cursor (CLI is enough) |
| Diff review, linters, tests | GitHub, Cursor |
| CodeGraph / Aider as CLIs | Those tools. Venus is the **bind**, not a second analyzer |
| Ops wiki, databases, Slack | Notion |
| Board, cycles, points | Linear |
| Hosted LLM / “AI add-on” | Notion Agent people never set up; Linear/Confluence optional AI. Venus is an **LLM-wiki framework** with the agentic process **on by default**, not a unique model. |

If Venus is “AFFiNE + a GitHub workflow, aimed at a Notion user, justified by a Cursor metaphor,” the unique list above did not ship.

## Files

| File | Role |
|---|---|
| [unique-features.md](./unique-features.md) | This catalog |
| [product-plan.md](./product-plan.md) | Goal, order, force, keep/cut |
| [pains.md](./pains.md) | What is broken in the world |
| [comparisons.md](../marketing/comparisons.md) | Vs Linear / Notion / Cursor / CodeSpeak |
| [pitch.md](../marketing/pitch.md) | Heart / empty cell |
