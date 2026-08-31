# Glossary

Words that collide in Venus. Product + data design: [venus-design.md](./venus-design.md). Stores: [datamodel](./datamodel/README.md). Runtime dataflow: [architecture.md](./architecture.md).

## Four “snapshot” senses

| Term | What it is | Pinned? | Browser? | When |
|---|---|---|---|---|
| **Doc export** | HTTP read of the **current** Y.Doc: `GET /api/block/venus-m0/export` (Yjs update v1). Next GET can differ. Reads **Postgres**, not the collab process’s live RAM (M1: keck; after [M3.0](./M3.0/README.md): hub). | No | No | [M1 step 7](./M1/plan.md#7-step-snapshot); M3.0 keeps the URL or an alias |
| **Pin** | Frozen copy of export bytes (and catalog) at time T. Convert and git run on this copy. Live CRDT is not paused. | Yes | No | M3+ ([LiveSnapshot](./LiveSnapshot/README.md)) |
| **`T0`** | The pin **kept** at lease acquire. Review Before/Diff is vs this clock. | Yes | No | M5 |
| **Git snapshot commit** | WYSIWYG pin → markdown + autocomment (`snapshot: <title>`). Not a review why. | Yes (git SHA) | No | M3 |
| **Markdown projection** | `MarkdownAdapter.fromDoc` of the block tree. Share format, **not** a second live replica. | n/a | Pane is in the app; git is files | [M2](./M2/README.md) ([MDGate](./MDGate/README.md)) |

**Doc export** is the only one M1 implements. A pin **keeps** an export (or a replica encode). Lease `T0` is a pin that outlives the flush. Git flush converts pins, not the live store.

## Related

| Term | Meaning |
|---|---|
| **Live CRDT** | BlockSuite `Store` / `store.spaceDoc` (Y.Doc). Browsers share it over the sync WebSocket. |
| **keck** | OctoBase WebSocket + HTTP front. Compose service `octobase`. **M1 (done).** Not the product collab front after [M3.0](./M3.0/README.md). Recon: [octobase.md](./LiveSnapshot/octobase.md). |
| **Hub** | Venus-owned merge buffer: apply Yjs, broadcast, persist ~1s. Compose service `hub`. Same `AFFiNE` + y-protocols wire as M1. Not JWST, not git, not `jobs`. [M3.0](./M3.0/README.md), [hub HA](./M3.0/high-availability.md). |
| **Postgres** | Persist for Yjs docs **and** blobs. Compose service `postgres`. M1: `jwst` via keck. After M3.0: Venus `crdt_*` + `blob` + `workspace_lease` + `dirty`. |
| **SyncProvider** | Host seam: `memory` \| `octobase` (M1) \| `venus` (product after M3.0; `octobase` may stay as a wire alias) \| `y-websocket` (unused kind). Cloud stays this wire, not a y-websocket / nbstore swap. |
| **Comment-commit** | Markdown lease accept: hunks + required why. Opposite of a git snapshot commit. |
| **Pin** | Frozen Yjs bytes for one flush / `T0`. Convert runs on the pin. [LiveSnapshot](./LiveSnapshot/README.md). |
| **Dirty set** | Pages (and blobs / catalog paths) whose clock moved since `last_flushed`. Queue grain is the wiki, not the keystroke. [high-availability.md](./LiveSnapshot/high-availability.md). |
| **Commit Before / After** | Review CRDTs on the hub (M1 language: OctoBase spaces) for one comment-commit. After is editable; edits are the next commit. [datamodel](./datamodel/crdt.md#commit-before-and-after). |
| **LifeIndexing** | **Main wiki feature:** hidden multidimensional index of a **git SHA** — spatial heading binds + temporal comment-commit why. After [LiveSnapshot](./LiveSnapshot/README.md) convert; must not delay `last_flushed`. [Agents](./Agents/README.md), [LifeIndexing](./Agents/LifeIndexing.md). |
| **Direct graph** | Deterministic edges from markdown links, `venus:doc:`, catalog, outline containment. No LLM. |
| **Logical graph** | LLM-typed heading binds (`defines`, `depends-on`, `constrains`, `contradicts`, `supersedes`). Incremental on dirty pages; full re-gist when rolling summaries drift. **Temporal:** those headings → comment-commits + comments (`decided-in`). [LifeIndexing](./Agents/LifeIndexing.md). |
| **Bind** | Pin of a markdown selection at a SHA (`docId` + `blockIds`) plus the LifeIndexing **pack**. Optional `productSha` + CodeGraph / Aider when the recipe asks. [agentic-binding](./Agents/agentic-binding.md), [code-bind](./Agents/code-bind.md). |
| **productSha** | Pin of the product remote at `productBranch`. CodeGraph CLI and/or Aider maps/reviews this tree, not the live worktree. **Select** which: [code-bind](./Agents/code-bind.md#select-codegraph-cli-or-aider-or-both). |

Stack and diagrams: [CRDT](./CRDT/README.md).
