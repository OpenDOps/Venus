# Code bind — two gits + code analyzer

**Status:** design. Product: [product-plan — Workspace and Aider](../../product/product-plan.md#workspace-and-aider). Plan slice: [agentic-binding — AB5](./agentic-binding.md#ab5--code-bind--aider). Wiki graph stays [LifeIndexing](./LifeIndexing.md). Wiki vs product trees: [venus-plan §2](../../drafts/pre-design/venus-plan.md#2-wiki-repo-vs-product-repo).

A Venus **workspace** pins **two** git clocks. The **code analyzer** on the product clock is **CodeGraph CLI, Aider, or both** ([select](#select-codegraph-cli-or-aider-or-both)). It is not the wiki copilot, not the implementer, and not a pack on every spec ask.

## Two remotes (lean)

Specs, plans, DoD, aims live in the **wiki** git tree. Application code and implementation PRs live in the **product** git tree. Histories stay separate.

| Tree | What | Clock |
|---|---|---|
| **Wiki** | Spec / plan / DoD / aims | `wikiSha` (`last_indexed` / lease `T0`) |
| **Product** | Application code, **named branch** | `productSha` (the pin, not the dirty working tree) |

**Lean: separate remotes, or submodules, with separate histories** (venus-plan option **C**). Parent workspace repo; wiki and product are submodules (or product is parent and wiki is a submodule). One clone for agents. Product `git blame` is not spec snapshots. Product PRs stay ordinary.

Do **not** leave the remotes unbound (yaml stores a URL, nobody updates the pin). Do **not** mix wiki commits into the product’s history as the default.

Workspace record:

```text
Workspace
  wikiRemote
  productRemote
  productBranch
  wikiSha          // last_indexed or T0
  productSha       // pin of that branch
```

Parent-vs-nested (workspace-of-two vs product-owns-wiki) is still chosen when the first real product repo is wired. The **split** is not.

## Select: CodeGraph CLI or Aider or both

**Gate before AB5 ships a binary.** The jobs are fixed (map/query at `productSha`; review the step diff). The tool is not.

| Option | What you get | Cost |
|---|---|---|
| **Aider only** | Repo map (tree-sitter + PageRank, token budget) + LLM review of the diff | One CLI. Map is a packed skeleton, **not** a queryable graph. No honest `who calls this` / blast radius unless the model guesses. |
| **CodeGraph CLI only** | Queryable symbol/call/import graph at `productSha` (callers, callees, impact). Pack from graph queries. Review = graph impact + a cheap model, or comments you write from CLI output | No Aider agent. You own the review prompt. Several products are named “codegraph” — **pin one CLI** in the AB5 step plan. |
| **Both** (lean default) | CodeGraph = structural truth (query, `implements`, impact of the diff). Aider = LLM **review** of that diff, fed the graph (or its map), not a second live index | Two externals. Runner kicks both. Do not run two copilots in the wiki. |

**Lean: both, split jobs.** CodeGraph is the code analog of LifeIndexing (edges you can ask). Aider is better at **writing** a review of a hunk. They overlap if both dump a map into every AB2 turn — do not.

CodeGraph candidate to bake off when AB5 opens: [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) (local SQLite graph, CLI + MCP, `explore` / impact). Other CLIs exist (`@optave/codegraph`, codegraph-ai). The plan selects **a** CodeGraph CLI, not the word “codegraph.” Index **after checkout of `productSha`**, not the dirty worktree, same clock rule as Aider.

| Job | CodeGraph CLI | Aider |
|---|---|---|
| Queryable graph / impact / `implements` | **Yes** (this is the reason to take it) | No — packed map only |
| Token-budgeted orientation blob | `context` / explore dump | **Repo map** |
| LLM review of `productSha` → `productSha'` | Not its job | **Yes** |
| Implement / publish spec / wiki chat | **No** | **No** |

Pick **Aider only** if a second binary is refused. Pick **CodeGraph only** if Aider’s agent is refused and review is graph + model we already pay for. **Do not** leave this unset when the runner first kicks a code analyzer.

Record on the workspace: `codeAnalyzer: aider | codegraph | both` plus pinned CLI versions.

## What the analyzer is for

**PR or branch review in the agentic loop — not implementation.** Cursor writes the product branch / PR. CodeGraph CLI and/or Aider analyze and comment on **that** PR or branch at `productSha`. They do not implement. Not embeddings as the bind. Not Cursor’s inner loop.

Two jobs only (both on the implementer’s PR/branch, never as author):

| Job | When | Input | Output |
|---|---|---|---|
| **Map / graph** | Optional `expand_bind` when the recipe asks | Product tree **at `productSha`** | CodeGraph: queryable graph + cited symbols. Aider: compact repo map. Not dumped into the composer. |
| **Diff review** | After a **plan step** has implemented (product SHA moved) | `git diff` + CodeGraph impact and/or Aider map | Automated **code review**. Comments on the step / PR. Not a merge. Not a spec write. |

Cursor remains the implementer. Neither CodeGraph nor Aider types product code as the runner’s implementer, or types the live wiki.

## Automated review (after implement)

Plan step cycle on the **product** diff. The review pack is **spec-bound**, not a repo dump. Product: [spec-bound review](../../product/product-plan.md#spec-bound-review).

```text
implement (Cursor on product repo → branch / PR)
    → productSha'  (the PR or branch tip)
    → pack: landed feature bound to plan step bound to spec/docs at wikiSha
    → CodeGraph (if selected): index/query at that SHA; impact of the PR/branch diff; `implements` vs headings
    → Aider (if selected): review that PR/branch **against that pack** (+ graph or repo map)
    → review artifact (PR/branch comments): mistakes **and** follow-the-spec **and** closeness to the plan’s goal
    → tests → breakpoint → done
```

The runner **kicks** the selected analyzer(s) the same way it kicks CI. Venus does not host Aider’s REPL or CodeGraph MCP as wiki chat. Checkout is the product submodule/remote at the step’s SHAs.

- Review is **read**. It does not `git commit` the product and does not `putHunks` on the wiki.
- **Validate code against the specification.** Bug-hunt alone is Cursor’s review. “Looks fine” while the plan’s goal is unmet is a fail of this feature.
- If the analyzer says the code drifted from a bound heading, that is a **contradiction for a human** (spec lease or another implement pass) — not CodeSpeak (silently update the spec from HEAD) and not Notion (agent types the page).
- v1: always **run** the selected analyzer; a bad review is a **warning** on the step (same softness as two-agent DoD). Later: may fail the step. Do not add a human click that makes the loop longer than Notion + a PR. Merge check stays **spec current or skip**, not “analyzer approved.”

Do not run this on every wiki keystroke or every AB2 turn. Trigger is **plan step implement done** (product SHA moved for that step).

## Optional pack (not every wiki request)

Bind is still a **spec** pin. Code rides along only when the recipe asks.

```text
Bind { wikiSha, productSha?, docId, blockIds }
    → expand spec pack (spatial + AB4 decided-in)
    → expand code pack only if:
         heading has implements / satisfies at productSha
         or the user asked whether the code matches
         or this is a plan step with a product diff
    → CodeGraph graph and/or Aider map (+ cited files), budgeted
```

New edge (later): **`implements`** — heading → path/symbol at `productSha`. Deterministic first (CodeGraph callers/imports, test names, path mentions). Do not fake it with cosine of prose to source.

When spec and code disagree, the model **surfaces** it. It does not publish either side.

## vs Notion, vs Cursor

| | Notion + GitHub connector | Cursor | Venus + analyzer |
|---|---|---|---|
| Object | Live page + “All sources” | Product files | Spec heading at `wikiSha` + code at `productSha`; **PR bound to plan bound to docs** |
| Clock | None | Disk / chat | Two pins |
| Map / graph | n/a | Embeddings + agent open | CodeGraph query and/or Aider repo map + step diff |
| PR review | Connector dump; no spec SHA | Diff + `@` context. Bug-hunt | **Against the spec:** mistakes, follow the docs, closeness to the plan’s goal |
| Writes the wiki | Agent may type the page | No | Lease + meaning-accept only |
| Writes the product | No | Yes (implementer) | Cursor. Analyzer **reviews** the diff |
| Every wiki ask includes code | Connector dump | n/a | **No** |

Notion already wins “ask the workspace and also search GitHub.” Cursor already wins “review this diff.” Venus wins only if code context is **clocked, bound to the plan and the spec, and not allowed to publish** — and the review asks how close we are to that goal.

CodeGraph is stronger as a **queryable** graph (impact, callers). Aider is stronger at **LLM review** of a diff and at a packed map. Cursor is stronger at semantic find, LSP, and the daily edit loop. Select per [above](#select-codegraph-cli-or-aider-or-both). Do not replace Cursor.

## Spine

| When | What |
|---|---|
| **After M4** | Record workspace remotes + `productBranch`. Dogfood wiki still does not wait on the analyzer. **Do not delay M5.** |
| **Before AB5 code** | **Select** CodeGraph CLI, Aider, or both. Pin CLI versions. |
| **AB5** | Analyzer at `productSha`; optional `expand_bind` code pack; runner hook for diff review. Needs a product pin and [AB1](./agentic-binding.md#ab1--lifeindexing) if the pack is spec+code. Review-after-implement needs the [plan runner](../../product/product-plan.md#2-plan-runner-orchestrator). |
| **Not M3–M8 exit** | Wiki spine stays wiki. Analyzer is not convert, not `last_flushed`, not lease apply. |

Until M3 has `wiki/` SHAs there is nothing to bind. Until a product remote is pinned, the analyzer has no honest `productSha`.

## Do not

- Enrich **every** wiki request with the codebase (Notion connector).
- Put Aider, CodeGraph MCP, or Cursor **in** the page as a teammate that types CRDT.
- Use Aider or CodeGraph as the **implementer** (Cursor already won).
- Index or map the **live** product working tree; clock is `productSha`.
- Run **two live indexes** of the same dirty tree. If both: CodeGraph at the pin; Aider review consumes that (or a map of the same SHA).
- Delay `last_flushed` / pin cut on the analyzer ([LifeIndexing invariant](./LifeIndexing.md#invariant)).
- Treat analyzer review as meaning-accept or as merge green.
- Run analyzer review as a **bug-hunt only** (no plan, no spec pack). That is Cursor. The feature is **landed ↔ plan ↔ docs**.
- Update the spec from code without a human (CodeSpeak pulse).
- Mix wiki history into the product repo as the default layout.
- Open AB5 without recording `codeAnalyzer: aider | codegraph | both`.

## Files

| File | Role |
|---|---|
| [code-bind.md](./code-bind.md) | This design |
| [agentic-binding.md](./agentic-binding.md) | AB5 |
| [product-plan](../../product/product-plan.md#workspace-and-aider) | Flow, runner step, keep/cut |
| [venus-plan §2](../../drafts/pre-design/venus-plan.md#2-wiki-repo-vs-product-repo) | Two trees; lean C |
| [implementation plan](../venus-implementation-plan.md) | After M4 / after M8 pointers |
| [LifeIndexing](./LifeIndexing.md) | Spec graph; code is not this index |
