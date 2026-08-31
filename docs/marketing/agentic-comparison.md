# Agentic comparison — bound chat

How Venus **asks** and **remembers** the spec vs Notion Agent and Cursor chat. Better for **LLM wiki design** only if the write path stays a checkout and the **multidimensional graph** stays honest (spatial binds + accepted why). Chat-edit is **AB3** ([agentic-binding](../design/Agents/agentic-binding.md#ab3--chat-edit-markdown)) — Cursor apply, not Notion live type. Write loop: [comparisons.md](./comparisons.md). Aider / CodeGraph CLI: **spec-bound PR or branch review** (landed ↔ plan ↔ docs), not implement ([product-plan](../product/product-plan.md#spec-bound-review)). Graph: [Agents](../design/Agents/README.md), [LifeIndexing](../design/Agents/LifeIndexing.md), [product-plan](../product/product-plan.md#multidimensional-spec-graph). Pain (why / evolution): [pains §7](../product/pains.md#7-why-is-it-designed-this-way). Pitch: [pitch.md](./pitch.md).

The **gesture** is Cursor. The **object** (a page, a block) is Notion. The **index** — a **main product feature** — is the multidimensional spec graph.

Venus is not “put an agent in a wiki.” That is adopting agents **onto human tools** (Notion Agent). Venus is an **agentic-native** spec environment: the pack, lease, and git exist for the model; the human UI is a **user-to-agent** tool (point, ask, accept). [product-plan](../product/product-plan.md#agentic-native-user-to-agent).

## If it feels like Notion with a copilot, Notion wins

Notion already ships the obvious wiki-AI product:

- Default context is the **current page**.
- Select **blocks** → Agent focuses on those blocks.
- `@` a page (or person); “All sources,” search, connectors, MCP for more.
- The agent may then **edit the live workspace**.

That is **page + selection + search + explicit mentions**, on a living CRDT. It is a very good ops product. It is not a spec working set.

If Venus chat is a sidebar that “knows the wiki,” answers from the open page, and can tidy the prose in place, users will score it against Notion Agent: databases, permissions, connectors, Custom Agents, ten years of workspace. **You cannot win “collab wiki with an AI teammate.”** Notion already is that. Product: if Venus looks like Notion with a copilot, Notion still wins at ops ([comparisons — Notion](./comparisons.md#versus-notion-and-notion-agents)).

Bound chat must not feel like a teammate **in** the page. It must feel like pointing at **source**.

## Bound chat should feel like Cursor add-to-chat

Cursor’s ask gesture:

- You **highlight** a span and add it to the thread.
- The composer shows **your quote**, not the retrieval dump.
- The engine expands (`@file`, grep, embeddings, **import graph** for code).
- Answers **cite** paths. You did not paste the neighborhood.
- A reply does not become the file until you take a write path (diff / apply).

That is the feel to copy. The human action is “this is what I mean.” Enrichment is **internal**. The model sees a **working set**; the user still sees the pin they chose.

On a wiki, the analog of the import graph is [LifeIndexing](../design/Agents/LifeIndexing.md): direct links + logical heading binds (`depends-on`, `constrains`, `contradicts`), packed at a **git SHA**. Cursor is strong at that for **code** (real name resolution). For markdown in a repo it is mostly files + search. Venus’s extra is the spec graph, not a friendlier chatbot.

## Table

| | Notion Agent | Cursor chat | Venus AB2 (ask) | Venus AB3 (chat-edit) |
|---|---|---|---|---|
| Point at a span | Selected blocks | Selection / `@` | Selection → `blockId` | Same Bind |
| Extra context | `@` page, search, connectors | `@` files, codebase, tools | Direct + logical graphs | Same pack on the draft |
| Why / history | Page history blob | Why in the **chat** | Empty until **AB4 after M6**; then comment-commit why + comments | Same chain on the draft |
| Clock | Live workspace | Files on disk / chat | `last_indexed` SHA | Lease `T0` |
| What the user sees | Chat; often edits | Quote + citations | Quote; pack hidden | Quote + hunks on Before |
| After the answer | May edit the page | May edit code | No write | `putHunks` → meaning-accept |
| Index | Workspace search / mentions | Repo + import graph (code) | Gists + heading binds on a snapshot SHA | Same |

## How the multidimensional graph compares

The graph is what makes Venus a different **LLM wiki**, not a nicer copilot. Notion and Cursor each own **one** axis and leave the other empty.

| | Spatial (what binds to this clause) | Temporal (why it is this way) |
|---|---|---|
| **Notion** | Page graph, `@`, DB relations, synced blocks. Agent: current page + selection + search. Similar blocks count as related. | Version history / activity — a **blob**. Undo the page, not “this accept.” No `decided-in` on a heading. |
| **Cursor** | Strong for **code** (imports, LSP, `@file`). For markdown: files + grep + embeddings. No heading-level `constrains` / `contradicts`. | `git log` / blame. **Why lives in chat**, not on the hunk. Next session does not pack the accepted rationale for this paragraph. |
| **CodeSpeak** | Requirements mapped to **code**, not wiki headings. | Intent extracted from **chats**; spec follows the body. Opposite pulse ([comparisons — CodeSpeak](./comparisons.md#versus-codespeak)). |
| **Obsidian-like** | Backlinks — cheap **direct** graph. No collab CRDT, no lease. | Git plugin at best; not meaning-accept. |
| **Venus** | Direct links **plus** logical heading binds at a **git SHA**, sidecar ids. | Same ids → **comment-commit why + rail comments** (`decided-in`, `supersedes`, `reverts`). Ask evolution; name the SHA to revert. |

Notion Agent is **human-wiki-native**: live page, search, then type. Cursor is **code-native**: working set, then diff. Venus’s graph is **spec-native**: neighborhood **and** accepted history on the same pin.

Workspace RAG / GraphRAG-style entity graphs are spatial **recall**. They are not clocked to `T0` and they do not store human meaning-accept as nodes. Do not fake `constrains` / `contradicts` with cosine similarity. On a spec, similar prose is often the **other** design.

**Trap:** snapshots (`snapshot: <title>`) do not feed the temporal axis. If contract changes skip comment-commit, the why graph is empty — Notion history with extra latency. The feature is only real where humans **accept why** ([pains §7](../product/pains.md#7-why-is-it-designed-this-way)).

## LLM wiki design (not ops, not IDE)

In **that** domain — a wiki LLMs and humans share as the spec, PMs write, agents clone, design evolves — Venus is the right architecture **if** dual store + lease **and** this graph ship. Notion and Cursor each win a neighbor and lose here.

**Versus Notion.** Ops (databases, standups, teammate types the page): Notion wins. LLM **wiki design**: Notion is the wrong object — live CRDT, no cloneable SHA pack, similar prose mistaken for the same decision, history without meaning. Bound chat + spatial binds beat page+`@`+search for “if I change freeze, what else is in force?” Temporal why is what Notion cannot copy without becoming a review product. AB3 must not type the live page or that advantage is donated back.

The graph does **not** get worse if you stream into the live CRDT. The **draft** can still see `constrains`. What you throw away is the **clock**: pack is `last_indexed` / `T0`, the tree has moved, no hunk to accept against. You kept Venus latency and lost the loop Notion cannot copy.

**Versus Cursor.** Cursor remains the inner loop (**implementation**). For a **docs folder**, Cursor-on-git is a decent LLM-wiki **read** (grep, `@file`, dump-if-small) and a weak **design memory** (why in chat). Venus is the better wiki when PMs will not live in the IDE, when you need heading-level binds not file-level `@`, and when the next agent must not re-litigate freeze. Add-to-chat is the right **gesture**; graph + lease is the right **object**. Chat-edit should feel like Composer on frozen `T0`, not Agent in Notion.

**Aider / CodeGraph CLI** are not in that inner loop as authors. In Venus they **review the PR or branch against the spec** — landed feature bound to plan, plan bound to docs: mistakes, follow the specification, closeness to the goal ([product-plan](../product/product-plan.md#spec-bound-review), [comparisons](./comparisons.md#spec-bound-pr-review)). Do not sell them as “Venus writes code.”

**Not yet better until shipped.** Spatial (M3 + AB1) is “what else is in force,” not pain 7. Why (M6 + **AB4**) cannot exist until comment-commits exist. Until both axes ship, this is a design claim. A small wiki still dumps into context. Noisy LLM `contradicts` edges are worse than grep. Dual store without the graph is still a real product (honest git + meaning-accept); it is not yet a better **LLM wiki** than Cursor-on-`wiki/` plus Notion for the PM.

**One line:** Notion owns the living page; Cursor owns the code working set; Venus wins LLM wiki **design** only as a **user-to-agent spec graph** — spatial binds + accepted timeline — on honest git.

## Why / timeline (main feature — after M6 / AB4)

The spec graph is **multidimensional** — not an add-on to chat. Spatial *and* temporal. Product: [Agents](../design/Agents/README.md), [product-plan](../product/product-plan.md#multidimensional-spec-graph). Do not sell spatial AB1 as this feature. AB4 cannot exist until M6.

The pain it closes: people and agents **cannot understand why it is designed this way**. The clause is silent. Notion history is a slider. Cursor why died in last week’s thread. A new PM re-argues freeze. You cannot name the accept to roll back. [pains §7](../product/pains.md#7-why-is-it-designed-this-way).

Ask that question on a bound heading and the pack is the **logically explained diffs**: required whys, pinned comments, which SHA superseded which — not a history blob. That is design evolution, and the SHA for M8 revert.

Snapshot autocomments and hidden change-gists are *what moved*, not why. Do not sell those as rationale. Honest only after comment-commits exist ([AB4](../design/Agents/agentic-binding.md#ab4--history--why-pack)).

## Chat-edit (AB3)

**Starts at checkout (M5–M6), not when bound chat exists.** Ask-only AB2 can sit for a long time with no edit control. Chat-edit is the Cursor apply analog: the same thread may propose markdown for the bound spans → hunks vs `T0` → meaning-accept. Not Notion (tokens land on the live block tree).

Pack still rides along (spatial + temporal), so a rewrite of freeze can see lease **and** the old why. Apply is [M6](../design/venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks), not the spectator pane.

## What to ship vs what to refuse

**Ship (Cursor-shaped ask on a wiki):** highlight markdown or WYSIWYG blocks → add to chat → Bind `{ sha, docId, blockIds }` → expand LifeIndexing pack → model cites headings. Composer shows the quote. Lag vs live is visible.

**Ship (AB3, Cursor-shaped edit, after M5–M6):** same thread proposes markdown vs `T0` → hunks on Before → meaning-accept. Not an AB2 follow-on. Pack still includes `constrains` / `contradicts` / `decided-in`.

**Ship (AB4, why pack, after M6):** bound heading → `decided-in` chain (human why + rail comments). Not an AB1 follow-on. Snapshots stay `changed-at` only. Revert target is a SHA, not a live undo.

**Ship (AB5, spec-bound PR review):** after Cursor implements, CodeGraph CLI and/or Aider review **that PR or branch** with the pack **landed ↔ plan ↔ docs**. Comments: mistakes, did we follow the spec, how close to the goal. Not a product commit. Not the implementer. Not a GitHub bot without the bind.

**Refuse (Notion-copilot shaped):** workspace-wide “ask my wiki” with no pin; agent types the published page from the thread; gists as page body; vector “related” as a bind; pack dumped into the composer; an agent that **uses the human wiki UI** as if that were native.

**Refuse (Aider / CodeGraph as implementer):** those CLIs as the coding agent, as in-wiki coding, or as a second Cursor. They review the loop’s PR or branch only.

AB1 is an internal index, not a UI. Notion indexes the workspace for Q&A; Cursor indexes the repo. Neither uses snapshot SHA as the bind clock, and neither attaches **accepted why** to the clause. That clock + that why is the LLM-wiki difference.

## Files

| File | Role |
|---|---|
| [agentic-comparison.md](./agentic-comparison.md) | This note |
| [comparisons.md](./comparisons.md) | Full vs Linear / Notion / Cursor / CodeSpeak (write loop) |
| [Agents](../design/Agents/README.md) | Main feature: multidimensional graph |
| [agentic-binding](../design/Agents/agentic-binding.md) | AB1 → AB4 plan |
| [LifeIndexing](../design/Agents/LifeIndexing.md) | Index contract |
| [pains §7](../product/pains.md#7-why-is-it-designed-this-way) | Why / evolution pain |
| [product-plan — Workspace and Aider](../product/product-plan.md#workspace-and-aider) | Two gits. **Spec-bound PR review** (landed ↔ plan ↔ docs). Aider / CodeGraph: not implement |
