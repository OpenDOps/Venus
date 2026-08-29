# CRDT limitations with markdown
_Exported on 29.08.2026 at 12:27:39 GMT+3 from Cursor (3.16.17)_

---

**User**

Explain why CRDT cannot work with markdown

Question isn't related to the repo. Don't look there

---

**Cursor**

CRDTs can merge markdown **as a string**. They cannot merge markdown **as a document**. The merge is well-defined; the meaning after the merge is not.

## Merge and parse do not commute

A text CRDT (Yjs, Automerge, RGA, …) gives you a single, deterministic character sequence after concurrent edits. That is all it guarantees.

Markdown meaning is the *parse* of that sequence. Parsing is context-sensitive and not a homomorphism:

```text
parse(merge(A, B))  ≠  merge(parse(A), parse(B))
```

Two users can each leave behind valid markdown. The merged source can be a different document, or invalid.

## Markers are not data, they are syntax

Bold, links, headings, lists, and fences are paired or positional tokens in a linear string. A CRDT inserts and deletes those characters independently. It has no notion of “this `*` opens emphasis” or “this `` ``` `` must match that one.”

Typical failures:

- Two people wrap the same word in `**` → `****word****`, or mismatched markers.
- One person types `[label](url)`, another edits the same span → a broken link, or a link whose label and URL no longer belong together.
- Concurrent `#` / `-` / indent at the start of a line → a paragraph becomes a heading or a nested list, or stops being one.
- One fence added or deleted → the rest of the file is swallowed as a code block.

The CRDT did its job: everyone converges on the same bytes. The markdown grammar did not.

## Structure lives in lines and indentation

Markdown structure is mostly line-oriented (ATX headings, lists, quotes, fences) and indentation-sensitive (nested lists, code). Character CRDTs do not know what a line or an indent level is. Concurrent indent/outdent, list splitting, or wrapping a block in a fence routinely produce a different tree than either author intended.

## An AST CRDT does not save you

You can parse to a tree and CRDT the tree instead. Then you hit the other side of the same problem:

- Markdown has **no unique, stable document model**. CommonMark, GFM, and “original” Markdown disagree. The same source can round-trip to different ASTs; the same AST can serialize to many sources.
- Users edit **source**, not the tree. Whitespace, which marker was used (`*` vs `_`), reference-style links, and hard wraps are not in the AST. Store the tree and you lose what people typed. Store the source and you are back to the first problem.
- Concurrent structural edits (split paragraph, turn into a list, wrap in a quote) need tree operations with intent. Markdown source does not record that intent.

So you either CRDT characters and break structure, or CRDT a rich-text/block tree and stop treating markdown as the source of truth.

## What actually works

Collaborative “markdown” products that hold up do one of these:

1. CRDT a **rich-text or block model** (Peritext, ProseMirror + Yjs, Notion-style blocks) and **export** markdown.
2. CRDT the **source as plain text** and accept broken/surprising markdown, with humans cleaning it up.
3. Restrict concurrency (locks, OT on a linear buffer with extra constraints) so the bad interleavings cannot happen.

The limitation is not “CRDTs cannot store text.” It is that markdown is a **lossy, context-sensitive serialization**, and CRDT merge is defined on the replica state, not on the language’s invariants. Those invariants are not closed under concurrent character (or naive tree) merge.

---

**User**

So problem is when someone is editing CRDT in editor, and another one  is editing markdown. Correct?

---

**Cursor**

No. That mismatch is a real extra problem, but it is not the core one.

The core failure happens when **everyone is editing the same markdown source through the same CRDT**. Two people in the same collaborative markdown editor, both typing characters, still converge on one string whose markdown meaning can be wrong or invalid. The CRDT never “sees” bold, lists, or fences — only characters — so concurrent edits do not preserve markdown structure.

The case you described is different: **two representations of one document**.

- Person A edits a CRDT of a rich-text/block tree (WYSIWYG).
- Person B edits the markdown source.

Those are not the same data type. Mapping tree ↔ markdown is lossy and not unique, so you cannot merge the two views without picking a source of truth. That is a dual-model problem, not “CRDT vs markdown” as such.

So:

| Situation | What goes wrong |
|---|---|
| Both edit markdown source via a text CRDT | Merge is consistent; **parsed document** may not be |
| One edits a tree CRDT, one edits markdown source | **Two models**; no faithful merge between them |

The first row is why people say CRDT does not work *with markdown*. The second row is why you should not treat markdown source and a structured editor as interchangeable replicas.

