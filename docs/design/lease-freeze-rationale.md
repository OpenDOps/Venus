# Why markdown lease freezes the published editor

Venus freezes WYSIWYG (and any other published write) on a page for the whole time a markdown lease is held. This is **Policy Freeze** from the earlier design discussion, adopted as a product rule — not a temporary shortcut.

This note explains why freeze is the correct exclusive mode for Venus, given comment-commits, alternative/stacked proposals, and git snapshots. The dual-model argument is in [cursor_crdt_limitations_with_markdown.md](../drafts/pre-design/cursor_crdt_limitations_with_markdown.md). The full model is in [venus-design.md](./venus-design.md).

## What freeze is

When user or agent `M` holds the lease on page `D`:

- The published BlockSuite doc `D` is **read-only** for everyone, including `M` in WYSIWYG.
- `M` writes a **private markdown buffer**. That buffer is not a CRDT and is not synced as text.
- Everyone else stays in the page: they watch **old / new / diff**, comment, accept, reject, and attach alternative commits.
- The **published** CRDT and git **do not move** until a **comment-commit** is accepted (no WYSIWYG snapshots during the lease).

Freeze is a lock on **published mutation**, not a lock on **conversation** (Before-view comment rail).

## The two exclusive policies

Both policies already assume at most one editable markdown buffer.

**Policy Live** — others keep typing in WYSIWYG. The markdown editor must splice remote block updates into its buffer, map the caret, and treat the focused block as dirty so a mid-syntax paragraph is not rewritten.

**Policy Freeze** — others cannot type in WYSIWYG. The markdown buffer is a clean export of snapshot `T0`. No inbound splices. No caret-vs-remote-edit.

Venus chooses Freeze.

## Why Live is the wrong product here

Policy Live is a collaborative source editor. Venus is a **leased revision** that must produce a commented commit and a git snapshot.

### 1. Markdown and the block tree are different types

A text CRDT can merge characters. It cannot merge markdown *as a document*: `parse(merge(A,B))` is not `merge(parse(A), parse(B))`. If WYSIWYG keeps mutating the tree while someone edits source, Venus would have to continuously project tree ops into a dirty markdown buffer (Live) or periodically re-parse and clobber the tree.

Live spends its complexity on that projection. Venus spends it on review: hunks, pins, alternatives, stacks, git. Those features need a **stable `T0`**, not a moving base.

### 2. Comment-commits need a frozen base

A commit is `diff(T0, proposed)` plus a comment pinned to a selection.

If A types in WYSIWYG after `T0`:

- Hunk line numbers and block slices drift.
- A pin `{ side: old, start, end }` no longer names the sentence the commenter saw.
- Alternative commits on the same thread no longer share a base, so “pick A or B” is ill-defined.
- Stacked commits (`C` based on `A`) cannot compose: `A` was against `T0`, but the tree is already `T0'`.

Freeze makes `T0` a real snapshot (OctoBase/y-octo clock + exported markdown + block-id map). Every proposal on the page during the lease is against that clock.

### 3. Git history would lie

Published git has two classes ([venus-design.md](./venus-design.md)): WYSIWYG **snapshots** (autocomment) and markdown **comment-commits** (required why).

If WYSIWYG writes **during a lease**, either:

- those keystrokes are unpublished CRDT-only changes (git lags), or
- Venus interleaves snapshot commits with no review comment **while a comment-commit is in flight** (the After/Before base moves), or
- the final accept is a mash of lease hunks plus live typing (the required comment cannot explain the change).

Freeze: no snapshots and no published WYSIWYG until accept. **Published CRDT and git advance together on comment-commit accept.** Snapshots only when there is no lease.

### 4. Agents make Live unsafe

An agent is a lease holder that streams hunks. Live would mean humans editing the same blocks the agent is rewriting. That is the Cursor dirty-buffer problem: either the agent’s next patch misses human edits, or a re-export wipes the human, or you build a second merge (markdown ∩ tree) that this project already rejected.

Freeze is the same contract Cursor uses: the file (here, the published tree) is still on disk/`T0`; the working set is the review session.

### 5. Freeze is the only exclusive mode that stays simple

Live’s hard case is “remote edit to the block that contains the markdown caret.” Exclusive mode exists so Venus can **refuse that case**. Implementing Live means building the system exclusive mode was meant to avoid: per-block range maps, editor-transaction splices, dirty-block queues, IME-safe `setValue` bans.

Venus still needs parse → id-diff → BlockSuite ops for **outbound** apply. That is one direction, on accept. Inbound splice for a live caret is the other direction, continuously. Freeze deletes that direction.

## What people can still do during a lease

Freeze is easy to hear as “the page is dead.” It is not.

| Allowed | Forbidden |
|---|---|
| Read **Before** (`T0` / parent After) + **comment rail** | Type in **published** WYSIWYG |
| Edit **After** proposal CRDT (OctoBase space) | Merge markdown as a CRDT |
| Comment on pins (Google Docs / Jira rail) | Apply hunks to the **published** CRDT except via accept |
| Accept / reject / request regenerate (draft only) | Folder-level published edits of this page’s body |
| Next comment-commit **over** a submitted After | Silent rewrite of a submitted commit’s hunks |

The wait state **is** the review UI. Mutating the **published** tree without a **comment-commit** is forbidden during the lease. WYSIWYG **snapshot+autocomment** only on published, when there is no lease. After-space typing is review (sequence), not a snapshot.

Comment-only pins **before** a lease do not freeze the page. Freeze starts when someone needs to write markdown or attach hunks.

## Scope of the lock

v1: **one page**. Other pages stay collaborative. The catalog (folder moves of *other* docs) stays live. Moving the leased page itself waits until release (path is part of the published snapshot story).

Later, a folder lease can freeze a subtree for a multi-file agent commit. Same rule, larger `T0`.

## Steal, timeout, crash

Freeze must not become a stuck lock.

- Heartbeat; expiry → release, drop unsubmitted buffer, keep submitted proposals as rejected or “abandoned” per policy.
- Steal: explicit confirm. Current holder’s dirty buffer is discarded unless they submitted a draft commit.
- Holder may **submit draft commits and keep the lease** (agent loop) or **submit and release**.

Submitted review commits (session record + Before/After **spaces**) are in OctoBase, not only the holder’s browser. A crash does not lose a submitted After CRDT; it only loses an unsaved CodeMirror buffer that was never submitted.

## What we are not claiming

Freeze does **not** mean:

- Markdown is a better source of truth than the block tree. It is not. The tree remains the live document.
- We will never build Live. If a later product wants “source mode while others type,” it is a new project on top of a working Freeze wiki.
- Comments require a lease. They do not. Only **writes that produce hunks** do.

## Decision

**Adopt Policy Freeze.** One markdown writer, frozen published page, review as the multiplayer surface, CRDT + git advanced only on accept.

That is the **current** exclusive mode, designed for a Cursor-like loop (prompt → wait → review → accept), not a Notion-like live page while an agent patches. Whole-page freeze can be **extended later** (narrower leases, other modes) once that loop is boring. Product framing: [comparisons.md](../marketing/comparisons.md#versus-cursor).

That is how Venus stays a CRDT wiki for humans and a markdown/git wiki for humans and LLMs, without pretending those two representations can merge.
