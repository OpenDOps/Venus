# Unique features of Venus

What Venus **is** that neighbors are not. Not the pitch ([pitch.md](../marketing/pitch.md)), not order/keep-cut ([product-plan.md](./product-plan.md)), not vs-whom tables ([comparisons.md](../marketing/comparisons.md)). Pains these close: [pains.md](./pains.md).

Pieces exist elsewhere (CRDT wikis, git markdown, PR review, CodeGraph). Uniqueness is the **join**: one spec, two honest stores, agents cannot publish, review is against that spec. Sell the join. Do not sell the editor, the board, or a copilot.

**Bar:** the loop must stay shorter than Notion + a PR ([product-plan.md](./product-plan.md), Product goal). A unique gate that makes the loop longer is a bug.

**Shipped vs story:** dual store is the next spine (M3). Graph, bound chat, spec-bound PR review are in the story from day one; do not sell them as shipped before their gates ([product-plan.md](./product-plan.md)).

## 1. Honest dual store (block CRDT + git markdown)

Live collab is a **block CRDT** (WYSIWYG for PMs, high-level engineers, CTOs). Agents and implementers clone **the same spec** as folders of `.md`. Markdown is a **clocked projection** (pin → convert → git SHA), not a second live replica and not an export dump.

| Neighbor | What they do instead |
|---|---|
| Notion / AFFiNE | CRDT is truth. Markdown is dump or AI context. No cloneable `wiki/` as the page. |
| OpenKnowledge / Stele / Muesli | Markdown **is** the CRDT (Y.Text / dual-observer). Agents type the live doc or the file. |
| GitBook / Tina | Git markdown is truth. No live block-CRDT wiki. Collab is branches. |
| HedgeDoc | CRDT over markdown in a database. Git is not the product. |

MDGate + snapshotter make the two stores **one spec**. Do not teach agents Yjs. Detail: [product-plan.md](./product-plan.md) (Dual store is honest). Pain: [pains.md](./pains.md) (§1).

## 2. Agentic-native (user-to-agent)

Surfaces exist so **agents can work** (git, lease, hunks, Bind pack, comment-commit). Human chrome **aims** them: point at a span, bless meaning, skip. Not a human wiki/board with a copilot adopted onto the same UI.

Notion Agent types the live page. Linear bots click tickets. That category already won. Venus is the other direction. Detail: [product-plan.md](./product-plan.md) (Agentic-native). Pain: [pains.md](./pains.md) (§8).

## 3. Agents cannot publish

Spec / plan / DoD / docs **apply is human-only**. Agents draft under a **lease**; the published page is frozen. They never type the live CRDT. Truth is a **meaning-accept** (human-language description of the spec change, once the human understands it) — not mute apply, not page-history undo.

That is the CodeSpeak-shaped review cycle aimed at **spec**, not code. Detail: [product-plan.md](./product-plan.md) (Force these). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus Notion).

## 4. Skip + merge check (spec is the done bar)

If contract **did not** move: **skip**. No lease theater on a bugfix. If it **did**: story is not done until meaning-accept. GitHub (or equivalent) stays red until Venus says spec is current **or** skipped.

Linear “Done” and “PR merged” are not the pulse. Pain: [pains.md](./pains.md) (§5).

## 5. Accept inbox without Cursor

PMs, other managers, high-level software engineers, and CTOs adopt Venus: tree, WYSIWYG, **accept inbox**. They will not adopt an IDE. Implementers clone accepted wiki git. The IDE is optional inner-loop chrome; Venus kicks **Cursor CLI**.

Without this audience in Venus, only day-to-day implementers have a heart. Detail: [product-plan.md](./product-plan.md) (Who needs the flow).

## 6. Multidimensional spec graph (spatial + temporal)

The **main wiki feature**. Headings bind to headings **and** to the timeline of accepted whys, keyed by sidecar ids at a git SHA.

| Axis | Ask |
|---|---|
| **Spatial** | If I change this, what else is in force? |
| **Temporal** | Why is it designed this way? Which SHA do we revert? |

