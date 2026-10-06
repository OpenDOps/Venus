# M4 step-tree — React / network issues

Review of [step-tree](./plan.md#5-step-tree) after it shipped: `CatalogTree` + `App` page switching. Named e2e (`e2e/m4-*.spec.ts`) is green. This is remaining correctness, React cost, and socket behavior — **not** a board reopen unless these are fixed in a follow-up.

**Code:** `apps/web/src/App.tsx`, `apps/web/src/host/catalog/CatalogTree.tsx`, `drop.js`, `schema.js`, `apps/web/src/host/workspace.js`, `apps/web/src/host/providers/venus-hub-provider.js`. `@headless-tree/react` `useTree` (1.7.0) calls `setConfig` every render; the tree instance is stable.

**Status check:** 2026-10-06 against that code. **16 fixed**, **0 open**, **2 product** (leave as-is).

| Status | Meaning |
|---|---|
| `fixed` | Proposed fix is in the tree/open code |
| `open` | Not implemented |
| `product` | Matches the M4 plan / wire A; not a bug to ship in this follow-up |

**What holds (do not “fix”):** headless-tree is a view — custom `onDrop` writes catalog ops, not `createOnDropHandler`. Home cannot change parent (`canCatalogDrop` / `home_protected`). `createDoc` does not open a page socket. Click opens via `openWorkspaceDoc`; home + catalog stay connected. Header undo binds the open `Store` (`canUndo$` / `canRedo$`). Outline is still headings. Wire A: disconnect the previous extra page except home. SharedWorker is [step 8](./plan.md#8-step-shared-worker), not a one-tab cache.

| # | Sev | Kind | Status | Issue |
|---|---|---|---|---|
| [1](#1-failed-open-impersonates-a-missing-doc) | High | Logic + net | `fixed` | Failed open impersonates a missing doc |
| [2](#2-abandoned-opens-keep-opening-sockets) | High | Network | `fixed` | Abandoned opens keep opening sockets |
| [3](#3-retry-storms-the-hub-hydrate-path) | Med | Network | `fixed` | Retry storms the hub hydrate path |
| [4](#4-empty-hydrate-can-seed-a-blank-page) | Med | Network | `fixed` | Empty hydrate can seed a blank page onto the hub |
| [5](#5-multi-select-drop-reuses-one-dest-slot) | Med | Logic | `fixed` | Multi-select drop reuses one dest slot |
| [6](#6-drop-is-n-catalog-transactions) | Med | Logic | `fixed` | Drop is N catalog transactions |
| [7](#7-three-observedeep-listeners) | Med | React | `fixed` | Three `observeDeep` listeners, two `setState` |
| [8](#8-setrev-plus-rebuildtree) | Med | React | `fixed` | `setRev` plus `rebuildTree` is a double render |
| [9](#9-inline-onopendoc-no-memo) | Med | React | `fixed` | Inline `onOpenDoc`, no memo |
| [10](#10-getchildren-full-scans-and-gitpath) | Med | Perf | `fixed` | `getChildren` full-scans nodes and joins `gitPath` |
| [11](#11-debug-git-log-poll-redraws-the-tree) | Med | React | `fixed` | Debug git-log poll redraws the tree |
| [12](#12-every-leaverevisit-is-a-new-page-ws) | Med | Network | `product` | Every leave/revisit is a new page WS |
| [13](#13-missing-node-rendered-as-a-folder-stub) | Low | Logic | `fixed` | Missing node rendered as a folder stub |
| [14](#14-folder-left-click-toggles-expand) | Low | Logic | `fixed` | Folder left-click toggles expand |
| [15](#15-per-row-style-objects) | Low | React | `fixed` | Per-row style objects and `className` joins |
| [16](#16-full-editor-remount-on-page-switch) | Low | React | `product` | Full editor remount on every page switch |
| [17](#17-waituntilsynced-is-provider-global) | Low | Network | `fixed` | `waitUntilSynced` is provider-global |
| [18](#18-haschild-scans-the-map-every-render) | Low | Perf | `fixed` | `hasChild` scans the map every render |

**Fix order:** [#1](#1-failed-open-impersonates-a-missing-doc)–[#11](#11-debug-git-log-poll-redraws-the-tree) and Low leftover [#13](#13-missing-node-rendered-as-a-folder-stub)–[#15](#15-per-row-style-objects), [#17](#17-waituntilsynced-is-provider-global), [#18](#18-haschild-scans-the-map-every-render) are done. [#12](#12-every-leaverevisit-is-a-new-page-ws) and [#16](#16-full-editor-remount-on-page-switch) stay product.

---

## 1. Failed open impersonates a missing doc

**Status:** `fixed` — `App.tsx` `openDoc` catch keeps the last good page (no switch to home). Already-open extra binds `getDoc` → `setOpenStore`. Auto-switch to home is still only the catalog-miss `observeDeep` path. Vitest: `openDoc catch does not impersonate a missing doc`.

**Where:** `App.tsx` `openDoc` `catch`; `extraDocIdRef` early-return.

### Problem

`resolveOpenDocId` mapping a missing or non-doc catalog id to home is correct (deleted leaf, folder click). The `openDoc` `catch` does the **same UI transition** when `openWorkspaceDoc` throws: `setOpenDocId(PAGE_DOC_ID)` + `setOpenStore(homeStore)` even though the catalog row still exists.

Headless-tree selection is independent of `openDocId`. The user sees: tree row still selected (uuid), header title `home`, editor is the home seed. If they were already on page A and B fails, `extraDocIdRef` still points at A: UI shows home, A’s socket stays up, and clicking A hits the early-return and **does not restore A’s Store**.

### Flow

**From home, click a created page that fails to sync**

```text
click uuid
  → onPrimaryAction → openDoc(uuid)
  → resolveOpenDocId finds KIND_DOC → nextId = uuid
  → extraDocIdRef is null (still on home)
  → openPageStore: connect + waitUntilSynced, 3×, then throw
  → catch: setOpenDocId(home), setOpenStore(homeStore)
  → extraDocIdRef still null
  → tree is-selected = uuid; is-open = home; venus-page-title = home
```

**Already on page A, click B, B fails**

```text
extraDocIdRef = A, openStore = A
click B → openPageStore(B) throws
catch: UI = home Store, openDocId = home
extraDocIdRef still A   ← A socket still connected
click A → extraDocIdRef === nextId → setOpenDocId(A); return
         ← does not setOpenStore; editor stays home
```

The e2e “click the uuid, header stayed home” failure is this class (hub `ensure_doc` hydrate), not a catalog miss.

### Proposed fix

- Treat WS/hydrate failure as an **error**, not as `resolveOpenDocId` miss. Keep the last good `openDocId` / `openStore`. Optional: surface the error once; do not silently switch to home.
- On failure, if you *do* decide to show home: disconnect `extraDocIdRef` and set it to `null` (same as the explicit home branch).
- Early-return `extraDocIdRef.current === nextId` must also `setOpenStore` from that doc’s Store (or drop the early-return and always bind store + id together).
- Auto-switch to home only when `getNode(catalog, openDocId)` is gone (the existing `observeDeep` check), not when connect throws.

---

## 2. Abandoned opens keep opening sockets

**Status:** `fixed` — `openPageStore` in `workspace.js` takes `AbortSignal`; App aborts the previous controller on a new `openDoc` (and on unmount). Abort during wait or backoff does not start another `connect`. Same-id in-flight clicks coalesce. Vitest: `openPageStore does not connect when signal is already aborted`, `abort during wait does not retry connect`, `abort during backoff does not reconnect`, `openDoc aborts in-flight openPageStore when navigating away`.

**Where:** `App.tsx` `openPageStore`; `openWorkspaceDoc` already accepts `options.signal`.

### Problem

Retries are not tied to `openGenRef` and never pass the `AbortSignal` that `openWorkspaceDoc` / `waitUntilSynced` already honor. Navigating away does not stop in-flight connects. Each `VenusHubProvider.connect` starts with `disconnect(docId)` and `this.synced = false`, so a stale open also flaps the provider-global ready flag and hits hub `ensure_doc`.

### Flow

```text
click A                    gen = 1, openPageStore(A) starts
  connect(A)               new WS ?doc=A, provider.synced = false
  waitUntilSynced          up to 15s
click home (or B)          gen = 2, extra disconnected / B starts
  openPageStore(A) still   attempt 1 fails → disconnect(A) → delay 250ms
                           attempt 2 connect(A) again
                           attempt 3 …
  gen mismatch (only after all retries return)
  if extraDocIdRef !== A   disconnect(A)
```

Worst case: three full handshakes (≈45s) for a page the user already left. Home/catalog sockets stay up; the abandoned page still makes the hub hydrate.

### Proposed fix

- Pass `AbortSignal` from `openDoc` into `openPageStore` → `openWorkspaceDoc`. Abort on `openGenRef` bump (new `AbortController` per gen).
- Check `gen !== openGenRef.current` **inside** the retry loop, before `connect`, not only after the whole `openPageStore` returns.
- On abort: `provider.disconnect(nextId)` if `extraDocIdRef` is not that id (same stale-completion rule as today).

Do this **before** adding more retries.

---

## 3. Retry storms the hub hydrate path

**Status:** `fixed` (client). `openPageStore` retries only transport timeout / `ECONNREFUSED`, with `OPEN_PAGE_RETRY_MS` (2s) backoff. Websocket error/close (how hub hydrate failure shows up) is one `connect`. Hub `ensure_doc` nested transaction is still a hub bug; the client no longer hammers it. Vitest: `isOpenPageTransportError`, `does not retry websocket close`, `does not retry hub websocket error`, `retries transport timeout until success`, `retries ECONNREFUSED then succeeds`.

**Where:** `openPageStore` (3 attempts, `delay(250 * (attempt + 1))`); hub `ensure_doc`.

### Problem

The 3× retry was added because the first click in e2e failed: hub `ERROR ensure_doc … error=hydrate` plus `already a transaction in progress` while home was still fetching blobs. Immediate disconnect + 250ms reconnect **repeats** that path. It masks a hub bug and can make nested-transaction hydrate worse.

### Flow

```text
boot: home WS + catalog WS already syncing / blob fetch
click created uuid
  connect(uuid) → hub ensure_doc(uuid) while a tx is in progress
  hydrate fails → WS error/close → waitUntilSynced rejects
  disconnect(uuid)
  250ms later connect(uuid) again → ensure_doc again
  ×3
```

Client looks “flaky”; hub logs a storm. A later attempt can succeed and hide the race.

### Proposed fix

- After [#2](#2-abandoned-opens-keep-opening-sockets): one connect, or backoff in seconds not hundreds of ms, and only retry **transport** close/timeout — not hydrate errors (those need hub fix).
- Hub: `ensure_doc` must not start a nested transaction during blob/hydrate of another doc in the same Room. Client retries must not be the fix for that.

---

## 4. Empty hydrate can seed a blank page onto the hub

**Status:** `fixed` — `openWorkspaceDoc` seeds empty `affine:page` only for `kind: memory` (or `seedIfEmpty: true`). Venus/y-websocket empty after sync throws `EmptyPageSyncError` and disconnects. Catalog `createDoc` seeds locally (no socket) so a minted uuid still has a root to sync up. `openPageStore` does not retry this error. Vitest: `openWorkspaceDoc on venus does not seed a blank page after empty sync`, `createDoc seeds empty affine:page locally without connecting`, `openPageStore does not retry empty venus sync`, `shouldSeedEmptyPage is memory-only`.

**Where:** `workspace.js` `openWorkspaceDoc` after `waitUntilSynced`.

### Problem

If sync step 2 completes but the Y.Doc has no `affine:page` root, the client runs `seedEmptyPage` on the **live** Store and broadcasts it. That is correct for a brand-new uuid that has never been persisted. It is wrong if the hub acknowledged sync with an **empty** doc because `ensure_doc` hydrate failed. Retries ([#3](#3-retry-storms-the-hub-hydrate-path)) increase the chance of writing a blank page over a page that exists in Postgres.

### Flow

```text
createDoc                  mints uuid, catalog row, no page socket
click uuid
  connect + sync step 2    hub hydrate error → empty Y.Doc still “synced”
  hasPageRoot == false
  store.load(() => seedEmptyPage(store))
  Y update origin ≠ REMOTE → WS send → hub apply
  published page is now empty affine:page
```

Memory provider: `synced` is already true; seed-empty is the intended first-open path. Hub mode must distinguish “never existed” from “hydrate failed”.

### Proposed fix

- Seed empty only when the hub (or a dedicated flag) says the space is new — not whenever `hasPageRoot` is false after a sync that may have been empty-by-error.
- If hydrate/WS failed, throw ([#1](#1-failed-open-impersonates-a-missing-doc)); do not seed.
- Do not `seedEmptyPage` on retry N if attempt 1 already connected an empty doc.

---

## 5. Multi-select drop reuses one dest slot

**Status:** `fixed` — `applyCatalogDrop` walks first-to-last and sets `afterId` to the just-placed node so each key is in a new gap. Vitest: `applyCatalogDrop multi-item uses a fresh dest slot per id`, `applyCatalogDrop multi-item reparent keeps drag order and distinct keys`.

**Where:** `drop.js` `destFromDrop` + `applyCatalogDrop`; `selectionFeature` is on.

### Problem

Shift-select can drag several rows. Destination `afterId` / `beforeId` is computed **once** from the sibling list with dragged ids skipped. Every dragged id then `setOrder` / `reparent` with that **same** pair. `generateKeyBetween(after, before)` is deterministic: duplicate fractional `order` keys. `compareNodes` tie-breaks by `id`, so visual order is not drag order. Single-row e2e is fine.

### Flow

```text
Shift-select P1, P2 (P1 then P2 in the list)
drop between A and B in folder F
  destFromDrop → afterId=A, beforeId=B
  applyCatalogDrop:
    setOrder(P1, { after: A, before: B })  → key K = generateKeyBetween(A,B)
    setOrder(P2, { after: A, before: B })  → same K
  siblings sort: order K, then id
```

Second item never uses “after P1” as the new gap.

### Proposed fix

- Apply dest slots **last-to-first** (or first-to-last with `afterId` updated to the just-placed node) so each key sits in a fresh gap. **Shipped:** first-to-last walk in `applyCatalogDrop`. Unordered dest (`{ parentId }` only) still appends the first item, then chains `afterId`.
- Or drop only the primary dragged item until multi-drag is a product requirement; keep `canDrop` length-1.
- Cover with Vitest: two ids, one dest, assert distinct `order` and list order.

---

## 6. Drop is N catalog transactions

**Status:** `fixed` — `applyCatalogDrop` wraps the loop in one `catalog.transact`; nested `setOrder` / `reparent` txs join. Vitest: `applyCatalogDrop multi-item is one catalog transaction`.

**Where:** `applyCatalogDrop` loop; `ops.js` `setOrder` / `reparent` each call `catalog.transact`.

### Problem

One user drop is one intent. Each op opens its own Yjs transaction → one update per item → hub apply/broadcast per item → `observeDeep` / `rebuildTree` per item ([#8](#8-setrev-plus-rebuildtree)). Two-item drop = two WS catalog updates.

### Flow

```text
onDrop([P1, P2], target)
  applyCatalogDrop
    transact setOrder(P1)  → Y update → hub → all tabs observeDeep
    transact setOrder(P2)  → Y update → hub → all tabs observeDeep
```

Remote tab B sees P1 move, then P2; never one atomic drop.

### Proposed fix

- Add a catalog-level `transact` around the loop in `applyCatalogDrop`, **or** a batch op that `putNode`s all dragged ids inside one `catalog.transact`. Nested `transact` in yjs joins the outer tx — if `setOrder` stays as-is, wrapping the loop is enough. **Shipped:** outer wrap in `applyCatalogDrop`; `setOrder` / `reparent` unchanged.
- One update, one rebuild, one hub persist tick for the drop.

---

## 7. Three `observeDeep` listeners

**Status:** `fixed` — `listenCatalogHost` is the only `nodesMap.observeDeep`. App applies title + auto-home from the live open id (`applyCatalogHostChrome` on page switch so the subscription does not rebind). CatalogTree does not observe; it exposes `rebuildRef` → `tree.rebuildTree()`. Auto-home stays in App. Vitest: `listen.test.ts`.

**Where:** `listen.js`; `App.tsx`; `CatalogTree.tsx`.

### Problem

All three subscribe to `nodesMap(catalog).observeDeep`. Any catalog tx (create folder at root, rename a cousin) runs three JS callbacks. Title `setState` on every event (React 19 bails if the string is unchanged). Tree always rebuilds. Auto-switch only `setState`s when the open node is gone.

### Flow

```text
rename a sibling in the tree
  catalog.transact putNode
  Yjs afterTransaction
    App title observer     → setPageTitle(same or new string) → maybe App render
    App auto-home observer → getNode(openDocId) exists → no setState
    CatalogTree observer   → setRev + rebuildTree → CatalogTree render(s)
```

Three subscriptions also mean three `unobserveDeep` on unmount / `openDocId` change (title and auto-home effects rebind when `openDocId` changes).

### Proposed fix

- One host-level `observeDeep` (or `catalog.on('update')`) that: rebuilds the tree, updates title from the **open** node, and auto-homes if that node is missing. **Shipped:** `listenCatalogHost` + App chrome on page switch; CatalogTree `rebuildRef`.
- Or: title observes only `nodes.get(openDocId)` (that Y.Map), not the whole map.
- Keep auto-home out of the tree component.

---

## 8. `setRev` plus `rebuildTree`

**Status:** `fixed` — `setRev` is gone. Catalog mutations update the tree only via `rebuildTree` (`rebuildRef`). `CatalogTree` is `React.memo` with `onOpenDoc={openDoc}` so header `setPageTitle` does not render the tree again. Title `setState` bails when the string is unchanged. Vitest: `memo(function CatalogTree)`, `onOpenDoc={openDoc}`.

**Where:** `CatalogTree.tsx`; `App.tsx` `listenCatalogHost` `onChange` / `onTitle`.

### Problem

`useTree` identity is stable (`useState(() => createTree)`). `rebuildTree()` already `setState`s inside the library and re-renders CatalogTree. `setRev((n) => n + 1)` forces a **second** CatalogTree render per catalog event. `dataLoader` reads the live Y.Doc; a dummy rev is not required for freshness.

### Flow

```text
catalog tx
  bump()
    setRev(n+1)           → React render 1 → useTree setConfig
    tree.rebuildTree()    → library setState → React render 2 → setConfig again
```

`useEffect(..., [catalog, tree])` does **not** re-subscribe every render (`tree` is stable). The extra cost is the dummy state.

### Proposed fix

- Delete `setRev`. Call only `tree.rebuildTree()` in the observer. **Shipped** (with #7).
- If a render is needed without rebuild (toolbar `hasChild`), read catalog in render; Y.Doc is mutable and the rebuild already scheduled an update.
- Title `setState` must not render CatalogTree again: **`React.memo` + stable `onOpenDoc={openDoc}`**.

---

## 9. Inline `onOpenDoc`, no memo

**Status:** `fixed` — `onOpenDoc={openDoc}`, `React.memo(CatalogTree)`, git-log/flush lifted to `VenusDebugBar` (sibling of `m0-columns`, not App state). Vitest: `git-log poll is not App state; debug bar is a sibling of the tree columns`, `parseGitLog` / `sameGitLogShas`.

**Where:** `App.tsx`; `CatalogTree.tsx`; `VenusDebugBar.tsx`.

### Problem

`openDoc` is `useCallback(..., [])` — stable. The wrapper `(id) => { void openDoc(id); }` is a **new function every App render**. `CatalogTree` is not `React.memo`. Any App `setState` (git-log poll, page title, `openStore`) re-renders the tree and rewrites `useTree` config (`onDrop`, `canDrop`, `dataLoader`, `features` array).

Library `setConfig` every render is expected; feeding it new closures from unrelated App state is not.

### Flow

```text
SHOW_DEBUG: setInterval 2s → fetch git/log → setGitLog(new array)
  App render
    onOpenDoc={new fn}
    CatalogTree render
      useTree({ ...config, onPrimaryAction closes over new onOpenDoc })
      setConfig(prev => ({ ...prev, ...config }))
      map rows, new style objects
```

Page title `setState` after a rename of the **open** node does the same.

### Proposed fix

- Pass `onOpenDoc={openDoc}` (async vs `void` is fine). **Shipped** with [#8](#8-setrev-plus-rebuildtree).
- `React.memo(CatalogTree)` with props `catalog`, `workspace`, `selectedDocId`, `onOpenDoc`. **Shipped** (`rebuildRef` is stable).
- Lift debug git-log UI **below** the tree (sibling, not parent state) so poll cannot redraw CatalogTree — see [#11](#11-debug-git-log-poll-redraws-the-tree). **Shipped:** `VenusDebugBar` owns poll + Flush; App does not `setGitLog`.

---

## 10. `getChildren` full-scans and `gitPath`

**Status:** `fixed` — CatalogTree rebuild refreshes one `childrenIndex` and passes it to every `getChildren`. Index / tree `getItem` skip `withGitPath`. Missing `getItem` is an `unknown` leaf, not a wiki-root folder clone. Vitest: `childrenIndex skips gitPath join; getNode still joins`, `CatalogTree rebuild uses one childrenIndex and skips gitPath join`.

**Where:** `CatalogTree` `dataLoader.getChildren` → `childrenOf` → `childrenIndex`; `getItem` → `getNode` → `withGitPath`.

### Problem

The tree displays `name` only. Every `rebuildTree` calls `getChildren` for each expanded folder. With no shared index, each call rescans **all** nodes, groups by `parentId`, sorts, and derives `gitPath` (ancestor `gitName` join). Cost is O(expanded × N) per catalog tx. Fine at N≈20; not the loader the view needs.

### Flow

```text
observeDeep → rebuildTree
  getChildren(wiki:root)     childrenIndex() full scan + gitPath all nodes
  getChildren(folder:spec)   childrenIndex() again
  getChildren(other expanded) childrenIndex() again
  getItem(each visible id)   getNode → gitPathOf walk
```

`destFromDrop` also calls `childrenOf` once per drop (acceptable).

### Proposed fix

- Build **one** `childrenIndex(catalog)` per rebuild (or per `observeDeep`) and pass it into `childrenOf(catalog, parentId, index)`. **Shipped:** `childrenIndexRef` in CatalogTree `rebuildRef`.
- Tree loader: skip `withGitPath` (sibling `order` + `id` + `name` + `kind` are enough). Keep `gitPath` for Flush / tests. **Shipped:** `getNode(..., { gitPath: false })`; `childrenIndex` no longer joins.
- `getItem` miss: do not fabricate a folder — see [#13](#13-missing-node-rendered-as-a-folder-stub). **Shipped:** `kind: 'unknown'` leaf.

---

## 11. Debug git-log poll redraws the tree

**Status:** `fixed` — `VenusDebugBar` owns git-log state and the 2s poll. `sameGitLogShas` skips `setState` when the sha list is unchanged. App/CatalogTree do not subscribe. Vitest: `git-log.test.ts`, `chrome.test.ts`.

**Where:** `VenusDebugBar.tsx`; `git-log.js`. Playwright m4 sets `VITE_DEBUG=1`.

### Problem

`parseGitLog` always returns a **new array**. `setGitLog` every 2s re-renders App, therefore CatalogTree ([#9](#9-inline-onopendoc-no-memo)), while the user is dragging or renaming. Production without debug is unaffected. e2e and local hub runs pay it.

### Flow

```text
VITE_DEBUG && SIDECAR_URL
  setInterval(2000)
    GET /git/log?path=spec/home.md
    setGitLog([...])     even if subjects/shas unchanged
    App render → CatalogTree → setConfig
```

### Proposed fix

- Skip `setGitLog` when sha list is equal (compare joined shas). **Shipped:** `sameGitLogShas`.
- Move git-log state into the debug bar component so App/CatalogTree do not subscribe. **Shipped:** `VenusDebugBar`.
- Combined with memo + stable `onOpenDoc`, poll cannot rebuild the tree.

---

## 12. Every leave/revisit is a new page WS

**Status:** `product` — leave as-is for M4. `extraDocIdRef` still disconnects the previous extra except home. SharedWorker is step 8.

**Where:** `App.tsx` `extraDocIdRef`; plan: disconnect previous extra except home.

### Problem

Matches [step-tree](./plan.md#5-step-tree) / wire A. Cost: A → home → A is a full handshake + hydrate every time. There is no in-memory “keep last page socket” cache. SharedWorker (step 8) shares sockets **across tabs**, not across toggles in one tab.

### Flow

```text
open A     connect(A), extraDocIdRef=A
open home  disconnect(A), extra=null, editor=homeStore
open A     connect(A) again, waitUntilSynced, seed-or-load
```

`openWorkspaceDoc` reuses `workspace.getDoc` (Y.Doc in the collection) but **always** `connect`s a new WS (`connect` disconnects that id first).

### Proposed fix

- **Do not change the product rule** for M4 (one extra page socket).
- Optional later: keep the last extra session connected until a third page is opened (LRU of 1). Not SharedWorker.
- Step 8: worker holds the same A URLs; leaving a page in the tab may still drop the tab’s `Y.Doc` listener — design then, not in this tree pass.

---

## 13. Missing node rendered as a folder stub

**Status:** `fixed` — `getItem` miss is `kind: 'unknown'` (shipped with #10). Rebuild runs `pruneGoneTreeItems`: `abortRenaming` and drop gone ids from `selectedItems`. Vitest: `tree-prune.test.ts`.

**Where:** `tree-prune.js`; `CatalogTree` `rebuildRef`.

### Problem

`getNode` miss returns `{ ...WIKI_ROOT, id: itemId }` (`kind: folder`, empty name). A row that dies mid-rebuild (delete, remote remove) can flash as an empty folder instead of disappearing. `onPrimaryAction` will not open it (`kind !== 'doc'`); drop onto it may look like a folder until `canCatalogDrop` fails (`getNode` false).

### Flow

```text
deleteNode(id) / remote delete
  observeDeep rebuildTree
  getChildren still lists id for one frame, or selection still holds id
  getItem(id) → stub folder
  row: empty label, is-folder class
  next rebuild: id gone
```

### Proposed fix

- `getItem`: return `getNode` or a dedicated “unknown” leaf; do not clone wiki root. **Shipped** with #10.
- After delete, `tree.abortRenaming` / clear selection if `selectedId` is gone. **Shipped:** `pruneGoneTreeItems` after `rebuildTree`.
- `rebuildTree` then `getChildren` from a fresh index so the id is not listed. **Shipped** with #10 (`childrenIndexRef`).

---

## 14. Folder left-click toggles expand

**Status:** `fixed` — row click selects (docs still `onPrimaryAction` open). Expand/collapse is the chevron (`data-catalog-expand`). e2e: `left-click spec selects without collapsing children`.

**Where:** `CatalogTree` `activateTreeRow`; chevron sibling of the row button.

### Problem

Left-click on `spec` selects **and** collapses/expands. Selecting spec to hide Delete also collapses it, so a just-created child is not in the DOM. e2e had to right-click (select without toggle). Product: select and expand are the same click.

### Flow

```text
create page under spec (expanded)
left-click spec          → selected = spec, expanded toggled off
Delete hidden (hasChild)
new uuid row not in DOM  → Playwright timeout if it left-clicked spec
```

Right-click path: `item.select()` + `setContextCreateAt`, no toggle.

### Proposed fix

- If the library allows: expand/collapse only on the chevron; row click = select (docs still `onPrimaryAction` open). **Shipped:** override row `onClick` (`activateTreeRow`); chevron calls `expand` / `collapse`. Arrow keys still expand (library hotkeys).

---

## 15. Per-row style objects

**Status:** `fixed` — `data-level` + CSS `--level`; no per-row `paddingLeft` style object. Chevron is a sibling, not nested in the row button.

**Where:** `CatalogTree` `tree.getItems().map`; `CatalogTree.css`.

### Problem

Each visible row allocates `style={{ paddingLeft: … }}` and a `className` array join per render. Cheap at N≈20. Combined with [#8](#8-setrev-plus-rebuildtree)/[#9](#9-inline-onopendoc-no-memo), drag/rename churnes GC.

### Flow

```text
every CatalogTree render
  for each item:
    new className string
    new style object
    <button {...item.getProps()} />
```

### Proposed fix

- CSS `--level` / `padding-inline-start: calc(var(--level) * 16px)` on a data attribute. **Shipped:** `.venus-tree-row[data-level]`.
- Memoize a `TreeRow` on `item.getKey()` + rename/select/open flags if the wiki grows. Not urgent.

---

## 16. Full editor remount on page switch

**Status:** `product` — leave as-is for step-tree. Mount effect deps are still `[session, openStore]` (unmount + remount). Optional later.

**Where:** `App.tsx` mount effect deps `[session, openStore]`.

### Problem

Switching pages unmounts editor, markdown pane, and outline, then mounts again. Correct for undo bind to a new `Store`. Cost: BlockSuite custom element, `fromDoc` for the md pane, blob paint. Returning to home remounts even though `homeStore` is the same object you left.

### Flow

```text
openStore changes A → B
  cleanup: unmountMd, unmountOutline, unmount editor
  mountEditor(el, B), mountMdPane, waitForEditorHost → mountOutline
```

### Proposed fix

- Optional: keep the editor element and `editor.doc = store` (or equivalent) instead of tearing down. Re-subscribe header `$` on the new store (already keyed by `store`).
- Do this after [#1](#1-failed-open-impersonates-a-missing-doc)/[#2](#2-abandoned-opens-keep-opening-sockets). Not required for step-tree DoD.

---

## 17. `waitUntilSynced` is provider-global

**Status:** `fixed` — `whenReady(docId)` waits that session only. `openWorkspaceDoc` / `openCatalog` pass the id. Boot `createM0Workspace` still waits all sessions. Vitest: `whenReady(docId) does not fail when another session closes`, `waitUntilSynced(docId) does not wait on other sessions`.

**Where:** `venus-hub-provider.js` `whenReady`; `workspace.js` `waitUntilSynced`.

### Problem

Opening a third doc waits on home + catalog `ready` (already resolved) plus the new session. Fine today. If catalog reconnects (`connect` sets `provider.synced = false` and a new `ready`), a **page** open waits on catalog too. `waitUntilSynced` early-returns if `provider.synced` is already true — `connect` currently sets it false first, so the early-return is not a skip-new-doc bug.

### Flow

```text
home.ready resolved, catalog.ready resolved, provider.synced true
connect(page)           provider.synced = false, new session.ready
waitUntilSynced         not synced → whenReady = all([home, catalog, page])
                        waits on page.ready
```

If catalog were reconnecting at the same moment, `Promise.all` waits for catalog’s new `ready` as well.

### Proposed fix

- Optional: `whenReady(docId)` for the session just connected. Page open should not fail because catalog’s socket bounced. **Shipped.**
- Keep the global `synced` for boot (`createM0Workspace` waits on home+catalog together). **Shipped** (`waitUntilSynced` without `docId`).

---

## 18. `hasChild` scans the map every render

**Status:** `fixed` — toolbar `hasChild(catalog, id, loadChildrenIndex())`. Home still skipped (`isHome`). `deleteNode` keeps the map scan (no tree index).

**Where:** `CatalogTree` `showDelete`; `schema.js` `hasChild`.

### Problem

Toolbar visibility walks every node (`hasChild` stops at first hit, still O(N) worst case: empty folder check misses until the last node). `childrenOf(selectedId).length` on a cached index is enough. Same scan family as [#10](#10-getchildren-full-scans-and-gitpath).

### Flow

```text
CatalogTree render
  selected = tree.getSelectedItems()[0]
  getNode(selectedId)
  hasChild(catalog, selectedId)   for-loop all nodes
  show Delete or not
```

Runs again on dummy `setRev` and on App re-renders.

### Proposed fix

- `childrenOf(catalog, selectedId, index).length > 0` with the rebuild’s index, or `hasChild` that uses the index. **Shipped:** `hasChild(..., loadChildrenIndex())`.
- Skip the walk when selection is home (`isHome` already hides Delete). **Already true.**

---

## Source

`CatalogTree.tsx`, `App.tsx`, `drop.js`, `workspace.js`, `venus-hub-provider.js`, `schema.js` `childrenIndex` / `hasChild`, `@headless-tree/react` `useTree` `setConfig`. Review 2026-09-15; this note 2026-09-16; statuses checked 2026-10-06. [step-tree](./plan.md#5-step-tree) board stays **done**.
