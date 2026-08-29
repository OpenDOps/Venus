# v1 concerns: git and review scope

Product rules that were underspecified in [venus-design.md](../../design/venus-design.md). This note picks v1 meaning; it does not replace the design.

Markdown round-trip is **implementation**, not a product hole: [venus-implementation-plan.md](../../design/venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it).

Related: [lease-freeze-rationale.md](../../design/lease-freeze-rationale.md), [licensing.md](../../legal/licensing.md), [venus-plan.md](./venus-plan.md).

---

## 1. WYSIWYG vs git

### The rule (not a contradiction)

Casual typing in BlockSuite **does not** create a comment-commit. It creates a **snapshot git commit** of the full diff since the last commit, with an **autocomment**.

**Only markdown editing** (lease) **must** create a **comment-commit** (required why, pin on the Before view).

Git stays current enough to clone; `git log` still has a real why on markdown. Keystroke-level git is forbidden (coalesce to one snapshot). “Git only on markdown accept” is also forbidden (clones would rot).

| | WYSIWYG (no lease) | Markdown (lease) |
|---|---|---|
| Editor | Live CRDT = **After** last git | Private markdown vs frozen `T0` |
| Git | **One snapshot** of After − HEAD | **One comment-commit** of hunks vs `T0` |
| Message | **Autocomment** (`snapshot: <title>`) | **Required** review comment |
| When | Idle 30–120s, explicit flush, flush-before-lease | On accept |

Rejected: git only on review accept (A); commit every keystroke (B); treat WYSIWYG as a lease (C). Adopted: idle snapshot + autocomment, plus flush-before-lease (D).

**C** is Policy Freeze on the wrong writer. Freeze is for markdown so `T0` is stable ([lease-freeze-rationale.md](../../design/lease-freeze-rationale.md)).

### Flush-before-lease

Before a markdown lease: snapshot-commit any dirty WYSIWYG (autocomment) for that page and pending catalog moves. Then `T0` = live CRDT = git HEAD.

**Lag:** during the idle window, git may trail the CRDT. After flush (and always before lease), they match. HEAD is not a realtime replica.

**Agents:** live store for *now*; git for files. `git log` why = skip `snapshot:` / `chore:`, read comment-commits.

### CRDT After / Before

Folded into [venus-design.md](../../design/venus-design.md#crdt-views-after-and-before): After = look+edit (when allowed) as it will be after this commit; one git commit for the whole diff. Before = parent, comments in a **right rail** (Google Docs / Jira).

### Design

Commit classes, invariant 5, stores table, and views live in [venus-design.md](../../design/venus-design.md). This section is the rationale.

### Non-goals for this hole (v1)

- Bidirectional `git push` from clones into the live CRDT.
- Perfect `git blame` through snapshot commits (filter autocomments).
- Zero lag.
- Editing the After **preview** CRDT during a markdown lease (holder uses markdown).

---

## 2. v1 review UI is too much

### The hole

The goal line in the design already asks for: lease, freeze, commented diffs, accept / rollback / reply, **alternative and stacked proposals**.

That last cluster is Git-for-docs (multiple PRs, stacked diffs): a **graph of review commits** (alternatives sharing `T0`, stacks via `parentCommitId`). Real product later, not the wedge. Building that graph first will hide adapter bugs behind “which commit is selected.”

### Adopted v1 wedge

**In v1:**

- One lease per page.
- **One** review commit at a time (one set of hunks, one required message).
- Comments on that commit / hunks (thread, reply).
- Accept, reject (rollback), regenerate (replace hunks on the **same** commit, bump `generation`).
- Old / new / diff views → **After / Before / Diff**; Before has a **right-rail** comment pin (Google Docs / Jira).
- Pin on a selection when the writer has one.

**Not in v1:**

- Alternative commits on one thread (`A` vs `B` vs same `T0`).
- Stacked commits (`C` based on `A`).
- Entry B as “empty commit, diffs later” (a commit with no hunks). Pin-a-comment on **live** text can exist as a comment thread without creating a review commit.
- Multi-page / folder leases (design already says “later”).

Regenerate is enough of the Cursor loop: same lease, same commit id, new hunks, comments stay attached by `generation`.

### Why this order

1. Adapter fixture suite green ([implementation plan](../../design/venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it)).
2. Git snapshot class and review class do not fight ([§1](#1-wysiwyg-vs-git)).
3. One comment-commit + After/Before.
4. Then alternatives/stacks (`baseClock`, supersede).

---

## v1 summary

| Concern | v1 rule |
|---|---|
| WYSIWYG vs git | Snapshot + **autocomment** for casual typing; **comment-commit** only for markdown. After/Before CRDT views. Flush-before-lease. |
| Review UI | One lease, one commit, comments, accept / reject / regenerate. No alternatives, no stacks. |

§1 is in [venus-design.md](../../design/venus-design.md). Adapter round-trip is in [venus-implementation-plan.md](../../design/venus-implementation-plan.md#markdown-adapter-gate-build-this-do-not-debate-it).