---

**User**

My goal is to make standart notion-style WYSIWYG CRDT editor, that also has markdown editable representation.

As I see it:
1. We can use standart editors, so CRDT in editor not a problem
2. But when someone works in editor, markdown representation laso can be updated (in read mode) life
3. Seams problem is when someone is editing markdown in parallel session. But is it really a problem and could be merged?

---

**Cursor**

You can merge **editor sessions**. You cannot merge **two documents** (a block CRDT and a markdown string).

## What is easy

**WYSIWYG CRDT** is a solved problem: one canonical document (block tree + per-block rich text), like Notion. Standard stacks (Yjs/Loro + a block editor) already merge concurrent WYSIWYG edits.

**Live markdown in read-only mode** is not a replica. It is a serialization of the CRDT. On every update, render markdown again. No merge, no CRDT on the markdown. That path is safe.

## What actually has to be true for editable markdown

The markdown view must **write into the same CRDT**, not keep its own text document.

```text
                    ┌─────────────────────┐
  WYSIWYG  ───────► │  block-tree CRDT    │ ◄────── markdown editor
                    │  (source of truth)  │
                    └──────────┬──────────┘
                               │ serialize
                               ▼
                    read-only markdown view
```

Two WYSIWYG users merge. A WYSIWYG user and a markdown user also merge — **if** every markdown keystroke (or every parse of a finished edit) becomes operations on that same tree: insert/delete in a text CRDT inside a block, split/join blocks, change block type.

Then there is nothing special about “markdown session vs editor session.” Both are clients of one CRDT.

## When it is a real problem

It is a problem if the markdown session is a **second CRDT** (or a raw string you parse later):

```text
User A:  block CRDT  ──►  merge  ◄──  User B: markdown string CRDT
```

Those types do not merge. `parse(markdown)` and the live tree will disagree, and `parse(merge(A,B))` is not `merge(parse(A), parse(B))`. A full re-parse-and-replace also **clobbers** concurrent WYSIWYG ops (lost block IDs, lost marks, lost anything markdown cannot represent).

So: parallel markdown editing is fine as a **UI**. It is not fine as a **second source of truth**.

## Can those edits be merged?

Yes, with a binding — not with a generic CRDT merge of markdown.

Practical ways, from most to least Notion-like:

1. **Markdown-as-you-type in WYSIWYG** (`# ` → heading, `- ` → list). No source document. Notion’s real model. Concurrent sessions are all WYSIWYG CRDT.

2. **Per-block source edit.** Only the block you are focused on is markdown. Parse that block back into the CRDT on blur or on each transaction. Other people editing other blocks merge cleanly. Same-block concurrent edits are normal text-CRDT merges inside that block.

3. **Full-page source mode on the same CRDT.** User edits a markdown buffer; you diff/parse into tree ops (stable block IDs in the serialization help a lot). Merge works because you never merge markdown — you merge the resulting CRDT ops. This is the expensive path: cursor mapping, mid-keystroke vs live re-serialize, syntax your schema does not have.

4. **Exclusive source mode.** One writer in markdown at a time; others stay in WYSIWYG or read-only markdown. Avoids the dirty-buffer problem. Weakest concurrency, simplest product.

The nasty case is not “two people typed at once.” It is **User B’s unsaved markdown buffer** vs **User A’s CRDT updates**. If you push a fresh serialization into B’s editor, you wipe their cursor and half-typed syntax. If you do not, B’s buffer is already a fork. Fix: do not live-overwrite a dirty source buffer; apply CRDT updates as tree ops and keep source mode as a projection, or only update read-only markdown for everyone else.

## Round-trip is the real limiter

Even with one CRDT, markdown cannot name the whole Notion model (databases, page properties, colors, comments, nested pages). So:

- Live read-only markdown = **lossy export**, always OK.
- Editable markdown = only the **subset** of the schema you can parse back, plus a policy for the rest (untouchable, or raw/HTML blocks).

**Recommendation:** CRDT is only the block document. Markdown is a view. Read-only live preview for everyone. Editable markdown either as shortcuts + per-block source, or as exclusive page source — not as a parallel markdown CRDT. Then parallel sessions merge; you never “merge markdown.”

---

**User**

I like
4. Exclusive source mode. One writer in markdown at a time; others stay in WYSIWYG or read-only markdown. Avoids the dirty-buffer problem. Weakest concurrency, simplest product.

