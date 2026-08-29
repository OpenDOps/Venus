# Venus plans: spec-driven development

Venus is not only an LLM wiki. The wiki is the **spec store**. Humans keep heart, instinct, faith and intention; agents do the work ([pitch.md](../../marketing/pitch.md)). The first-class loop is **spec → plan → DoD → implement → test → PR review → update spec** (and generated API/aim docs). Plan and DoD are wiki pages: same lease, freeze, commented diffs, and **human accept** as any spec. DoD is always written by a **different agent** than the one that implements (and than the one that drafted the plan). A story is not done until the spec diff is accepted. This note is that product surface. Editor, lease, and git rules stay in [venus-design.md](../../design/venus-design.md) and [v1-concerns.md](./v1-concerns.md).

The **board is not the product.** Workflow lives in `*.state.yaml` and on the plan page. GitHub PRs are the execution path. [Plane](https://github.com/makeplane/plane) (or GitHub Issues, or anything else) is only an optional **representation mirror** of that yaml. Venus does not fork Plane and does not exist to be a Plane client.

## Goal

A user (or agent) can point at a document, a group of documents, or a part of a document and ask: **prepare an implementation plan**. Venus writes that plan as markdown in a dedicated `plans/` tree, with steps that are tasks, each with **test scenarios as definition of done**. The plan is a **user story**; steps are **tasks**. A YAML sidecar is the board. External tools may **mirror** it; they are not the goal.

**Plan and DoD are not a special protocol.** They are spec-wiki writes: acquire lease, edit markdown, commented hunks, human accept ([venus-design.md](../../design/venus-design.md)). The runner must not start until that published plan includes DoD authored by another agent and accepted by a human.

An agent can then **run** the plan: implement → autotests → review. Review is a GitHub PR plus a PR-review tool; failure loops back to implementation. Several plans may run in parallel. After each step there is a **breakpoint**: human review, or an agentic pass. When the code is done, **spec docs are updated the same way** (lease + human accept). That gate is not optional.

```text
  spec wiki (lease + human accept)
           │  "plan this page / these pages / this section"
           ▼
  plan page: story + steps     ← planner agent, human accept
           │  second lease
           ▼
  plan page: DoD scenarios     ← **other** agent, human accept
           │
           ▼
  plans/*.state.yaml          ← canonical board
           │  optional mirrors (not the goal)
           ▼
  GitHub Issues / Plane / …
           │
           ▼
  implementer agent (≠ DoD author, ≠ planner)
           implement  →  autotests against published DoD  →  PR review  ⟲
           │
           ▼
  spec lease: doc update hunks  →  **human accept**
```

## Where plans live

Plans are first-class wiki pages (BlockSuite + git), but they live in a **separate folder**, not mixed into `spec/` / `design/`:

```text
wiki/
  spec/
    protocol.md
  design/
    overview.md
  plans/
    2026-08-lease-freeze.md
    2026-08-lease-freeze.state.yaml
```

- **`.md`** — story, scope, steps, DoD. Humans and LLMs read this. Same dual representation as any page: live CRDT, git snapshot, leased markdown edits.
- **`.state.yaml`** — machine board: step status, PR urls, breakpoint policy, optional foreign ids (GitHub, Plane). Not a second replica of the prose. Not stored on published spec blocks.

Identity: `planId` is stable (like `docId`). Git path can change. Links from a spec section to a plan use `planId` / `docId`, not the filename.

## Same write path as spec

A plan page is a wiki document. Creating it, filling DoD, and later changing either one uses **the same path as editing `wiki/spec/…`**:

1. Flush-before-lease if needed ([v1-concerns.md](./v1-concerns.md)).
2. Acquire markdown lease on the **plan** `docId` (published WYSIWYG on that page freezes).
3. Writer (human or agent) edits markdown; Venus diffs by block id; required review comment.
4. **Human accept** (or reject). Agentic review may comment; it does not apply.
5. CRDT + git review commit. Release lease.

There is no “patch DoD in yaml” and no “agent marks the plan ready.” Yaml stores **run** state (`state`, `pr`, `dodResults`). Scenario text lives only in the published plan markdown.

## Two agents, then a human (anti-theater)

If the same agent writes DoD and the tests, both will pass. Venus forbids that.

| Role | Who | Writes | Must be |
|---|---|---|---|
| **Planner** | Agent A | Story, scope, step titles / `dependsOn` / `kind`. `dod` empty or omitted. | Not the DoD author |
| **DoD author** | Agent B | `dod[].name` and `dod[].scenario` on those steps | **≠ planner**, **≠ implementer** |
| **Human** | User | Accept / reject / comment on each lease (plan, then DoD, then any later plan edit) | Required on every plan/DoD apply |
| **Implementer** | Agent C | Product-repo code and tests that encode the **already published** DoD | **≠ DoD author** (and ≠ planner) |

Hard rules:

- Implement **does not start** until the plan page has published DoD and a human-accepted review commit whose author of the DoD hunks is agent B.
- Record on the plan (frontmatter or yaml `authors`): `plannerAgentId`, `dodAgentId`, later `implementerAgentId`. Venus refuses a run if `dodAgentId` is missing or equals planner or implementer.
- A human may still edit hunks during review (same as spec). That does not make the implementer the DoD author. If the implementer needs DoD changed, that is a **new lease** by the DoD author (or a new agent that is still not the implementer), then human accept again. Yaml `dodResults` is wiped or marked stale until re-run.

## Creating a plan

Entry points (all produce one plan doc + yaml):

| Scope | Meaning |
|---|---|
| One document | Plan covers `wiki/spec/protocol.md` (that `docId`). |
| Group | Several `docId`s (e.g. protocol + lease). One story, steps may name which spec they implement. |
| Part | Anchor on a page: heading / block range / pin (`docId` + `blockId` or text range at a snapshot clock). |

Sequence (two leases, both human-accepted):

1. User asks (wiki or MCP). **Planner** leases the new plan page, writes story + steps **without** DoD. Human accepts. Spec pages are not frozen.
2. **DoD author** (different agent) leases the **same** plan page, adds scenarios. Human accepts. Plan is now runnable.
3. Specs freeze only later, when a step (or the closing docs step) edits those `docId`s.

Rejecting the DoD lease leaves a published plan with no run; the planner may iterate (another lease) and DoD author tries again. Same loop as rejecting a spec edit.

## Plan markdown (story + steps)

A plan is markdown with a **required step schema** (stable `id`, title, `dependsOn`, `kind`, DoD scenarios) so it can render as a task list and, later, be mirrored to an issue tracker. Prose around it is free. **How that schema is encoded in the `.md` is an open decision** ([§ Open decisions](#open-decisions)). The block below is a candidate only, not the v1 format.

Ad-hoc `- [ ]` alone cannot hold DoD and ids. Do not ship a custom fence language until BlockSuite markdown round-trip is boring ([v1-concerns.md](./v1-concerns.md)).

````markdown
---
planId: 8f3a…
title: Lease freeze on markdown acquire
scope:
  - docId: …          # protocol.md
    anchor: null      # whole doc
  - docId: …          # lease.md
    anchor: { blockId: b12 }   # one section
---

# Lease freeze

## Story

As a reviewer I need the published page frozen while a lease is held so hunks share one T0.

## Steps

```venus-step
id: step-snapshot
title: Snapshot T0 on acquire
dependsOn: []
# dod: filled in a second lease by a different agent, then human-accepted
dod:
  - name: clock matches
    scenario: |
      Given a published page with known Yjs state vector S
      When a lease is acquired
      Then snapshotClock equals S and markdown export is from that snapshot
  - name: no published writes
    scenario: |
      Given a held lease
      When a second client types in WYSIWYG
      Then the editor is read-only and the CRDT does not change
```

```venus-step
id: step-docs
title: Update spec docs to match shipped behavior
dependsOn: [step-snapshot, …]
kind: docs          # always human-accept on apply
dod:
  - name: design matches code
    scenario: |
      Given the merged implementation
      When spec pages named in scope are leased and updated
      Then a human accepts the review commit
```
````

`dod[].scenario` is the **accept rule** (definition of done). The implementer encodes those published scenarios as tests; they do not get to author them. A step is not done when the implementer says so; it is done when those scenarios pass (and the breakpoint is cleared).

`kind`: `implement` | `test` | `docs`. The closing docs step is required on every plan that changes behavior. `kind: docs` **cannot** use agentic accept.

## YAML board mirror

Canonical **workflow** state for a plan is the sidecar, not Plane and not markdown checkboxes.

```yaml
# plans/2026-08-lease-freeze.state.yaml
planId: 8f3a…
docId: …                    # BlockSuite / catalog id of the plan page
authors:
  plannerAgentId: agent-a
  dodAgentId: agent-b       # ≠ planner, ≠ implementer; required before run
  implementerAgentId: agent-c   # set when run starts; refused if = dodAgentId or plannerAgentId
dodAccepted:
  gitSha: …                 # review commit that published DoD
  acceptedBy: user-…
plane:
  workspace: acme
  projectId: …
  storyWorkItemId: …        # parent work item
columns:                    # Venus names; mirrors map these to Issue/Plane states
  implement: { planeStateId: … }
  autotests: { planeStateId: … }
  review: { planeStateId: … }
  done: { planeStateId: … }
steps:
  step-snapshot:
    state: review           # implement | autotests | review | done | blocked
    breakpoint: human       # human | agent | none
    planeWorkItemId: …
    pr: https://github.com/org/repo/pull/412
    dodResults:
      - name: clock matches
        status: pass
      - name: no published writes
        status: fail
        evidence: "…"
  step-docs:
    state: implement
    breakpoint: human       # forced for kind: docs
    planeWorkItemId: …
```

- **Git** stores the yaml on snapshot/review commits of the plan (same two commit classes as [v1-concerns.md](./v1-concerns.md)).
- Live runners may hold a small Venus session object for in-flight PRs; flush yaml before a human breakpoint so the clone matches the board.
- Markdown checkboxes, if shown, are a **view** of yaml `state` / DoD **results**, not a third source of truth. Scenario prose is only in the plan `.md`.
- Changing DoD is a plan-page lease (DoD author + human accept), never a silent yaml edit. After that commit, in-flight autotests are invalid until re-run.

## Board mirrors (Plane is not the goal)

The human board in v1 is the **plan page + `*.state.yaml`**. Execution is already **GitHub PRs**. That loop does not need Plane.

External trackers are **representation mirrors**: they show the same yaml columns so people who live in Issues or Plane can glance. They are not a source of truth, not a Venus differentiator, and not a v1.1 milestone.

| Priority | Surface | Why |
|---|---|---|
| Canonical | Plan markdown + `*.state.yaml` | Agents and clones run without any tracker |
| First mirror, if any | GitHub Issues (parent issue = plan, sub-issues = steps, `pr:` is native) | Same host as the PR review tool |
| Later mirror | [Plane](https://github.com/makeplane/plane) work items | Teams that already use Plane; AGPL; extra sync and UX |

Venus **must run with zero mirrors**. Do not use Plane Pages (or GitHub wiki) as the spec store.

If a mirror is wired, **yaml stays canonical.** Push state/title out; pull card moves back with a clock. In-flight agent runs win until a breakpoint. Foreign ids live on the yaml (`githubIssueId`, `planeWorkItemId`, …), not on published spec blocks.

Plane, if used later: [work-items API](https://developers.plane.so/api-reference/introduction), parent work item = plan (`external_id` = `planId`), children = steps. Venus integrates; it does not vendor or relicense Plane (AGPL).

## Running a plan

Multiple plans may be **in progress** at once. They must not take overlapping **spec leases**. Two plans that edit the same `docId` (or the same anchored part) serialize: second plan waits, or the agent splits scope. Code PRs for different plans may still be parallel if the repo allows.

### Per-step machine

```text
                    ┌─────────────┐
              ┌────►│ implement   │
              │     └──────┬──────┘
              │            │ agent opens/updates PR, writes code
              │            ▼
              │     ┌─────────────┐
              │     │ autotests   │  DoD scenarios as tests
              │     └──────┬──────┘
              │            │ pass
              │            ▼
              │     ┌─────────────┐
              │     │ review      │  GitHub PR + PR-review tool
              │     └──────┬──────┘
              │            │
              │     fail ──┘ loop to implement (same step)
              │     pass + breakpoint human → wait
              │     pass + breakpoint agent → agentic review record
              │            ▼
              │     ┌─────────────┐
              └─────│ done        │  next step (respects dependsOn)
                    └─────────────┘
```

- **Implement:** agent C (≠ DoD author, ≠ planner) works in the **product** git tree (see [repo layout](#2-wiki-repo-vs-product-repo)), pushes a PR linked in yaml `pr`. Must not hold a lease that edits `dod` on the plan page.
- **Autotests:** CI runs tests the implementer wrote against the **published** DoD (the human-accepted plan markdown at `dodAccepted.gitSha`). Fail → `implement`. Tests that do not trace to a published scenario do not satisfy the step.
- **Review:** a GitHub PR review tool (Copilot, Bugbot, or equivalent) comments on the PR. Requested changes → `implement` with the comment thread as context. Approve → breakpoint.
- **Breakpoint:** `human` — Venus notifies; a person looks at the PR (and optionally the wiki). `agent` — a designated reviewer agent records pass/fail; on fail, loop. `none` — proceed (only for trivial steps; never for `kind: docs`).
- **Done:** yaml `state: done`; any configured mirror is updated; next unlocked step starts.

The plan page itself stays readable (old/new/diff if someone is editing the plan). Execution does not require a lease on spec pages until a step writes specs.

### Parallelism

- Plans A and B: parallel if spec scopes do not overlap.
- Steps inside one plan: respect `dependsOn`. Independent steps may run in parallel (two PRs) if the user allows; default v1 is **one active step per plan** to keep breakpoints simple.
- After each completed step, the breakpoint is the join point before the next step starts.

## Docs close the plan

Shipping code without updating the spec is an incomplete plan.

1. Last (or dedicated) step `kind: docs` lists which spec `docId`s / anchors to update.
2. An agent drafts markdown under a **lease** on those pages — **the same path as spec and as plan/DoD** ([venus-design.md](../../design/venus-design.md)).
3. **Accept is human-only.** Agentic review may comment; it must not apply. Same rule as publishing spec or publishing DoD.
4. Only then the plan may move to fully `done` (yaml + plan page). Mirrors, if any, close the parent issue/story.

If implementation drifted from the original spec, the docs commit is where the spec catches up. If the human rejects, the plan stays open (`step-docs` not done); code can remain merged, but Venus does not mark the story done.

## Autodocumenting (APIs, aims, Hugo)

A finished project is not only a spec that matches the code. **Aims of the software** (why it exists, who it is for, what it refuses) and **API surfaces** (what callers may depend on) must be documented in the same markdown tree, so agents and humans share them and a site can publish them.

This is still the same gate: agents generate; humans keep intention. Generated files do not bypass the lease.

```text
  aims (wiki pages, human intention)
  spec (behavior)
  api/ or equivalent   ← generated markdown from code / OpenAPI
           │
           │  only after human-accepted git
           ▼
  Hugo site (publish mirror of accepted markdown)
```

| Kind | Who drafts | Who publishes | Notes |
|---|---|---|---|
| **Aims** | Agent may draft; human owns the meaning | Human accept on the aims page(s), same as spec | Not a dump of the README. Intention, faith in the product, non-goals. |
| **API markdown** | Agent (or `go generate` / OpenAPI → md) under a docs lease | Human accept of that lease (may be a fast review if mechanical) | Lives in the wiki git tree (`wiki/api/…` or similar), stable paths for Hugo. Regen after a plan is a `kind: docs` step, not a silent CI overwrite of unpublished CRDT. |
| **Hugo** | Config + layouts in git (product or wiki submodule) | CI builds **accepted git HEAD** (review + snapshot commits), never the live un-flushed CRDT | Hugo is a **site mirror**, like Plane is a board mirror. It is not a second spec. |

Folder sketch (names TBD; must be ordinary markdown Hugo can take as `content/` or via a mount):

```text
wiki/
  spec/
  plans/
  aims/
    product.md
  api/
    overview.md          # generated + accepted
    rest.md
hugo/
  hugo.toml
  layouts/
  # content → wiki/ (module, symlink, or copy in CI)
```

**Autopublish:** on each accepted wiki git commit (or parent submodule pin), CI runs Hugo and deploys. No separate “docs CMS.” If the adapter round-trip is not boring, generated API files should be mostly vanilla CommonMark so Hugo and BlockSuite both survive ([adapter gate](../../design/venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it)).

**Plan close:** `kind: docs` includes spec **and** aims/API regen when the change affects them. The story stays open until a human accepts those diffs. Agents do the generation; they do not ship the site off an unaccepted tree.

## What this adds to the three stores

| Store | Plans |
|---|---|
| Published BlockSuite doc | Plan pages under `plans/` (prose + structured steps; encoding TBD) |
| Git | `plans/*.md` + `plans/*.state.yaml`; also `aims/`, `api/` (wiki tree; product code is a separate git history — [§ Open decisions](#open-decisions)) |
| Review session | Leases for plan, DoD, spec/aims/API docs — all the spec write path |
| Outside Venus | Product git PRs, CI, PR-review bot; optional issue-tracker **mirrors**; **Hugo** as site mirror of accepted git |

Board yaml is closer to a review session than to the block tree: workflow, not published spec prose. It is still committed to git so clones and agents see status **without** Plane or Issues.

## Phasing (do not block the wiki)

Spec-driven is the product direction; it still sits on a working spec store.

| Slice | Ships |
|---|---|
| Wiki core | BlockSuite, lease, snapshot git, adapter ([v1-concerns.md](./v1-concerns.md)) |
| Plans v1 | After **open decisions** below: `plans/` schema, two-agent plan then DoD, yaml board, one-step-at-a-time runner, human breakpoints, **human docs accept** |
| Later | Issue-tracker mirrors (GitHub Issues first, Plane optional), parallel steps in one plan, overlapping-scope queue UI, Hugo autodoc (aims + generated API) |

## Open decisions

Take these before a runner exists. They do not change invariants (two-agent DoD, human accept, yaml as board). They change encoding and git topology.

### 1. How steps appear in markdown

` ```venus-step ` YAML-in-fences is **hostile markdown**: humans and LLMs cannot read it as a normal page, BlockSuite’s adapter will not round-trip a custom fence honestly, and it repeats the sidecar problem *inside* the document. The adapter is already the highest technical risk ([adapter gate](../../design/venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it)). **Do not add a custom fence language until export → parse → export is boring on the round-trippable subset.**

| Option | Shape | Risk |
|---|---|---|
| **A. Fences** (example above) | ` ```venus-step ` YAML per step | Adapter jitter; ugly clone; ids in a language git and LLMs mishandle |
| **B. Headings + tables** | `### step-snapshot` plus a small table or definition list (`id`, `dependsOn`, `kind`) and DoD as numbered scenarios in prose | Likely best BlockSuite round-trip; parser must be strict about heading ids |
| **C. Steps only in sidecar** | Plan `.md` is prose (story, narrative). Step records + DoD live in `*.steps.yaml` (or the existing state file). Markdown is not the schema. | Clean adapter; weaker “the plan file is the whole story”; DoD lease must edit yaml through the same human-accept path (treat sidecar as part of the plan page commit) |
| **D. Frontmatter list** | All steps in page YAML frontmatter; body is commentary only | Frontmatter size; still yaml; BlockSuite may strip unknown frontmatter |

**Constraint:** whatever is chosen must survive lease diff-by-block-id, human review hunks, and agent clone/grep. Prefer B or C over A.

**Status:** undecided. Example in this doc is A for compactness only.

### 2. Wiki repo vs product repo

Specs, plans, and Venus git snapshots live in the **wiki git tree**. Implementation PRs (`yaml.pr`) live in a **product git tree**. If that split is implicit, `pr:`, CI, submodule SHAs, and doc leases become two uncoordinated workflows.

| Option | Shape | `pr:` and docs |
|---|---|---|
| **A. Monorepo** | One remote: `wiki/` and application code together | One PR can contain code + wiki. Simple runner. Mixes product history with spec snapshots. |
| **B. Two remotes, no link** | Wiki remote; product remote; yaml stores a URL | Runner must know two remotes. Doc updates cannot land in the same PR as code. Easy to forget the wiki bump. |
| **C. Submodules** (lean) | Parent workspace repo; **wiki** and **product** are submodules (or product is parent and wiki is a submodule) | Product PR is normal. Wiki lease commits in the wiki submodule; parent pin updates when docs are accepted. Clone is one command; histories stay separate. |

**Lean: C (submodules).** Keep Venus markdown history out of the product’s `git blame`, keep product PRs ordinary, still have one workspace checkout for agents. Decide parent-vs-nested (workspace-of-two vs product-owns-wiki) when the first real product repo is wired.

**Status:** undecided; **prefer submodules**. Must be written into the runner and MCP before implement starts. Do not assume “the product repo” is the same git as `wiki/plans/`.

## Invariants

1. A plan is a user story; a step is a task; DoD is test scenarios on the step.
2. Plan and DoD use the **same write path as spec** (lease, hunks, human accept). Yaml does not store scenario text.
3. DoD is written by an agent **other than** the planner and **other than** the implementer, then human-accepted, before implement starts. Changing DoD is another such lease, not a yaml patch.
4. Workflow state lives in `*.state.yaml` (and the plan page). Trackers are **mirrors**, not the goal. Markdown is the spec of the work (including DoD).
5. Several plans may run at once; they must not hold overlapping spec leases.
6. Step cycle is implement → autotests → PR review → (breakpoint) → done, with loop back on failure. Tests bind to published DoD at `dodAccepted.gitSha`.
7. `kind: docs` (and any apply to published spec, aims, or generated API markdown) is **human-accepted**. Agentic review cannot publish spec, plan, or DoD. Hugo builds only that accepted git.
8. Autodoc is a generate-then-lease path, not a side channel. Aims stay human-intention pages; API files may be generated but still accepted.
