# Pin then convert — git / lease `T0`

**Status:** helper shipped in M2 (`apps/web/src/host/mdgate/pin-from-doc.js`). Git write of `wiki/` is [M3](../venus-implementation-plan.md#m3--git-snapshotter-week), **gated on** [high-availability.md](../LiveSnapshot/high-availability.md) **Acceptance**. Lease acquire reuses the same convert ([M5](../venus-implementation-plan.md#m5--lease--freeze-week)). Apply consumes the pair ([apply.md](./apply.md)). Exporter: [README.md](./README.md). Spectator: [live-pane.md](./live-pane.md).

This is **Path B**: a frozen CRDT clock → markdown + sidecar. It is not the live pane.

## Two directions

| Path | Source | Markdown for | `T0`? |
|---|---|---|---|
| **A — spectator** | Live synced Store | Read-only pane | No. Subscribe / single-flight; `fromDoc` or RAM splice. |
| **B — convert** | **Pinned Yjs bytes** | Git flush, lease `T0`, review `old` | Yes. Full `fromDoc` on an **offline clone** of those bytes. |

Typing in WYSIWYG is Path A. OctoBase sees Yjs updates. The pane photographs the live tree. There is no `Tn`.

Path B exists because an agent (or lease holder) edits **markdown**, which has **no block ids**. Apply diffs that buffer against `markdown_T0` and looks up spans in `sidecar_T0`. Those offsets are true **only** for that baseline string. That pair must be a photograph of a **frozen pin**, not the pane’s last splice.

## Pin first, then copy to markdown

What you pin is **CRDT export bytes**, not `.md`. Files are the output of convert.

```text
live Store (may keep mutating)
    │
    ├─ Y.encodeStateAsUpdate          pinYjsBytes
    │  + encodeStateVector clock
    │         │
    │         ▼
    │   { bytes, clock }              the pin (RAM; keck never sees it)
    │         │
    │         ▼
    │   hydrateM0FromUpdate           new Store, Y.applyUpdate
    │   (no seed, no SyncProvider)
    │         │
    │         ▼
    │   fromDoc(clone)                same exporter as the pane
    │         │
    │         ▼
    └─  { pin, markdown, sidecar }    markdown_T0 + sidecar_T0
```

Live collaboration does not wait on convert. Clients keep sending; keck applies and broadcasts; Postgres persist keeps running. Updates after the pin clock are the **next** flush ([LiveSnapshot](../LiveSnapshot/README.md)).

Do **not**:

- `fromDoc` the live published Store for git / `T0` (the tree can move during stringify).
- Pass `incrementalFromDoc` output as `T0` or write it to `wiki/.venus/ids/`.
- `Y.applyUpdate` of `toDoc` onto the published Y.Doc.

## Helper (Actual)

| Name | File | Does |
|---|---|---|
| `pinYjsBytes(spaceDoc)` | `pin-from-doc.js` | `{ bytes, clock }` from the live Y.Doc |
| `hydrateM0FromUpdate(bytes)` | `workspace.js` | Offline `doc:home` Store. Throws if pin is empty or has no `affine:page`. **Does not seed.** |
| `fromPinnedBytes(bytes, opts)` | `pin-from-doc.js` | Hydrate + full `fromDoc`. Sidecar `clock` / `docId` from the pin. |
| `pinThenFromDoc(store, workspace)` | `pin-from-doc.js` | Pin then `fromPinnedBytes`. Optional shared `blobSources` so image `sourceId`s resolve. |

Tests: `apps/web/src/host/mdgate/pin-from-doc.test.ts` (`pin-then-fromDoc` in [fixtures.md](./fixtures.md)). Pane (`mount-md-pane.js`) must not import this module.

`fromDoc` on the clone is the **same** builder as the pane ([one exporter](./README.md#one-exporter)). Incremental splice is Path A only.

## Why convert is `pinThenFromDoc` (not the pane splice)

This is **decided and implemented**, not an open exporter bug. Path A still uses `incrementalFromDoc` as a cheaper live photograph. Path B never reads that map.

Apply ([apply.md](./apply.md)):

```text
diff(markdown_T0, proposed)  →  attribute with sidecar_T0  →  updateBlock(id)
```

`sidecar_T0` says: for **this** baseline string, bytes `[start, end)` are CRDT `b2`. Those offsets are true only for a string taken from a **frozen Yjs pin**. The pane splice is a cheaper photograph of the **live** Store: later `{start,end}` slide by adapter UTF-16 slice length. A missed dirty id (last-N empty para, linked-doc title middleware) can leave markdown that looks fine and ranges that do not match a full export. Even a **correct** splice is the wrong artifact for apply — it is “whatever this tab last computed,” not a clock.

`pinThenFromDoc` is the convert API: pin bytes first, full `fromDoc` on an offline clone. M3 writes `wiki/`; M5 must call this helper, not the pane map. Path A typing does not use `T0` at all.

## What M3 / M5 still do

| Still later | This helper does not |
|---|---|
| Idle / Flush collect of dirty `docId`s | Convert one page |
| `GET /api/block/…/export` as pin source | Pin from an in-memory `spaceDoc` (tests, and later a snapshotter replica) |
| Write `wiki/<gitPath>.md` + `.venus/ids/<docId>.json` | Return RAM `{ pin, markdown, sidecar }` |
| Keep the pin until lease end | Caller holds `pin.bytes` |
| Catalog / `git mv` / blobs as a dirty set | Optional `blobSources` share for image export |

Flush-before-lease: pin so `T0` is not missing in-memory-not-yet-SQL updates ([LiveSnapshot](../LiveSnapshot/README.md#pin-source-prototype)).

## Files

| File | Role |
|---|---|
| [pin-convert.md](./pin-convert.md) | This note |
| [README.md](./README.md) | Two directions; where each copy lives |
| [live-pane.md](./live-pane.md) | Path A loop |
| [apply.md](./apply.md) | Path B consume: md vs `T0` → hunks |
| [LiveSnapshot](../LiveSnapshot/README.md) | Pin beside keck; git snapshotter |
| [fixtures.md](./fixtures.md) | `pin-then-fromDoc` row |