Let's model how this works.
If writers rewrite crdt tree from editor it will be reflected also in markdown. How only one markdown editor will see this, how his cursor will move on edits?
Also when he writed to markdown how crdt-tree users will see this?

---

**Cursor**

Exclusive mode does **not** mean the markdown client has a private tree. Everyone still shares one CRDT. Exclusive only means: **at most one editable markdown buffer**. Everyone else is WYSIWYG and/or a throwaway read-only export.

That is why only one client has a cursor problem.

## Roles

```text
                    lease: markdownWriter = User M
                              │
  User A (WYSIWYG) ──ops──► ┌─┴─────────────┐ ◄──ops── User M (markdown editor)
  User B (WYSIWYG) ──ops──► │  block CRDT    │
  User C (read-only md) ◄── │  source of truth│
                            └────────────────┘
```

- **Lease:** one `markdownWriter`. Heartbeat it. On steal/timeout, the old editor becomes read-only.
- **A/B:** normal CRDT clients. They never see markdown.
- **C:** `markdown = serialize(tree)` on every update. No cursor to keep. Full replace is fine.
- **M:** the only client that must **splice** markdown and **map a cursor**.

A/B/C all “see” the same tree. M is special only because it has a live source caret.

## Two exclusive policies

You liked exclusive mode to avoid a dirty buffer. That forces a choice when A is still typing in WYSIWYG.

**Policy Freeze (simplest exclusive).** While M holds the lease, A/B become read-only. M’s cursor never moves from remote edits. M’s keystrokes update the CRDT; A/B watch WYSIWYG live. No inbound mapping.

**Policy Live (what you are asking).** A/B keep writing. M still holds the only markdown editor. Then you must project tree changes into M’s text and map the caret.

Below is Policy Live, plus the one rule that keeps exclusive mode simple.

## Rule that makes exclusive mode work

Do **not** do this on M:

```text
editor.setValue(serialize(tree))   // wipes caret and IME
```

Do this:

1. Keep a **range map** while M is in source mode: `blockId → [from, to)` in the markdown string.
2. Serialize **per block**, not the whole page, when applying inbound ops.
3. Apply **text splices** through the editor API (CodeMirror/Monaco). The editor moves the caret.
4. Treat **only the block containing M’s caret as dirty**. Remote updates to other blocks splice in immediately. Remote updates to the dirty block wait until that block is parse-clean (blur, idle, or a successful parse).

That last point is the exclusive-mode trick: you never have two markdown buffers, and you never rewrite the paragraph M is mid-syntax in.

## Inbound: WYSIWYG → M’s markdown (cursor)

Example. Tree:

```text
b1 heading  "Hello"
b2 para     "World"
```

M’s buffer (ranges are the map, not shown to the user):

```text
# Hello\n\nWorld
  ^b1^     ^b2^
```

Caret is in `b2`: `Wor|ld`.

**A changes the heading to “Hello there”.**

1. CRDT applies on every client. A/B’s WYSIWYG already shows it.
2. C replaces its read-only string; no caret.
3. M: only `b1` changed → replace `b1`’s slice `# Hello` with `# Hello there`.
4. Editor splice: insert 6 characters **before** the caret → caret stays `Wor|ld`.
5. Map: `b1` grows; `b2` shifts right by 6.

That is ordinary editor-transaction mapping: edit before caret shifts it; edit after leaves it; edit overlapping the caret pins it to the edge of the splice.

**A edits `b2` while M’s caret is in `b2`.**  
Do not splice. Queue those ops (or let the text CRDT absorb them but do not re-serialize). When M’s parse of `b2` is valid, flush: rebase queued ops into `b2`, or re-serialize `b2` once and put the caret at a relative position (same offset, clamped). Overlap is the one ugly case; exclusive mode exists so you can **refuse to fight** here instead of doing a second markdown CRDT.

**A inserts/deletes/moves a block.**  
Splice the corresponding markdown slice (plus the `\n\n` separator), shift the map. Caret moves only if the splice is before it — same as someone else inserting a paragraph above you in a text editor.

## Outbound: M types markdown → A/B’s WYSIWYG

M’s buffer is not a replica. Each successful parse becomes CRDT ops on the **same tree**.

```text
M keystroke
  → editor text + update dirty block range
  → parse dirty block (or full doc on structural paste)
  → diff with CRDT by block id
  → apply: insertText / deleteText / changeType / split / join / insertBlock / deleteBlock
  → A/B receive normal CRDT ops
  → their WYSIWYG updates the same way as if another WYSIWYG user typed
```