Notion: `@` / search / page-history **blob**. Cursor: why in **chat**. Snapshot autocomments are not why.

**Do not sell at AB1.** Spatial without temporal is “what else is in force,” not pain 7. **AB4 cannot exist until M6.** Detail: [product-plan.md](./product-plan.md) (Multidimensional spec graph), [Agents README](../design/Agents/README.md). Pain: [pains.md](./pains.md) (§7).

## 7. Bound chat (Cursor gesture, spec object)

Point at a span → Bind `{ sha, docId, blockIds }` → pack from the graph. Composer shows the **quote**, not the dump. **AB2 is ask-only** (no wiki write). **AB3** chat-edit is lease + hunks after **M5–M6 checkout**, not Notion typing the live page, not “AB2 next.”

That is Cursor **add-to-chat** on a spec SHA — not Notion-with-a-copilot. Detail: [agentic-comparison.md](../marketing/agentic-comparison.md), [agentic-binding.md](../design/Agents/agentic-binding.md).

## 8. Spec-bound PR review (landed ↔ plan ↔ docs)

**Strong vs Notion and Cursor.** When CodeGraph CLI (and/or Aider) reviews a PR, Venus builds a **bound pack**, not a repo dump:

```text
landed feature   (PR / branch at productSha')
      bound to
plan step        (accepted wiki plan that asked for that work)
      bound to
documentation    (spec / aims / API at wikiSha)
```

Review **validates code against the specification**: mistakes, **did we follow the spec**, **how close to the goal**. Not a GitHub bug-hunt bot. Analyzer does **not** implement. v1: warning; merge green stays spec current or skip.

Detail: [product-plan.md](./product-plan.md) (Spec-bound review). Comparison: [comparisons.md](../marketing/comparisons.md) (Spec-bound PR review). Pain: [pains.md](./pains.md) (§9).

## 9. Two gits, two clocks

Wiki remote (spec / plan / DoD) ≠ product remote (application code). Pins `{ wikiSha, productSha }`. Code is packed only when the recipe asks, at the product pin — not Notion’s GitHub connector dump.

Cursor remains implementer. Aider / CodeGraph **review** the step’s PR or branch. Detail: [product-plan.md](./product-plan.md) (Workspace and Aider), [code-bind.md](../design/Agents/code-bind.md).

## 10. Plan and DoD are spec (not a ticket checklist)

When a plan exists, it is a **wiki page** on the same lease/accept path. DoD is written by a **different agent** than implementer (v1: warn, not a hard block). The board is yaml + the plan page; Linear stays a mirror.

Intention lives in accepted spec, not in chats (CodeSpeak’s object, opposite pulse). Comparison: [comparisons.md](../marketing/comparisons.md) (Versus CodeSpeak). Pain: [pains.md](./pains.md) (§2, §6).

## Not unique — do not sell these as Venus

| Piece | Who already won |
|---|---|
| WYSIWYG / CRDT editor | Notion, AFFiNE, BlockSuite |
| Markdown in git | Git, Tina, GitBook, every `docs/` folder |
| Implement code from a prompt | Cursor (CLI is enough) |
| Diff review, linters, tests | GitHub, Cursor |
| CodeGraph / Aider as CLIs | Those tools. Venus is the **bind**, not a second analyzer |
| Ops wiki, databases, Slack | Notion |
| Board, cycles, points | Linear |

If Venus is “AFFiNE + a GitHub workflow, aimed at a Notion user, justified by a Cursor metaphor,” the unique list above did not ship.

## Files

| File | Role |
|---|---|
| [unique-features.md](./unique-features.md) | This catalog |
| [product-plan.md](./product-plan.md) | Goal, order, force, keep/cut |
| [pains.md](./pains.md) | What is broken in the world |
| [comparisons.md](../marketing/comparisons.md) | Vs Linear / Notion / Cursor / CodeSpeak |
| [pitch.md](../marketing/pitch.md) | Heart / empty cell |
