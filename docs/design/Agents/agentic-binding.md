# Agentic binding

**Status:** high-level plan (parallel track) for the **main wiki feature** — multidimensional spec graph + bound chat. Product: [Agents README](./README.md), [product-plan](../../product/product-plan.md#multidimensional-spec-graph). Not M0–M8. Not the product orchestrator ([product-plan](../../product/product-plan.md) — lease MCP, runner, accept inbox). **AB5** (two gits + code analyzer on the product clock) is this folder’s code axis: [code-bind.md](./code-bind.md). **Select** CodeGraph CLI, Aider, or both before AB5 ships.

Humans **point** at spec (a markdown selection). Venus **binds** neighborhood from [LifeIndexing](./LifeIndexing.md). The chat model sees the pack; the user still sees the quote they added. **AB2** replies are not the spec and do not apply. **AB3** drafts markdown from the same thread; publish is still lease + meaning-accept, not typing the live CRDT. **AB4** packs **why** (comment-commits + comments) on the same heading ids. **AB5** may add a **product** pin (`productSha`) and CodeGraph / Aider analysis; it does not dump the codebase on every ask.

| Milestone | Name | Depends on | Design |
|---|---|---|---|
| **AB1** | LifeIndexing (spatial reindex on snapshot) | **[M3](../venus-implementation-plan.md#m3--git-snapshotter-week)** closed (`wiki/` SHA + sidecar on disk). **Not** pain 7 / not the main feature — that needs AB4. | [LifeIndexing.md](./LifeIndexing.md) |
| **AB2** | Bound chat (ask-only: pack → answer; **no wiki write**) | **AB1**. Selection: [M2 pane](../M2/README.md) and/or WYSIWYG. Cross-page binds get better after [M4](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks). Not a write path. Spatial pack only until AB4. | This file |
| **AB3** | Chat-edit markdown (lease, not live type) | **[M5](../venus-implementation-plan.md#m5--lease--freeze-week)** + **[M6](../venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks)** checkout (acquire, `T0`, hunks, apply, meaning-accept). Reuses the AB2 composer if it exists; **does not start when AB2 ships.** | This file |
| **AB4** | History / why pack (temporal graph) | **[M6](../venus-implementation-plan.md#m6--comment-commit-markdown-only-2-weeks)** comment-commits (real why). Writes into the AB1 index. **Does not start when AB1/AB2 ship.** Does **not** wait for AB3. [M8](../venus-implementation-plan.md#m8--revert--agent-loop-week) revert uses the same chain. | [LifeIndexing — temporal](./LifeIndexing.md#temporal-graph-commits--comments) |
| **AB5** | Code bind + analyzer | Workspace: two remotes / submodules ([code-bind](./code-bind.md)). **Select** CodeGraph CLI, Aider, or both. Map/graph: `productSha`. Review: after plan-step **implement**. Optional pack on AB2. Does **not** wait for AB3. Does **not** delay [M5](../venus-implementation-plan.md#m5--lease--freeze-week). Runner hook: [product-plan](../../product/product-plan.md#2-plan-runner-orchestrator). | [code-bind.md](./code-bind.md) |

Do not start AB1 while M3 has no `wiki/` writer. Do not start AB2 before `last_indexed` exists for the open page. **AB2 must not write the wiki** (not CRDT, not git, not `putHunks`). Do not start AB3 until **M5–M6 checkout** is honest (lease + apply fixtures) — not because AB2 exists. Do not start AB4 until **M6** has real comment-commit whys — not because AB1 exists. Do not start AB5 before a product remote is pinned (and, for spec+code pack, AB1) **and** `codeAnalyzer` is recorded. Do not put AB1 on the pin cut ([LiveSnapshot](../LiveSnapshot/README.md)). Do not implement AB3 as Notion Agent (stream into the published page). Do not implement AB5 as “Aider/CodeGraph in the wiki” or as the implementer.

## Principle

```text
human selects a span          ← what they meant
        │
        ▼
Bind { sha, docId, blockIds } ← clocked pin of that point
        │
        ▼
pack = LifeIndexing expand    ← spatial binds + (AB4) commit/comment why
                                 + (AB5) CodeGraph / Aider pack only if the recipe asks
        │
        ▼
chat turn = user text + pack  ← model answers about the selection
```

The user action is “add this to chat.” Enrichment is **internal**. Do not dump the whole pack into the composer. Do not make humans paste other pages.

This chrome is **user-to-agent**, not agent-as-teammate-in-the-page. The environment (SHA, sidecar, graphs, lease) is agentic-native; the human points. [product-plan](../../product/product-plan.md#agentic-native-user-to-agent).

**Citation vs pack:** the quote in the thread is the **live** selection (what they highlighted). The pack is **`last_indexed` SHA** (same lag as clone / snapshot). If live has moved past that SHA, the bind records both clocks; the model is told the pack may lag. Do not mix pane splice offsets with stored graph edges ([LifeIndexing — invariant](./LifeIndexing.md#invariant)).

## Relation to the spine

| Spine | What binding uses |
|---|---|
| **M2** | RAM sidecar + read-only markdown pane (selection of source). Prefer BlockSuite **block ids** from WYSIWYG when available; pane UTF-16 → RAM sidecar is fallback. |
| **M3** | Git SHA, `.venus/ids/`, dirty set, enqueue after `last_flushed`. **AB1 starts here.** |
| **M4** | Catalog `gitPath`, linked-doc titles — direct graph across pages. AB2 may ship on one page before this. **Record** wiki + product remotes (AB5); not M4 exit. |
| **M5** | Lease `T0`: pack at that pin’s SHA. **AB2** does not unfreeze. **AB3** **acquires** the lease (freeze is the write contract). |
| **M6–M8** | Apply / lease holder stay the write path. AB2 does **not** `putHunks`. **AB3** is a lease **holder** in the host chat. **AB4 starts here** (indexes comment-commits + rail comments). M8 revert is the write path when the pack names a bad `decided-in`. |
| **Product-plan MCP** | Same write tools. Bind protocol (`bind_*`) is read. AB3 host chat calls lease tools; it does not invent a third apply. |
| **AB5 / runner** | Product checkout at `productSha`. CodeGraph CLI and/or Aider is an **external** analyzer (graph/map, step diff). **Select** first. Cursor remains implementer. |

“Managers never need Cursor” ([product-plan](../../product/product-plan.md#force-these-or-it-is-not-the-flow)): bound chat is **Venus chrome** (PM asks the spec). Programmers still clone `wiki/`. Chat is not a second share format.

## AB1 — LifeIndexing

Index the published wiki **after** each snapshot commit. Incremental on dirty `docId`s; periodic full re-gist when rolling summaries drift. Direct (links) graph + logical (LLM) graph + component/domain tags.

**Gate:** [M3](../venus-implementation-plan.md#m3--git-snapshotter-week) exit — clone `wiki/`, autocomment snapshots, typing during flush still syncs. HA: [high-availability.md](../LiveSnapshot/high-availability.md) (accepted). AB1 is **not** M3 exit. AB1 is **spatial only** (“what else is in force”). It is **not** [pains §7](../../product/pains.md#7-why-is-it-designed-this-way) and not the main wiki feature until AB4.

**Exit**

- After idle/flush, dirty pages get gists + tags; `last_indexed` moves to that SHA without delaying `last_flushed`.
- Direct graph: outbound `links-to` / containment for dirty `.md`; reverse index for inbound.
- Logical graph: heading binds for dirty headings against the compact map; unchanged pages stay targets.
- Drift path: N incremental gists (start 8) or fat diff → full content re-gist of that file.
- Snapshot git message still `snapshot: <title>`. No LLM on convert.

Contract: [LifeIndexing.md](./LifeIndexing.md). Cheap JSON completions are a **separate** model key, not the Cursor agent harness.

## AB2 — Bound chat

Select any span in markdown (or the matching WYSIWYG blocks), add it to a chat turn. The engine binds context from AB1 and sends **user text + pack** to an LLM. The reply is grounded in those selections (citations to `docId` / `blockId` / heading).

**This is an enrichment test, not a write path.** The agent does **not** change the wiki. You are measuring whether answers improve when the model sees the bound pack vs the quote alone. That is **not** Notion-with-a-copilot. Notion Agent’s tell is the teammate **typing the live page**. A pinned ask that cannot apply is Cursor **add-to-chat** on a spec SHA.

Ask-only is still easy to **mis-sell**:

| Risk | What to do |
|---|---|
| Empty **why** until M6 | Pack spatial binds + `changed-at` only. Do not call snapshot autocomment “rationale.” AB4 comes later. |
| Users will ask “apply that” | No apply control. Reply is not the spec. AB3 is a **different** milestone, gated on checkout. |
| One-page wiki ≈ dump | Still require `bind_selection` then `expand_bind`. Dump-after-pin is allowed when small; dump-without-pin is not. |
| Chat becomes the home screen | Tree + WYSIWYG stay the product. Ask is a tool. |
| Opportunity cost vs M4–M6 | Allowed after AB1; **not** a reason to slip lease/apply. |

```text
pane / editor selection
    → resolve blockIds (sidecar)
    → Bind.create(sha, selections[])
    → pack = expand(Bind)     // LifeIndexing recipe
    → LLM(user message, pack)
    → reply + citations
```

**Pack recipe** (same as [LifeIndexing — selection](./LifeIndexing.md#selection-and-agent-pack)):

1. Selected blocks + containing heading + page title
2. Direct: `contains`, `links-to` / `transcludes` 1-hop, inbound
3. Logical: `depends-on` / `constrains`; surface `contradicts`
4. **AB4 (after M6):** `decided-in` (why + comments) for those headings; until then snapshots only as `changed-at`
5. **AB5 (optional):** CodeGraph graph and/or Aider map + cited product files at `productSha` — only if `implements` / user asked code / this is a plan-step diff. Not every turn.
6. Gists of those nodes; tags as hints
7. Stop at a token budget. Do not dump the wiki unless it still fits **after** the bound set. Do not dump the product repo.

**UI**

- Composer shows the **quote** (and path / heading), not the graph.
- Optional: a small “also in context” count or heading list the user can open. Not a second wiki.
- Reply cites headings; click-through to the page. No mute apply from the answer.

**Exit**

- Selecting a span and sending a message produces an answer that can name the bound headings (not only the quoted sentence).
- A second selection on another heading (same or other page, if M4 exists) adds a second bind to the same turn.
- If the index lags the live quote, the turn still sends; the pack is last_indexed and the lag is visible.
- No write to published CRDT or git from this chat. **No apply / putHunks / “insert this” control.** If the model proposes wording, it stays in the thread.

### Bind protocol (MCP-shaped)

One **Bind** object shared by in-app chat and, later, Cursor MCP. Wire can be MCP tools; the shape is the product, not “chat pasted a prompt.”

| Piece | Job |
|---|---|
| **Resource** `venus://wiki/{sha}/{docId}` | Published markdown + sidecar at a SHA (git HEAD or `T0`) |
| **Resource** `venus://index/{sha}` | Compact map (paths, headings, gists, tags) |
| **Resource** `venus://code/{productSha}` | Product tree at the pinned SHA (AB5). Not live worktree. |
| **Tool** `bind_selection` | `{ sha, docId, blockIds[] }` → `bindId` (pin of the point). May record `productSha` from the workspace. |
| **Tool** `expand_bind` | `bindId` → pack (direct + logical + gists; optional CodeGraph / Aider), budgeted |
| **Tool** `read_heading` | `{ sha, docId, blockId }` → heading body (when pack needs a claim, not only a gist) |
| **Tool** `expand_history` | `{ bindId }` or `{ docId, blockId }` → `decided-in` / `changed-at` chain + comments (AB4) |

Host chat calls these **inside** Venus (same process or local MCP). External agents later call the same tools **read-only**. **AB3** does not add a second apply: it calls the existing lease API (`acquire` / `putHunks` / comment). Bind stays read; lease stays write.

Do not implement Bind as “stuff all markdown into the system prompt” without `bind_selection`. Dump-with-a-map remains allowed **inside** `expand_bind` when the wiki is small, **after** the selection is pinned.

## AB3 — Chat-edit markdown

**Gate: M5–M6 checkout**, not “AB2 shipped.” Ask-only enrichment can exist for a long time with no edit button. Chat-edit is the Cursor **apply** analog: private buffer → diff vs `T0` → meaning-accept. Not Notion (tokens on the live tree).

The same composer as AB2, if it exists, may grow a **propose edit** turn. That growth is this milestone. Reuses Bind + pack; calls lease tools. Does not invent a third apply.

```text
AB2 thread + Bind
    → user: change this (and keep constrains from the pack)
    → model proposes markdown vs T0 (sidecar ids)
    → acquire lease if needed (freeze published)
    → putHunks (diff vs markdown_T0)
    → After / Before / Diff + required why
    → human meaning-accept → git comment-commit
```

The pack from AB1 is why this is better than Notion-in-place edit: `contradicts` / `constrains` ride with the selection, so a rewrite of freeze can see lease. The lease is why this is not Notion: the published CRDT does not move until accept.

**Depends on:** **M5–M6** (freeze, `T0`, hunks, apply fixtures green). Reuses AB2 Bind/composer if present; if AB2 is not built yet, AB3 still waits on checkout (it is not a shortcut around lease). Do not fake AB3 with pane splice or `Y.applyUpdate` of `toDoc`. Casual WYSIWYG typing stays snapshot+autocomment; chat-edit of **spec meaning** is comment-commit. Typos the human could have typed in WYSIWYG are not this milestone.

**UI**

- Same composer as AB2. A turn may be ask-only (AB2) or **propose edit**.
- Proposed hunks show on Before like any lease holder. User can reject, comment, regenerate — not mute apply from the bubble.
- Chat transcript is not the spec. The accepted why + hunks are.

**Exit**

- From a bound selection, “rewrite this to match heading X” yields hunks attributed to sidecar ids, reviewable on After/Before, not a live page rewrite.
- Bound `constrains` / `contradicts` appear in the pack the model used (same as AB2).
- Cancel / reject leaves published git unchanged. Accept is a comment-commit with a human why.
- No stream of tokens into the published Store.

**Lease tools (same as product-plan MCP; host chat is another client)**

| Piece | Job |
|---|---|
| `acquire` / heartbeat / release | Exclusive writer; `T0` pin |
| `putHunks` | Replace proposal bodies for this lease |
| submit + why | Meaning-accept path (human) |

Do not add `edit_page` that writes Yjs. Do not snapshot-autocomment a chat rewrite of contract.

## AB4 — History / why pack

**Gate: M6 comment-commits**, not “AB1 shipped.” Spatial AB1 without this pack is “what else is in force,” not [pains §7](../../product/pains.md#7-why-is-it-designed-this-way). Do not sell AB1 as the main feature. The main feature is spatial **and** temporal; temporal needs accepted why.

The logical graph is **multidimensional**. Spatial binds (AB1) link document parts. Temporal binds link those parts to **commits and comments**: logically explained diffs, not a page-history blob.

```text
“Why is it designed this way?”
    → Bind the heading
    → expand spatial neighborhood
    → expand_history: comment-commit whys + rail comments
    → model sees evolution; can name the SHA to revert (M8)
```

**Depends on:** **M6** (required why on comment-commits). Attaches to the AB1 index (heading ids at SHAs). Snapshot-only timeline is `changed-at` without rationale — not this milestone. Does **not** wait for AB3; does **not** start because AB1/AB2 exist. Makes AB2’s ask honest for design work once why exists.

**Exit**

- Asking why a bound heading is as it is returns **human** comment-commit messages and pinned comments, not `snapshot: <title>` and not an LLM change-gist sold as why.
- The pack can point at the commit where a decision landed so revert (M8) has a target, not “undo the live page.”
- Spatial and temporal edges stay in one index, keyed by `docId` + `blockId` + `gitSha`.

Contract: [LifeIndexing — temporal](./LifeIndexing.md#temporal-graph-commits--comments).

## AB5 — Code bind + analyzer

Two git clocks, CodeGraph CLI and/or Aider on the **product** one. Contract: [code-bind.md](./code-bind.md). **Must select** one or both: [code-bind — select](./code-bind.md#select-codegraph-cli-or-aider-or-both). Product: [product-plan — Workspace and Aider](../../product/product-plan.md#workspace-and-aider).

**Lean layout:** separate remotes **or** submodules, **separate histories**. Wiki git is not the product git. Workspace pins `{ wikiRemote, productRemote, productBranch, wikiSha, productSha, codeAnalyzer }`.

**Aider and CodeGraph CLI review the agentic-loop PR or branch. They do not implement.** Cursor stays the implementer. Neither is a wiki copilot. Lean default remains both, split jobs (graph vs review comments). Pin the CodeGraph binary when this milestone opens.

**Automated review:** after Cursor **implements** (PR or branch at `productSha'`), the runner kicks the selected analyzer(s) on **that PR or branch** with a **spec-bound pack** (landed ↔ plan ↔ docs). Output is review comments: mistakes **and** follow-the-spec **and** closeness to the plan’s goal. Not a merge, not meaning-accept, not a spec write, **not implementation**. v1: always run; flags are a **warning**. Later may fail the step. Merge check remains spec current or skip. Product: [spec-bound review](../../product/product-plan.md#spec-bound-review).

**Optional AB2 pack:** `expand_bind` may include CodeGraph citations and/or the Aider map when the recipe asks. Do not enrich every wiki request (that is Notion + GitHub).

**Depends on:** workspace remotes (record after [M4](../venus-implementation-plan.md#m4--folder-tree--links--product-header-12-weeks); do not delay M5). **`codeAnalyzer` recorded** before any kick. Spec+code pack needs AB1. Diff review needs the plan runner ([product-plan](../../product/product-plan.md#2-plan-runner-orchestrator)). Does **not** wait for AB3. Does **not** belong on the M3 convert path.

**Exit**

- Workspace has two pins; clone is one checkout (submodules) or two remotes that Venus binds.
- `codeAnalyzer` is `aider` | `codegraph` | `both`; CLI versions pinned.
- Graph and/or map is produced at `productSha`, not the live dirty tree.
- After implement, a step has a **spec-bound** review of **that** diff (follow the docs, closeness to the goal — not bug-hunt only). The analyzer did not commit product or wiki.
- Bound chat without a code recipe still packs spec only.

## Order

```text
M3 wiki/ SHA
    → AB1 LifeIndexing (spatial)
        → AB2 bound chat (ask-only; no wiki write)
M5 lease + M6 apply (checkout)
    → AB3 chat-edit (same composer; lease holder — not “AB2 next”)
M6 comment-commits
    → AB4 history/why pack (enriches AB2; **not** after AB1; independent of AB3)
M8 revert           → write path when AB4 names a decided-in
M4 catalog / links  → richer direct graph; **record** wiki + product remotes (do not delay M5; analyzer is not M4 exit)
product-plan MCP    → Bind + expand_history + lease tools
plan runner         → **select** CodeGraph / Aider / both → AB5 review after implement
AB1 + productSha    → optional code pack on AB2
```

Step plans and boards (M2-style) come when a milestone is opened. This file is the accept bar and the dependency arrow.

## Do not

- Start AB1 before M3 `wiki/` exists, or fold it into M3 convert.
- Start AB2 before AB1 has an index row for the open `docId`.
- Start AB3 before **M5–M6 checkout** is honest (lease + apply fixtures), or by typing the published CRDT from the thread. Do not treat “AB2 exists” as the AB3 gate.
- Start AB4 before **M6** has real comment-commit whys, because AB1 exists, or pack snapshot autocomments / change-gists as rationale.
- Treat chat as the spec ([product-plan — chat as the spec](../../product/product-plan.md#do-not-add)). AB3 still requires meaning-accept. AB4’s why is those accepts, not the thread.
- Mute-apply a chat reply (skip hunks / skip why).
- Show gists as page content.
- Use Cursor subscription as the cheap indexer or as a generic `/chat/completions` for AB1.
- Put Aider or CodeGraph in the wiki as a teammate, dump the graph/map on every AB2 turn, or let either implement / publish spec ([code-bind — do not](./code-bind.md#do-not)).
- Open AB5 without `codeAnalyzer: aider | codegraph | both`.

## Files

| File | Role |
|---|---|
| [agentic-binding.md](./agentic-binding.md) | This plan (AB1 → AB5) |
| [LifeIndexing.md](./LifeIndexing.md) | AB1 spatial + AB4 temporal |
| [code-bind.md](./code-bind.md) | Two gits; select CodeGraph CLI / Aider / both; post-implement review |
| [agentic-comparison](../../marketing/agentic-comparison.md) | Ask, chat-edit, why/history vs Notion / Cursor |
| [Agents README](./README.md) | Folder map |
| [LiveSnapshot](../LiveSnapshot/README.md) | SHA AB1 consumes |
| [implementation plan](../venus-implementation-plan.md) | M3–M8 spine; this track is parallel |
| [product-plan](../../product/product-plan.md) | Lease MCP / runner — later, different tools |