A/B do not “see markdown.” They see tree ops.

Preserve **block ids** in the projection (side map from ranges, or hidden `<!-- id -->` in the serialization). Diff by id, not by “paragraph 3.” Then:

- M types `x` in `b2` → `insertText(b2, offset, "x")` → A’s caret in that paragraph moves via the text CRDT, same as Google Docs.
- M turns a line into `## ` → `changeType(b2, heading, 2)`.
- M pastes a list → insert blocks with new ids; A sees new list blocks.
- M deletes a heading → `deleteBlock`; A’s selection in that block is dropped or moved, same as a WYSIWYG delete.

Flush policy:

- **In-block typing:** apply on every editor transaction if the block still parses; otherwise wait (unclosed `**`, half-typed `- [`).
- **Structural edits** (new lines, fences, pastes): parse immediately; they change the range map.

Until a parse succeeds, A/B do not see the broken syntax. That is what you want: the tree never stores `**wo`.

## What each person sees, at the same moment

| Client | A changes heading | M types in a paragraph |
|---|---|---|
| A WYSIWYG | local | live CRDT text insert |
| B WYSIWYG | live CRDT | live CRDT |
| C read-only md | full re-export | full re-export |
| M markdown editor | splice `b1`, caret in `b2` stays | local typing; parse → CRDT |

Only M has caret mapping. C can throw the string away every time. That is “how only one markdown editor sees this”: everyone sees the **tree**; only M has a **cursor in source**.

## Session lifecycle

1. M requests lease. If free, snapshot `serialize(tree)` into the editor, build the range map, open editable source.
2. While leased: M writes via parse→ops; A/B write via WYSIWYG ops; inbound splices skip M’s dirty block.
3. M leaves source (or lease expires): parse once, apply remaining diff, drop the buffer, release lease. Read-only views just keep exporting.

If you want **zero** cursor-vs-remote-edit logic, use Policy Freeze at step 2: A/B read-only until release. Then inbound mapping disappears; outbound is unchanged. That is the actual simplest product. Policy Live is “exclusive markdown **role**, concurrent WYSIWYG,” and the dirty-block rule is the whole model.

---

**User**

I think what I realy need is taking lease on markdown edit.

Also what I want is that markdown edit has commits like cursor IDE code changes:

Agent takes a lease, can update markdown with diffs (each markdown block has old and new version - what was removed, what was added - diff), and each diff also has a comment with the reasoning, why it was changed and what idea was under it, what was wrong in original concept)
While agent is editing markdown, human users from editor can't edit - they wait, but see this diffs: they can look at new text, or old text (different modes) but each diff also has a comment, and option to accept/rollback the change, or add a comment in concersation and ask regenerate again

---

**Cursor**

This is no longer a collaborative markdown editor. It is a **leased revision**: one writer (the agent), a frozen published tree, and a review overlay like Cursor’s agent diffs.

Humans never merge markdown with the CRDT. They either wait, or they accept/reject hunks. The CRDT only moves on **commit**.

## Source of truth vs proposal

```text
 published tree (CRDT)          revision (not CRDT)
 T0, frozen for humans          snapshot = T0
        ▲                       hunks[] + conversation
        │ apply accepted
        │ then release lease
        └───────────────────────┘
```

- **Published tree:** what WYSIWYG is. Untouched until hunks are accepted.
- **Lease:** agent holds it; human editors are read-only.
- **Revision:** proposed markdown/tree plus per-block diffs and threads. Synced to everyone as ordinary session state (not as a second document CRDT).

The agent may edit markdown internally. Users never share that buffer. They see **hunks**.

## Lease

```text
idle ──acquire──► leased (agent writing / humans reviewing)
                      │
                      ├─ commit accepted hunks → apply to CRDT → idle
                      ├─ reject all → drop revision → idle
                      └─ timeout / steal (with confirm) → drop or commit policy
```

While `leased`:

- WYSIWYG typing is disabled (wait state).
- Everyone can switch view mode and act on hunks: accept, rollback, comment.
- Only the lease holder may **write** new proposed text.

That is the exclusive mode you wanted, with a review UI instead of a second caret.

## What the agent commits (a revision)

On acquire, snapshot the tree as `T0` and serialize to markdown **with stable block ids** (hidden in the serialization, not shown as syntax the model must invent).

The agent does not stream into the published CRDT. It emits **hunks**:

