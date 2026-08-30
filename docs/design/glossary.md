# Glossary

Words that collide in Venus. Product model: [venus-design.md](./venus-design.md). Stores: [datamodel](./datamodel/README.md). Runtime dataflow: [architecture.md](./architecture.md).

## Four “snapshot” senses

| Term | What it is | Pinned? | Browser? | When |
|---|---|---|---|---|
| **Doc export** | HTTP read of the **current** Y.Doc: keck `GET /api/block/venus-m0/export` (Yjs update v1). Next GET can differ. Reads **Postgres**, not keck live memory. | No | No | [M1 step 7](./M1/plan.md#7-step-snapshot) |
| **Pin** | Frozen copy of export bytes (and catalog) at time T. Convert and git run on this copy. Live CRDT is not paused. | Yes | No | M3+ ([LiveSnapshot](./LiveSnapshot/README.md)) |
| **`T0`** | The pin **kept** at lease acquire. Review Before/Diff is vs this clock. | Yes | No | M5 |
| **Git snapshot commit** | WYSIWYG pin → markdown + autocomment (`snapshot: <title>`). Not a review why. | Yes (git SHA) | No | M3 |
| **Markdown projection** | `MarkdownAdapter.fromDoc` of the block tree. Share format, **not** a second live replica. | n/a | Pane is in the app; git is files | [M2](./M2/README.md) ([MDGate](./MDGate/README.md)) |

**Doc export** is the only one M1 implements. A pin **keeps** an export (or a replica encode). Lease `T0` is a pin that outlives the flush. Git flush converts pins, not the live store.

## Related

| Term | Meaning |
|---|---|
| **Live CRDT** | BlockSuite `Store` / `store.spaceDoc` (Y.Doc). Browsers share it over the sync WebSocket. |
| **keck** | OctoBase WebSocket + HTTP front. Compose service `octobase`. Not the database. |
| **Postgres** | M1 persist for Yjs docs **and** blobs. Compose service `postgres`. |
| **SyncProvider** | Host seam: `memory` \| `octobase` \| `y-websocket`. M1 implements `octobase` only. Stock `y-websocket` is the later cloud swap, not the prototype wire. |
| **Comment-commit** | Markdown lease accept: hunks + required why. Opposite of a git snapshot commit. |
| **Pin** | Frozen Yjs bytes for one flush / `T0`. Convert runs on the pin. [LiveSnapshot](./LiveSnapshot/README.md). |
| **Commit Before / After** | Review CRDTs in OctoBase for one comment-commit. After is editable; edits are the next commit. [datamodel](./datamodel/crdt.md#commit-before-and-after). |

Stack and diagrams: [CRDT](./CRDT/README.md).