```text
Hunk
  id
  blockId        // null if insert
  kind           // modify | insert | delete | move
  old            // markdown + tree slice at T0 (empty if insert)
  new            // markdown + tree slice proposed (empty if delete)
  diff           // character/line diff: removed[] / added[]
  rationale      // why, what idea, what was wrong in the original
  status         // pending | accepted | rejected | regenerating
  thread         // comments: human | agent
```

One hunk ≈ one markdown block (or one structural move). Inside a modified block, `diff` is the red/green the way Cursor shows a file hunk.

Streaming works: hunks appear as the agent writes. Humans can already accept/reject early hunks while later ones are still generating.

## What humans see (they are not editing)

Three projections of the same revision. No one is typing in the page.

| Mode | Shows |
|---|---|
| **Old** | `T0` only (published, frozen) |
| **New** | `T0` with pending+accepted hunks applied (proposed page) |
| **Diff** | block-by-block old vs new, plus rationale and thread |

WYSIWYG can stay the renderer: Diff mode is the same block editor with an overlay (old/new/split) and a card per hunk. You are not running a markdown CRDT in that view.

Each hunk card:

- rationale (agent)
- old / new / inline diff
- **Accept** — mark hunk accepted (does not have to hit the CRDT yet)
- **Rollback** — mark rejected; proposed block returns to `T0`
- **Comment** — append to `thread`, ask regenerate

## Apply path (the only CRDT writes)

Do **not** rewrite the tree on every agent token. Apply once, from hunk status.

**Accept one hunk:** `status = accepted`. Published CRDT still `T0` until you choose when to materialize.

Two valid policies:

1. **Apply on each accept.** Immediate WYSIWYG of accepted blocks; remaining pending hunks still overlay. Simple feedback, slightly harder regenerate (base moved).
2. **Apply on session commit.** CRDT stays `T0` until everything is resolved or the user hits “Commit accepted.” Cleaner, closer to a git commit.

For an agent loop, **(2) plus a local preview** is the better default: New mode is a preview of `T0 + accepted + pending`. Published CRDT changes only on **Commit** or **Accept all**.

Commit:

```text
for hunk in revision.hunks where accepted:
  apply to CRDT by block id
    modify → set block type/content from hunk.new
    insert → insertBlock
    delete → deleteBlock
    move   → moveBlock
release lease
drop revision
```

Rollback of a hunk is not a CRDT undo. It only drops that proposal. Rollback of the session drops the whole revision; CRDT never moved.

## Comment → regenerate (Cursor-style loop)

The lease **stays held**. The published tree still does not move (if you use commit-at-end).

```text
human comment on hunk H
  → H.status = regenerating
  → agent gets: T0, all hunks, H.thread, which hunks already accepted
  → agent emits replacement hunks (same ids or new ids)
  → UI updates those diffs; other accepted hunks stay
```

Agent input should be explicit:

- frozen original: `T0`
- already accepted proposals (treat as “human said this is good”)
- rejected hunks (do not revive unless asked)
- the conversation on the hunk they pinged

That is the same shape as Cursor: the file on disk may still be old; the review session is the working set.

## Why this sidesteps the CRDT/markdown mess

- Humans do not have a markdown caret, so no cursor mapping.
- Agent markdown is a **private write** against `T0`, then parse → tree `T1` → block diff vs `T0`.
- Parse failures stay in the revision (hunk invalid / retry). They never land in the published CRDT.
- Concurrent humans cannot create a second replica; they are in wait+review.
- Accept is a normal CRDT transaction on the block tree, so after release everyone is just WYSIWYG again.

## Session, end to end

1. User asks the agent (or agent starts). **Acquire lease.** Snapshot `T0`. Editors lock.
2. Agent writes markdown against `T0`, parses, diffs by block id, attaches a rationale per hunk. Hunks stream into the review overlay.
3. Everyone watches. Modes: old / new / diff.
4. Per hunk: accept, rollback, or comment.
5. Comment → regenerate those hunks; lease remains.
6. **Commit** accepted hunks into the CRDT (or **Reject session**). **Release lease.** Editors unlock on the new published tree.

If you want the lock to feel less dead, the wait state is not a spinner: it is this review. The only thing they cannot do is mutate the published tree until they accept.

---

**User**

With the commentable diffs, what should be CRDT tree data structure? Should it be changed?

---

**Cursor**

Keep the published CRDT tree as a **clean block document**. Put commentable diffs in a **sibling review document**. Mixing them will make every editor, export, and undo path special-case “maybe this block is only a proposal.”

## Published tree — do not grow a pending schema

This stays the Notion-style CRDT, and it only ever holds **accepted** content:

```text
Doc
  blocks: Map<blockId, Block>          // or tree of CRDT nodes
  order:  Sequence of blockId          // parent → children

Block
  id
  type                                 // paragraph, heading, list, …
  parentId
  props                                // heading level, etc.
  text                                 // rich-text CRDT (chars + marks)
```

Do **not** add `proposedText`, `hunkStatus`, `rationale`, or `old/new` onto `Block`. Those are revision fields. If they live on the tree:

- freeze is a lie (the tree is mutating under reviewers)
- rollback leaves ghost blocks
- markdown export and WYSIWYG must filter “not real yet”
- agent streaming becomes concurrent tree edits you then have to undo

You only need two small affordances that a good block CRDT already has:

1. **Stable `blockId`s** that survive serialize → agent markdown → parse.
2. **A snapshot clock** (`docVersion` / CRDT state vector) so the lease is “this exact `T0`.”

That is the whole tree change: none of the review UX, plus identity + snapshot.

## Where diffs and comments live

A second CRDT (or server-authoritative doc synced like a CRDT) for the **lease/revision**. Several humans comment at once; only the agent writes hunk bodies.

```text
Revision                          // sibling of Doc, keyed by docId + leaseId
  lease: { holder, snapshotClock, snapshotTree }   // T0 frozen
  hunks: Map<hunkId, Hunk>
  thread: Array<Comment>          // session-level conversation (optional)

Hunk
  id
  blockId                         // null if insert
  afterBlockId                    // insert/move anchor in T0 order
  kind                            // modify | insert | delete | move
  old                             // immutable snapshot (markdown + tree slice)
  new                             // immutable snapshot
  rationale                       // agent, immutable per generation
  status                          // pending | accepted | rejected | regenerating
  generation                      // increments on regenerate
  thread: Array<Comment>          // commentable diff

Comment
  id, author, createdAt, body
  anchor?                         // see below
```

`old` / `new` are **values**, not live text CRDTs. Nobody is co-editing the proposal; the agent replaces `new` on regenerate. Storing them as CRDT strings buys you nothing and invites the markdown-merge problem you already rejected.

The published `text` CRDT stays for human WYSIWYG **after** the lease. During the lease it is frozen.

## What is concurrent (and what is not)

| Data | Type | Why |
|---|---|---|
| Published blocks + rich text | Document CRDT | Normal editor, only written on commit |
| Hunk `old`/`new`/`rationale` | Last-writer (agent) | One leased writer |
| Hunk `status` | LWW or single reviewer | Workflow, not text |
| `thread` comments | Sequence CRDT | Many humans, live |
| Lease | Single owner | Not a merge |

Comments are the only part that *needs* CRDT-like concurrency during review. Diffs do not.

## Comment anchors

Do not anchor review comments in the published text CRDT. `new` is not in that tree yet.

```text
anchor =
    { hunkId, generation, side: old|new, start, end }   // offsets in that snapshot
```

On **commit**, you may **rebase** a comment onto the published tree:

```text
{ hunkId, side: new, start, end }  →  { blockId, start, end } on Block.text
```

That rebase is optional. Two products:

- **Review-only threads** — die with the revision (Cursor-like: conversation is about the change).
- **Document comments** — survive as a third, small structure on the published doc:

```text
DocComments
  threads: Map<threadId, { blockId, start, end, messages }>
```

That last map is a normal Notion-style comment layer. It is **not** the agent-diff model. Do not reuse hunk threads as the document comment schema, or regenerate will move offsets and corrupt anchors. Copy on commit if you want them to persist.

## Commit is the only tree mutation

```text
accepted hunks + T0
  → CRDT ops on Doc (insert/delete/move/setText)
  → Revision archived (immutable history)
  → DocComments maybe receive rebased threads
  → lease released
```

Archived revision is a log record (git-like), not leftover pending nodes in the tree.

## Should the tree change at all?

**Almost no.** Same block tree, same rich-text CRDT.

**Add beside it:** `Revision` (hunks + commentable threads) and optionally `DocComments` (post-commit anchors).

**Add on the tree only:** stable ids and a snapshot clock.

If you change the tree to hold diffs, you are modeling a review session as document content. It is not. The tree is the published page; the commentable diff is a leased transaction on top of a snapshot.

