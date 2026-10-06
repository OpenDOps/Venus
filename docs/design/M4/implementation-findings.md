# M4 — Implementation review findings (steps 1–7)

This review covers the M4 work that is already marked `done` in [plan.md](./plan.md): the catalog schema and operations, the per-document hub sockets, the tree, the header, `git mv` on publish, and linked-doc export with `.venus/links.json`. The SharedWorker step and the recon step are out of scope because they are not implemented yet.

**Review date:** 2026-10-06. It was run against the uncommitted working tree on branch `M4`.
**Method:** I read the code paths end to end (host → hub → SQL → sidecar cut → convert → git) and checked them against the live `wiki/` repo, which has 36 Flush commits.
**Scope:** logic gaps, leftover code and documentation, logic mistakes, performance, and security. The review itself changed no product code. Follow-up on this file: **C1** is deferred to the gateway, and **H1** is fixed.

| Severity | Meaning |
|---|---|
| **Critical** | An untrusted hub client can write or delete files outside the wiki, or get code to run. |
| **High** | Silent data loss, a Flush that is permanently stuck, wrong content in git, or broken product behaviour on a normal user path. |
| **Medium** | Wrong output in edge cases, or a concurrency gap that needs two tabs or unusual input. |
| **Low** | Performance, hygiene, leftover code or docs, or test gaps with little user impact. |

## Summary

| # | Sev | Kind | Finding |
|---|---|---|---|
| <a id="summary-c1"></a>[C1](#c1) | Critical | Security | **Deferred (gateway).** The sidecar trusts catalog `gitName`, which allows path traversal into the filesystem and into `wiki/.git` |
| <a id="summary-h1"></a>[H1](#h1) | High | Security / logic | **Fixed.** All workspaces flush into the same `wiki/` directory |
| <a id="summary-h2"></a>[H2](#h2) | High | Logic | A page that is created but never opened is dead everywhere |
| <a id="summary-h3"></a>[H3](#h3) | High | Logic | After a reload, linked-doc cards render as deleted and write that state back to the CRDT |
| <a id="summary-h4"></a>[H4](#h4) | High | Logic | Path collisions inside one Flush cause a permanent `git mv would overwrite` error or overwrite a file |
| <a id="summary-h5"></a>[H5](#h5) | High | Logic / ops | A Flush that fails the same way every time is retried every 2 minutes forever, and nobody sees it |
| <a id="summary-h6"></a>[H6](#h6) | High | Logic | `VenusHubProvider` does not reconnect, so edits after a hub restart are silently lost |
| <a id="summary-m1"></a>[M1](#m1) | Medium | Logic | Concurrent delete or reparent leaves orphan or cyclic nodes that are hidden in the tree but published to git |
| <a id="summary-m2"></a>[M2](#m2) | Medium | Logic | Deleting a link target leaves a dangling href in git, later followed by a `./workspace/` URL |
| <a id="summary-m3"></a>[M3](#m3) | Medium | Logic | Exported hrefs are not URL-encoded, and JS and Rust escape link text differently |
| <a id="summary-m4"></a>[M4](#m4) | Medium | Logic | The `pages.yaml` writer can emit invalid YAML, and the reader that parses it is line-based |
| <a id="summary-m5"></a>[M5](#m5) | Medium | Logic | Home and `spec` can be renamed, but the sidecar hard-codes `spec/home.md` |
| <a id="summary-m6"></a>[M6](#m6) | Medium | Logic | Flush edits the working tree before committing, and its database updates are not atomic |
| <a id="summary-m7"></a>[M7](#m7) | Medium | Concurrency | The job lease is never renewed, and `delete_job` does not check the owner |
| <a id="summary-m8"></a>[M8](#m8) | Medium | Perf / DoS | The hub keeps every opened `?doc=` in memory for the whole life of the room |
| <a id="summary-l1"></a>[L1](#l1) | Low | Perf / logic | Rebuilding `links.json` does filesystem I/O inside the database transaction, and its errors are swallowed |
| <a id="summary-l2"></a>[L2](#l2) | Low | Perf | The catalog is decoded and walked several times in one Flush |
| <a id="summary-l3"></a>[L3](#l3) | Low | Perf | The debug bar triggers a full git history walk every 2 seconds |
| <a id="summary-l4"></a>[L4](#l4) | Low | Logic | Commit messages are `snapshot: Venus` or an empty `snapshot:` for most catalog Flushes |
| <a id="summary-l5"></a>[L5](#l5) | Low | Security | Test hooks on `window.__VENUS_*` are shipped in every build |
| <a id="summary-l6"></a>[L6](#l6) | Low | Perf | Every catalog event rebuilds the whole tree, and the header renames on every keystroke |
| <a id="summary-l7"></a>[L7](#l7) | Low | Security | Untrusted names and `pageId`s are written into markdown without full escaping |
| <a id="summary-l8"></a>[L8](#l8) | Low | Security | `POST /flush` can be triggered cross-site and accepts any workspace |
| <a id="summary-l9"></a>[L9](#l9) | Low | Residual | `.venus/ids/doc:home.json` contains a colon, which Windows does not allow in filenames |
| <a id="summary-l10"></a>[L10](#l10) | Low | Residual | Test and documentation gaps (assertions in `m4-link`, `tsc` errors, a dangling README link, stale `links.json` edges) |

---

## Critical

<a id="c1"></a>

### C1. Catalog `gitName` is trusted by the sidecar: path traversal into the filesystem and into `wiki/.git`

[↑ Summary](#summary-c1)

**Status:** `deferred` — gateway. Checking catalog `gitName` before it becomes a filesystem path belongs to the gateway, which is a separate component. It is not implemented in the sidecar or in this M4 slice. The problem and the checks below stay as the brief for that component.

**Problem.** The rules that make a filename safe (no `/`, no leading dot, no control characters, one `.md` suffix) only run in the browser (`git-path.js` `sanitizeDocname` / `uniqueGitName`). The catalog is a plain Y.Doc on an unauthenticated hub, so any client that can open `/collaboration/<ws>?doc=4fe5c16e-…` can write any `gitName`. This includes `../../x`, `/abs/path`, `.git/config`, `.git/hooks/post-commit`, `.venus/links.json`, or a 10 KB name. The sidecar uses that string as a filesystem path without checking it.

**Data flow.**

1. A malicious or buggy client sets `nodes[<id>].gitName = "../../../home/venus/.profile"`, or `".git/config"`, on the catalog doc. The hub persists the update.
2. Flush runs `cut_workspace`, which pins the catalog. `catalog::walk_pin` → `walk_doc` → `git_path_of` → `join_git_names(parent, git_name)` concatenates the strings. If the node has no parent, the raw `gitName` *is* the path.
3. `git::commit_catalog_walk` then:
   - calls `write_page(&wiki.dir, &page.git_path, …)` → `dir.join(git_path)` → `fs::write`. In Rust, `Path::join` with an absolute path *replaces* the base path, and `..` is not normalised. The result is an **arbitrary file write** with the sidecar's user id.
   - calls `git_mv(dir, old, new)` → `fs::rename`, which can **move any file** reachable under the old path.
   - calls `git_rm_page(dir, old.git_path, …)` → `fs::remove_file`. `old.git_path` comes from `page_identity`, which the sidecar itself filled from the previous walk. Rename a node to a traversal path, Flush, then delete the node, and the sidecar **deletes an arbitrary file**.
4. Writing a page to `.git/config` (the markdown body is controlled by the attacker) sets `core.fsmonitor` or `core.hooksPath`. That command runs the next time a developer runs `git status` or `git log` in `wiki/` on the host (`pnpm wiki:*`, IDE git integration). This is **code execution on the developer's machine**. `.venus/pages.yaml` and `.venus/links.json` can also be overwritten through a `gitName` such as `.venus/links.json`.
5. A very long `gitName` fails with `ENAMETOOLONG`, which turns into the stuck job in H5.

**Where.** `crates/venus-sidecar/src/catalog.rs` (`read_node`, `join_git_names`, `git_path_of`). `crates/venus-sidecar/src/git.rs` (`write_page`, `git_mv`, `git_rm_page`). `page_identity` rows written by `catalog::replace_page_identity`.

**Proposed fixes.**
1. **Check every path segment in Rust** inside `walk_doc`, before any path is built. Reject any segment that is empty, `.` or `..`, starts with `.`, contains `/`, `\`, NUL or other control characters, is longer than 255 bytes, or has a case-insensitive name of `.git` / `.venus`. A doc leaf must end in `.md`, and a folder must not. When a node fails the check, do not abort the Flush. Publish it at a safe fallback such as `<parent>/<uuid>.md`, or quarantine it as `_invalid/<uuid>.md`, and log a warning.
2. **Containment check at the write site.** Add a single helper, `safe_join(root, rel) -> Result<PathBuf>`, used by `write_page`, `git_mv`, `git_rm_page`, and `write_blob`. It rejects absolute paths and any `ParentDir` or `RootDir` component, joins the path, and then checks that `canonicalize(parent)` starts with `canonicalize(root)`. Refuse to follow symlinks: check `symlink_metadata` before `remove_file` or `rename`.
3. **Check `page_identity` again when reading it** (`load_old_pages`). Rows written before this fix may already contain bad paths.
4. Defence in depth: run the sidecar container with a read-only root filesystem, mount only `/wiki` as writable, and drop the user's home directory.
5. Test: a catalog with `gitName` values of `../x.md`, `/tmp/x.md`, `.git/config`, and a 300-byte name. Expected result: nothing is written outside `wiki/`, nothing is written under `.git` or `.venus`, the Flush succeeds, and the bad nodes are quarantined.

---

## High

<a id="h1"></a>

### H1. Every workspace flushes into the same `wiki/` directory

[↑ Summary](#summary-h1)

**Status:** `fixed` (2026-10-06). One sidecar process publishes one workspace. `WIKI_WORKSPACE_ID` (unset → the M0 uuid `77e4a2b1-8b40-5979-a73c-fd4477216d00`) is the only id the observer enqueues and the only id a worker claims. `POST /flush` returns 400 for a bad shape and 404 for any other uuid. The first Flush writes `.venus/workspace`; a later Flush for a different id refuses before it writes pages. The hub allowlist is `HUB_WORKSPACES` (same default): HTTP and gRPC reject other ids with 404 / NotFound before a room, a lease, or a blob row. `Hub::new` and `WikiConfig::new` stay unbound so tests can still use a fresh uuid per case. Compose sets both env vars on `hub` and `sidecar`.

**Problem.** The hub accepts **any** UUID-shaped workspace id (`workspace_id_ok` only checks the shape). The sidecar observer creates jobs for **every** workspace that has dirty rows, and `POST /flush?workspace=<any uuid>` creates a job for any id. Each job is then flushed into the single configured `WikiConfig.dir`. The design says there is one wiki per workspace. The code does not enforce that.

**Data flow.**

1. Client X connects to `/collaboration/<random-uuid>` and edits the home page and the catalog. The hub writes `updates` rows for `workspace_id = X`.
2. `observe_once` (`OBSERVE_SQL`) inserts a job for X, because X has dirty rows with no `last_flushed` entry.
3. `flush_claimed(X)` cuts X, walks **X's** catalog, and calls `commit_catalog_walk` on `wiki/`:
   - `write_page(spec/home.md)` overwrites the real workspace's home page.
   - `write_pages_yaml` replaces `.venus/pages.yaml` with X's pages only.
   - `persist_on_flush` merges X's edges into the real `links.json`.
   - `old_pages` is scoped to X, so X does not `git rm` the real workspace's files. However, the real workspace's next Flush reads `pages.yaml` and `links.json` back from disk (`cut.rs` builds `path_to_doc` from `pages.yaml`), so its convert set is calculated from X's data.
4. The result is a commit that mixes two workspaces, and inbound link rewriting stops working for the real workspace until `links.json` is rebuilt.

**Where.** `crates/venus-hub/src/http.rs` (`take_workspace_id` / `workspace_id_ok`). `crates/venus-sidecar/src/queue.rs` (`OBSERVE_SQL`, `FLUSH_SQL`, `flush_now`). `crates/venus-sidecar/src/http.rs` (`flush`, `flush_workspace`). `git.rs` `commit_catalog_walk`.

**Proposed fixes.**
1. Make the mapping from workspace to wiki explicit in the sidecar: `WIKI_WORKSPACE_ID` in config, or a table `wikis(workspace_id, dir)`. `OBSERVE_SQL` and `CLAIM_SQL` should filter on that allowlist. `POST /flush` should return `404` for an unknown workspace.
2. In the hub, add an optional `HUB_WORKSPACES` allowlist. Until M5 auth exists, reject other workspace ids at upgrade time with a close code.
3. Stamp the workspace in the wiki (`.venus/workspace`). Before writing, `commit_catalog_walk` checks the stamp and refuses on a mismatch.

<a id="h2"></a>

### H2. A page that is created but never opened is dead everywhere

[↑ Summary](#summary-h2)

**Problem.** `ops.createDoc` adds the catalog node and calls `ensureWorkspaceDoc`, which runs `workspace.createDoc(uuid)` and then `seedEmptyPageIfNeeded`. This happens **only in the creator's tab and without a socket**. The seed reaches the hub only if that tab later opens the page, because `openWorkspaceDoc` connects the existing Y.Doc. If the user never opens it, the hub has catalog rows but no page rows for that uuid.

**Data flow and observed results.**

1. Tab A runs `createDoc`. The catalog node syncs to the hub. The page Y.Doc exists only in A's memory.
2. Tab B (or tab A after a reload) clicks the node. `openPageStore` → `openWorkspaceDoc` → hub `ensure_doc` returns an empty doc. `hasPageRoot` is false and the provider is `venus`, so it throws `EmptyPageSyncError`. The page **cannot be opened anywhere** once A's tab is gone.
3. Flush: the cut pins only dirty docs, and this uuid has no rows. `walk.pages` still contains the page, so `pages.yaml` and `page_identity` list it, but `write_page` never runs. The `.md` file never exists.
4. Linked-doc export: `catalog_ctx` includes the page, so inbound sources export `[name](relative/path.md)` pointing at a file that does not exist.
5. **Live evidence (`wiki/` HEAD):** `pages.yaml` lists `spec/64d7aca4-….md`, `spec/a2c00a9a-….md`, `spec/f12cf50f-….md`, and `design-6d9e0dad/protocol-6d9e0dad.md`. None of them is in `git ls-files`. `spec/home.md` lines 59–65 link to three of them. `links.json` records `doc:home → {64d7aca4, 6d9e0dad, a2c00a9a}`.
6. The `e2e/m4-link.spec.ts` test passes because it asserts the href in `home.md` and never checks that the target file exists. Step 7.2 asks for the link to *resolve* after a move.

**Where.** `apps/web/src/host/catalog/ops.js` (`createDoc`, `ensureWorkspaceDoc`). `apps/web/src/host/workspace.js` (`openWorkspaceDoc` → `emptyPageSyncError`). `crates/venus-sidecar/src/git.rs` (`commit_catalog_walk` writes `pages.yaml` for every walked page). `crates/venus-sidecar/src/from_doc/mod.rs` (`catalog_ctx`).

**Proposed fixes.**
1. **Seed on create, before the node is visible.** In `createDoc`, connect the new Y.Doc (`provider.connect(uuid, ydoc)`), wait until it is synced, seed, flush the seed, then disconnect. Make the catalog node visible only after the seed is acknowledged. Alternatively, write the node with `pending: true` and clear it after the seed.
2. Or **seed on the hub**: when the catalog gains a `kind:doc` node whose uuid has no rows, the hub (or the first `ensure_doc` for a uuid that the catalog lists) inserts a canonical empty `affine:page` update. The seed has a single owner, so concurrent creators cannot race.
3. In the sidecar, when a walked page has **no rows at all** and is not pinned, either write a stub `.md` file (an empty page with H1 = catalog name) or leave it out of `pages.yaml` and export the inbound link as plain text plus the `venus:doc` comment. Never point an href at a file that does not exist.
4. Stronger e2e: after Flush, assert `git ls-files` contains the target path. Open the created page from a second browser context and expect no `EmptyPageSyncError`.

<a id="h3"></a>

### H3. After a reload, linked-doc cards render as deleted and write that state back to the CRDT

[↑ Summary](#summary-h3)

**Problem.** BlockSuite's `EmbedLinkedDocBlockComponent` decides whether the target exists with `std.workspace.getDoc(pageId)`, and takes its title from `linkedDoc.meta.title`. A fresh tab only creates workspace docs for the pages it opens. Nothing turns catalog `kind:doc` nodes into workspace docs or `docMetas` at boot, and `workspace.meta` is not connected to the hub, so `setDocMeta` in `ops.setDocTitle` stays in the local tab. In every tab except the creator's, every card whose target has not been opened is treated as **deleted**.

**Data flow.**

1. Tab A creates page P, inserts a card in home, and renames P. A sets `docMetas[P].title` locally, and the card shows the right title.
2. Tab B loads, or A reloads. Home renders the card, `workspace.getDoc(P)` is `null`, the title falls back to `'Untitled'`, and the card style is "deleted".
3. In `updated()`, BlockSuite then calls `store.updateBlock(model, { style: 'horizontalThin', xywh })` inside `withoutTransact`. This is a **CRDT write to home** that is synced to the hub. When tab A renders the same card with the doc present, it writes `style: 'horizontal'` back. Two open tabs **flip the style back and forth**, and each flip marks home dirty, so home is converted again on every Flush.
4. Plan 7.2 scenario 1 says the live card title is the catalog name (from `docMetas`), not "untitled". That only holds in the creator tab before a reload. The e2e test only checks `model.pageId` after the reload.

**Where.** `@blocksuite/affine-block-embed-doc/dist/embed-linked-doc-block/embed-linked-doc-block.js` (`linkedDoc`, `docTitle`, `updated`). `apps/web/src/App.tsx` (no materialisation at boot). `apps/web/src/host/catalog/ops.js` (`setDocTitle`). `apps/web/src/host/catalog/listen.js` (does not update metas).

**Proposed fixes.**
1. **Project the catalog into the workspace.** In `listenCatalogHost`, at hydrate and on every catalog change, make sure there is a lazy workspace doc for every `kind:doc` node. Use `workspace.createDoc(id)` with **no seed and no load**, which needs a "create without seeding" path that is separate from `ensureWorkspaceDoc`. Then call `setDocMeta(id, { title: node.name })`. Remove the stub when the node is deleted. This keeps `docMetas` derived from the catalog, which the plan already says must happen ("docMetas.title = catalog name").
2. Make sure the card's `linkedDoc.load()` on a stub does not create a local root. The stub must stay empty until `openPageStore` connects it, otherwise it causes H2-style divergence. If BlockSuite insists on loading, give it a `getDoc` override that returns a read-only shell.
3. Short-term guard: a host-level block-update filter (or `store.readonly` for embed cards) that drops `style` / `xywh` writes coming from `updated()` when the target id is in the catalog.
4. e2e: after the reload, assert the card is not in the deleted style and its title equals the catalog name. Open two contexts and assert home's clock does not move while both are idle.

<a id="h4"></a>

### H4. Path collisions inside one Flush: permanent `git mv would overwrite` or a silent overwrite

[↑ Summary](#summary-h4)

**Problem.** Sibling `gitName`s are made unique only in the local tab, at the moment of the operation (`uniqueGitName`, using a case-sensitive "taken" set). In `commit_catalog_walk` the sidecar processes pages in walk order and runs each move straight away. It never checks whether the new paths are unique, and it never orders the moves.

**Data flows that break.**

- **Delete, then reuse the name** (a normal user path). The user deletes page P (`spec/notes.md`) and renames page Q to "notes". The host allows it because P is no longer a sibling. Flush: the walk loop reaches Q first and runs `git_mv(spec/q.md → spec/notes.md)`. `spec/notes.md` still exists because `git_rm_page` runs *after* the loop. `ensure!(!dst.exists() || same_file)` fails, so **every Flush fails from then on** (see H5).
- **Swap or rename chain.** A `a.md→b.md` and B `b.md→a.md` (for example a→tmp, b→a, tmp→b between two Flushes) fail the same way.
- **Two tabs at once.** Tab A and tab B each rename a different sibling to "spec" at the same moment. Each tab's taken set is clean, so both write `gitName = "spec.md"`. The CRDT merges two siblings with the same path. Both pages are converted and `write_page` runs twice on the same file (**last one wins, the other page's content is lost from git**). `pages.yaml` has a duplicate key, and `page_identity` has two rows with the same path.
- **Case-insensitive filesystem.** `Notes.md` and `notes.md` are distinct to the host's taken set, but they are the same file on APFS / Docker Desktop bind mounts. This produces the overwrite above, or a `same_file` false positive.

**Where.** `apps/web/src/host/catalog/git-path.js` (`uniqueGitName`, taken set). `crates/venus-sidecar/src/git.rs` (`commit_catalog_walk` loop order, `git_mv` guard, `git_rm_page` after the moves). `catalog.rs` `walk_doc` (no duplicate check).

**Proposed fixes.**
1. **Sidecar owns uniqueness.** After `walk_doc`, group pages by `case_fold(git_path)`. If there is more than one, keep a deterministic winner (lowest node id, or oldest order key) and give the others a suffix such as `name-<id8>.md` in the **published** path only. Do not edit the CRDT from the sidecar. Optionally have the host repair it on its next observe.
2. **Two-phase apply.** (a) `git_rm` every removed page first. (b) Move every page whose path changed to a temporary name (`.venus/tmp/<uuid>`). (c) Move each temporary file to its final path. (d) Write the converted pages. This removes the ordering problems with swaps, chains, and reused deleted names.
3. In the host, compare case-insensitively in `uniqueGitName`. After a remote merge, run a repair pass that renames duplicate siblings deterministically, so the tree shows what git will contain.
4. Tests: delete P, rename Q to P's name, then Flush. Swap two names, then Flush. Two concurrent docs rename to the same name, then Flush. Each Flush must succeed, and every page must exist exactly once in git.

<a id="h5"></a>

### H5. A Flush that fails the same way every time is retried every 2 minutes forever, and nobody sees it

[↑ Summary](#summary-h5)

**Problem.** `flush_claimed` deletes the job only when it succeeds. Any error that repeats on every attempt leaves the job in place, and it is claimed again when the 2-minute lease runs out. Examples: H4's `git mv would overwrite`, `catalog pin has no home doc` (a client deleted the home node), `ENAMETOOLONG` (C1), a broken YAML parse, or a disk-full error. The browser's Flush button gets `204` from `POST /flush` regardless, and the debug bar's git log simply stops changing. No new commit will ever be made for that workspace.

**Data flow.** Job inserted → `claim_one` (lease 2 min) → `cut_workspace` ✓ → `commit_pins` ✗ → `Err` propagates → worker logs `warn` → job stays with `lease_until` in the future → after 2 min it is claimed again → the same error.

**Where.** `crates/venus-sidecar/src/queue.rs` (`flush_claimed`, `CLAIM_SQL`, no attempt count). `crates/venus-sidecar/src/http.rs` `flush` (always `204`). `apps/web/src/host/chrome/VenusDebugBar.tsx` (no error state).

**Proposed fixes.**
1. Add `attempts int`, `last_error text`, and `last_error_at timestamptz` to `jobs`. On error, `UPDATE jobs SET attempts = attempts + 1, last_error = $e, not_before = now() + backoff(attempts), lease_until = NULL`. After N attempts, move the job to `failed` (or `dead_jobs`) and stop retrying until the CRDT changes again (the observer clears `failed` when a new dirty clock appears).
2. Add `GET /flush/status?workspace=` (last success sha, last error, attempts). The debug bar shows an error badge. `POST /flush` should return `409` with `last_error` when the job is in `failed`.
3. Split errors into two groups: **transient** (database or I/O timeouts, which keep retrying) and **deterministic** (validation or collision errors). Deterministic errors fail fast, and where possible they are skipped per page instead of per workspace, as in C1 fix 1 and H4 fix 1.

<a id="h6"></a>

### H6. `VenusHubProvider` does not reconnect: edits after a hub restart are silently lost

[↑ Summary](#summary-h6)

**Problem.** Each document session opens one WebSocket. The `close` and `error` listeners only act **before** the first sync (`if (… || session.synced) return`). After that, a closed socket is ignored. `onUpdate` drops local updates while `ws.readyState !== OPEN`, `provider.synced` stays `true`, and nothing reconnects. Since M4 step 2, every open page, the catalog, and home each hold their own socket, so a hub restart or proxy idle timeout breaks all of them at once.

**Data flow.** Hub restarts or nginx closes an idle socket → `close` fires after sync and is ignored → the user keeps typing → `ydoc.on('update')` → `onUpdate` returns early because the socket is not `OPEN` → the edits exist only in browser memory → the user reloads or closes the tab → **the edits are lost**. Other tabs never see them, and Flush never publishes them. Catalog operations (rename, move, create) are lost the same way.

**Where.** `apps/web/src/host/providers/venus-hub-provider.js` (`connect`: `close` / `error` listeners, `onUpdate` guard; there is no reconnect path).

**Proposed fixes.**
1. When the socket closes after sync, set `session.synced = false` and `provider.synced = false`, then reconnect with jittered backoff to the same `?doc=`. When the socket reopens, send `SyncStep1(ydoc)`. Yjs computes the diff, so updates made while offline are sent automatically. Do not drop them in `onUpdate`; let the state vector reconcile on reconnect.
2. Show a connection state in the UI (an "offline / reconnecting" badge in the header) and warn on `beforeunload` while any session is not synced.
3. Optional: add `y-indexeddb` persistence per doc so edits survive a reload while offline.
4. Test: start the hub, open a page, kill and restart the hub, type, wait, reload, and assert the text is there. Repeat for a catalog rename.

---

## Medium

<a id="m1"></a>

### M1. Concurrent delete or reparent leaves orphan or cyclic nodes that are hidden in the tree but published to git

[↑ Summary](#summary-m1)

**Problem.** `deleteNode` only refuses a non-empty folder based on the **local** `hasChild`. `reparent` only refuses a cycle based on the **local** `assertNotCycle`. Neither check holds across tabs once the CRDT merges.

**Data flows.**

- Orphan: tab A deletes the empty folder F while tab B creates a doc inside F. After the merge, the doc's `parentId` points to a node that no longer exists. Host `tree-prune` hides it, so it is **invisible in the tree**. Rust `git_path_of` → missing parent → `""` → `join_git_names("", name)` publishes it at **the wiki root** (`notes.md`).
- Cycle: tab A moves X under Y while tab B moves Y under X. After the merge, X ↔ Y. JS `gitPathOf` and Rust `git_path_of` both cut the cycle by returning only the leaf name, so the published path is a different, unrelated location. The tree hides both nodes.

**Where.** `apps/web/src/host/catalog/ops.js` (`deleteNode` line ~375, `reparent` line ~320, `assertNotCycle`). `apps/web/src/host/catalog/tree-prune.js`. `crates/venus-sidecar/src/catalog.rs` (`git_path_of`).

**Proposed fixes.**
1. Add a deterministic **repair pass** in `listenCatalogHost` (only one host repairs: the tab with the lowest client id, or each tab idempotently). Move orphans to the root (or a `lost+found` folder). Break each cycle at the node with the greatest id by moving it to the root. Because the result does not depend on which tab runs it, concurrent repairs converge.
2. In the sidecar, apply the same rule (orphan → root, cycle broken at the greatest id) so git agrees with the repaired tree, and log each repair.
3. Show orphans in the tree under "Unfiled" instead of pruning them.

<a id="m2"></a>

### M2. Deleting a link target leaves a dangling href in git, later followed by a `./workspace/` URL

[↑ Summary](#summary-m2)

**Problem.** The convert set is: dirty bodies ∪ inbound sources of pages whose path or name changed ∪ pages whose own directory changed and that have outbound links. It does **not** include inbound sources of a **deleted** target.

**Data flow.** Home links to P. P is deleted, and Flush runs `git_rm` on `P.md`, but `home.md` keeps `[P](P.md)`, which is now dangling. Later, home is edited and converted again. `catalog_ctx` no longer has P, so `linked_doc_inline` falls back to `[untitled](./workspace/<ws>/<P>)`. That form is explicitly forbidden in git ([subset](../MDGate/subset.md#linked-doc-stable-form)). The card itself still exists in home's CRDT.

**Where.** `crates/venus-sidecar/src/links.rs` (`convert_set`). `crates/venus-sidecar/src/from_doc/markdown.rs` (`linked_doc_inline` fallback). `apps/web/src/host/mdgate/from-doc.js` (`catalogLinkedDocLink` returns `null`, so the adapter default is used).

**Proposed fixes.**
1. In `convert_set`, add `inbound[d]` for every `d` in `old_pages − walk`.
2. Define a stable export for a missing target, for example the plain text `~~name~~ <!-- venus:doc:<id> missing -->`, or `[name](<last known path>)` with a `missing` marker. Never write `./workspace/`. Use the same form in JS and Rust and add a parity test for it.

<a id="m3"></a>

### M3. Exported hrefs are not URL-encoded, and JS and Rust escape link text differently

[↑ Summary](#summary-m3)

**Problem.** Filenames can contain spaces and parentheses (`spec/Renamed venus page.md` is in the live wiki). `posix_relative` / `posixRelativeFromFiles` output is inserted into `[text](href)` as-is. CommonMark does not allow spaces in a bare link destination, and an unbalanced `)` ends it early. As a result, a link to `Renamed venus page.md` renders as literal text on any markdown viewer, and M6 apply (remark) will not parse it as a link. Separately, Rust `escape_text` escapes `` \ ` * _ [ ] < > `` while JS `escapeLinkText` only escapes `\ [ ]`. For a name like `my_page`, the pane (JS) and git (Rust) produce different text.

**Where.** `crates/venus-sidecar/src/links.rs` `posix_relative`. `crates/venus-sidecar/src/from_doc/markdown.rs` (`linked_doc_inline`, `escape_text`). `apps/web/src/host/mdgate/from-doc.js` (`posixRelativeFromFiles`, `escapeLinkText`, `findLinkedDocInsert`, which also stops at the first `)`).

**Proposed fixes.**
1. Percent-encode each path segment (space → `%20`, `(` `)` → `%28` `%29`, plus non-ASCII if needed), or use the angle-bracket form `[text](<path with spaces.md>)`. Do this in both JS and Rust, and make `findLinkedDocInsert` understand the chosen form.
2. Use one escaping table for link text in both languages, and extend the JS↔Rust parity fixture with names containing `_ * ( ) [ ]` and spaces.

<a id="m4"></a>

### M4. The `pages.yaml` writer can emit invalid YAML, and the reader that parses it is line-based

[↑ Summary](#summary-m4)

**Problem.** `needs_yaml_quotes` misses newlines, a leading `-` / `?` / `!`, YAML 1.1 booleans and nulls (`yes`, `no`, `on`, `off`, `true`, `null`, `~`), and values that look numeric (`1e3`, `0x10`). `yaml_quoted` does not escape `\n` either. A catalog `name` such as `"a\nb"` breaks the document. A name `true` is read back as a boolean. `links::path_to_doc_id_from_pages_yaml` parses this file **line by line**, so a quoted key or a multi-line value gives the wrong `path → doc` map, and the convert set then misses inbound rewrites.

**Where.** `crates/venus-sidecar/src/catalog.rs` (`pages_yaml`, `needs_yaml_quotes`, `yaml_quoted`). `crates/venus-sidecar/src/links.rs` (`path_to_doc_id_from_pages_yaml`).

**Proposed fixes.**
1. Write the file with `serde_yaml` (or always use double quotes with full JSON-style escaping, since JSON strings are valid YAML).
2. Read it with the same library. Even better, build `path_to_doc` from `page_identity` (SQL, already loaded as `old_pages`) and stop reading the working tree inside the cut.
3. Reject control characters in `name` on the sidecar side (same check as C1).

<a id="m5"></a>

### M5. Home and `spec` can be renamed, but the sidecar hard-codes `spec/home.md`

[↑ Summary](#summary-m5)

**Problem.** `reparent` and `deleteNode` refuse to touch home (`home_protected`), but `rename` does not. The tree and the header title input both allow renaming home or the `spec` folder. The sidecar still assumes `GIT_PATH = spec/home.md` in several places: `cut` sets `pins.git_path`, `commit_home_only` uses it, `ensure_repo` always creates `spec/`, `is_catalog_log_path` only accepts `spec/home.md`, and the debug bar polls `CATALOG_GIT_LOG_PATH`.

**Data flow.** The user renames home to "Start" → Flush → `git_mv(spec/home.md → spec/Start.md)` → the debug bar's git log (`path=spec/home.md`) stops at the rename. The M3 e2e tests and the `git log` endpoint lose track of home, and `ensure_repo` recreates an empty `spec/`.

**Where.** `apps/web/src/host/catalog/ops.js` (`rename`). `apps/web/src/host/catalog/CatalogTree.tsx` (rename allowed on home). `apps/web/src/App.tsx` header `onTitleChange`. `crates/venus-sidecar/src/git.rs` (`GIT_PATH`, `ensure_repo`, `is_catalog_log_path`, `log_path`). `crates/venus-sidecar/src/cut.rs` (`pins.git_path`).

**Proposed fixes.**
1. Decide on the product rule. Either (a) home's `gitName` is fixed (rename only changes `name`, and the sidecar always publishes home to `spec/home.md`), or (b) home can move and the sidecar resolves its path from the walk.
2. For (b): `git/log?doc=doc:home` resolves the path through `page_identity` and uses `--follow` semantics (walk the history across renames). Remove `GIT_PATH` from `cut` and `ensure_repo`.

<a id="m6"></a>

### M6. Flush edits the working tree before committing, and its database updates are not atomic

[↑ Summary](#summary-m6)

**Problem.** `commit_catalog_walk` runs `git_mv`, `write_page`, `git_rm_page`, `pages.yaml`, `links.json`, and blob writes directly in `wiki/`, then `add_all` + `update_all` + commit. If any step fails part-way, the half-applied tree stays on disk. The next successful Flush commits it with `add_all`, together with any stray file in `wiki/`. After the commit, `replace_page_identity` and `upsert_last_flushed` run as separate statements. If the process crashes between the git commit and those writes, `old_pages` no longer matches git. A later rename back to the old path then leaves the moved file behind (`git_mv` skips because `src` does not exist), and it stays tracked forever.

**Where.** `crates/venus-sidecar/src/git.rs` (`commit_catalog_walk`, `commit_tree` `add_all`). `crates/venus-sidecar/src/queue.rs` (`flush_claimed` after `commit_pins`).

**Proposed fixes.**
1. Build the commit from the git **index or tree** (`git2::TreeBuilder` based on HEAD) instead of mutating the working tree, then `checkout_head(force)` once the commit succeeds. Alternatively, stage only the paths this Flush touched (no `add_all ["."]`) and run `git reset --hard HEAD` + `clean` at the start of every Flush.
2. Write `page_identity` and `last_flushed` in **one** SQL transaction. At the start of a Flush, if HEAD's `.venus/pages.yaml` and `page_identity` disagree, rebuild `page_identity` from HEAD (git is the record of what was published).

<a id="m7"></a>

### M7. The job lease is never renewed, and `delete_job` does not check the owner

[↑ Summary](#summary-m7)

**Problem.** `CLAIM_SQL` sets `lease_until = now() + 2 min` and nothing extends it. A Flush that takes longer than 2 minutes (a large wiki, `rebuild_from_wiki`, a slow disk) is claimed by a second worker (`SNAPSHOT_WORKERS=2`) **while the first is still writing to the same `wiki/`**, and the two corrupt the working tree and index. `delete_job` deletes by `workspace_id` only, so the slower worker deletes the newer job. A `POST /flush` that arrives during a running Flush updates `not_before` on the same row, and that row is then deleted on success, so the request is lost until the observer's idle timer runs out.

**Where.** `crates/venus-sidecar/src/queue.rs` (`CLAIM_SQL`, `FLUSH_SQL`, `delete_job`).

**Proposed fixes.**
1. Renew the lease periodically from the worker (`UPDATE jobs SET lease_until = now() + 2 min WHERE workspace_id = $1 AND owner = $2`) and abort the Flush if the renewal affects 0 rows.
2. `DELETE FROM jobs WHERE workspace_id = $1 AND owner = $2 AND requested_at <= $cut_time`. A `POST /flush` during a run bumps `requested_at`, so the row survives and runs again.
3. Hold a per-wiki filesystem lock (`flock` on `wiki/.venus/lock`) for the whole commit as a second safeguard.

<a id="m8"></a>

### M8. The hub keeps every opened `?doc=` in memory for the whole life of the room

[↑ Summary](#summary-m8)

**Problem.** `Room::ensure_doc` / `extra_or_empty` insert an `ExtraSpace` (a hydrated `Doc` plus a persistence buffer) for every `doc_id` any client connects to. They are removed only when the whole room is idle (`is_idle`). Any well-formed uuid is accepted, including ones that are not in the catalog. One client can loop over random `?doc=` uuids and each one stays in memory. In normal use, every page a user ever opened during the room's lifetime also stays resident.

**Where.** `crates/venus-hub/src/room.rs` (`attach_doc`, `extra_or_empty`, `ensure_doc`). `crates/venus-hub/src/http.rs` (`bind_doc_id`).

**Proposed fixes.**
1. Evict each document separately: track client count and last activity per `doc_id`, and evict when idle and the persistence buffer is empty.
2. Cap the number of documents per room and the number of connections per client IP.
3. Optional: refuse `?doc=` uuids that are not the catalog id, home, or a `kind:doc` node in the room's catalog. This needs the hub to read the catalog.

---

## Low

<a id="l1"></a>

### L1. Rebuilding `links.json` does filesystem I/O inside the database transaction, and its errors are swallowed

[↑ Summary](#summary-l1)

When `.venus/links.json` is missing, `pin_convert_set_extras` calls `links::rebuild_from_wiki`, which reads every `.md` file under `wiki/` while the repeatable-read cut transaction is still open. `.unwrap_or_default()` turns any I/O error into an empty index, so the convert set silently misses inbound rewrites. Also, an error loading the extras (`warn!("cut skipped convert-set extras")`) skips them, and the Flush commits stale hrefs without any signal. **Where:** `crates/venus-sidecar/src/cut.rs` (`pin_convert_set_extras`, ~180–215). **Fix:** read `links.json` / `pages.yaml` (or rebuild the index) **before** `BEGIN`, since they only depend on HEAD. Return the error instead of defaulting, or mark the Flush `degraded` in the job status (H5).

<a id="l2"></a>

### L2. The catalog is decoded and walked several times in one Flush

[↑ Summary](#summary-l2)

`cut` decodes the catalog to build the convert set, `flush_claimed` runs `walk_pin` again, and `convert_pins_catalog` builds `catalog_ctx` from the walk for each page. This is cheap now but grows with the size of the catalog multiplied by the number of converted pages. **Fix:** walk once in `cut` and store the `CatalogWalk` on `PinMap`. Build `catalog_ctx.pages` once and share it (`Arc`) across pages.

<a id="l3"></a>

### L3. The debug bar triggers a full git history walk every 2 seconds

[↑ Summary](#summary-l3)

`VenusDebugBar` polls `GET /git/log?path=spec/home.md` every 2 s. `git::log_path` walks the whole revision history and compares trees for each commit. **Fix:** add a `limit` parameter (default 50), cache by HEAD sha (return early when HEAD is unchanged), or switch to a server-sent event on commit.

<a id="l4"></a>

### L4. Commit messages are `snapshot: Venus` or an empty `snapshot:`

[↑ Summary](#summary-l4)

`autocomment` uses the page H1 only when **exactly one** page was converted. Convert-set extras (inbound rewrites) make that rare. Created pages have no H1, so they fall back to `Venus`. The live wiki log shows `snapshot: Venus` (×7) and an empty `snapshot:`. **Where:** `git.rs` `autocomment`, `snapshot_title`. **Fix:** build the subject from the **catalog name** of the dirty pages (not the extras), for example `snapshot: protocol (+2 link rewrites)`, and fall back to the page path.

<a id="l5"></a>

### L5. Test hooks on `window.__VENUS_*` are shipped in every build

[↑ Summary](#summary-l5)

`App.tsx` installs `__VENUS_CATALOG_OPS__` (attachCatalogTestHooks), `__VENUS_OPEN_DOC__`, and `__VENUS_INSERT_LINKED_DOC__` with no condition. Any script on the page (an extension, or an XSS in rendered markdown) can mass-create, delete, or move catalog nodes and insert blocks through them. That is not much more than it could already do through the DOM, but it makes scripted damage trivial and it adds to the product surface. **Fix:** register them only when `testidsFromEnv()` or a dedicated `VITE_E2E_HOOKS` is set (Playwright already sets `VITE_TESTIDS`).

<a id="l6"></a>

### L6. Every catalog event rebuilds the whole tree, and the header renames on every keystroke

[↑ Summary](#summary-l6)

`listenCatalogHost` runs one `observeDeep` and does a **full** `rebuildTree` plus `applyCatalogHostChrome` for each event. Typing in the header title calls `onTitleChange` → `ops.rename` **for every keystroke**, which means one catalog transaction (a `name` and `gitName` write), one hub frame, one full rebuild in every tab, and a `uniqueGitName` probe each time. Each intermediate name also becomes CRDT history. **Fix:** debounce the header rename (commit on blur, Enter, or after 300 ms), batch rebuilds with `requestAnimationFrame`, and update only the changed subtree when the event's `keysChanged` touches a single node.

<a id="l7"></a>

### L7. Untrusted names and `pageId`s are written into markdown without full escaping

[↑ Summary](#summary-l7)

The Rust export escapes a few characters in link text but not newlines. A catalog `name` containing `\n# Injected` breaks out of the link into a new block in `home.md`. In the Rust `./workspace/` fallback, `page_id` (from the card's CRDT `pageId`) is inserted without validation. JS checks it with `isSafePageId`. **Where:** `from_doc/markdown.rs` (`linked_doc_inline`, `escape_text`). **Fix:** strip or replace control characters in names (C1 check), and validate `page_id` with the uuid check in Rust as JS does.

<a id="l8"></a>

### L8. `POST /flush` can be triggered cross-site and accepts any workspace

[↑ Summary](#summary-l8)

An empty `POST` is a CORS "simple request", so any website the developer visits can trigger Flushes. H1 now returns 404 for a workspace this wiki is not bound to; a request that omits `?workspace=` still flushes the bound wiki and returns 204. **Fix:** require a custom header (`X-Venus-Flush: 1`, which forces a preflight), and return `202` with the job state.

<a id="l9"></a>

### L9. `.venus/ids/doc:home.json` contains a colon

[↑ Summary](#summary-l9)

The sidecar file for home is named `doc:home.json`. Windows (and some sync tools) do not allow `:` in filenames, so `git clone` of the wiki on Windows fails to check it out. **Where:** `git.rs` `sidecar_rel`. **Fix:** key sidecar files by the SQL uuid (`395cd07b-….json`), or encode `:` as `%3A`. Migrate with a `git mv` during a Flush.

<a id="l10"></a>

### L10. Test and documentation gaps

[↑ Summary](#summary-l10)

- `e2e/m4-link.spec.ts` does not assert that the link target file exists in git (it would have caught H2), the card title after the reload (H3), or the card style.
- `pnpm tsc` still reports `drop.test.ts(102)` "o1 / o2 possibly undefined". This came from an earlier step.
- [README.md](./README.md) links to `step-3-findings.md`, which does not exist and is not in git history.
- `links::persist_on_flush` upserts outbound edges for converted pages that are not in the walk (created and deleted between two Flushes, so never in `old_pages`). Those edges are never removed, and `links.json` keeps edges to a source that does not exist. They are harmless today because inbound sources are filtered through `doc_to_sql`, but they grow without limit. **Fix:** drop `converted` ids that are not in `walk` before `persist_on_flush`.
- Deleted pages keep their CRDT rows in SQL forever and can still be opened by uuid (`__VENUS_OPEN_DOC__`, hub `?doc=`). **Fix:** add a tombstone flag in `page_identity` and have the hub refuse it, plus a GC job, after M5 decides how long deleted pages are kept.

---

## Suggested order

1. **C1** is deferred to the gateway (separate component). **H1** (workspace binding) is fixed.
2. **H5** (job attempts, status, backoff), so the remaining failures can be seen instead of looping.
3. **H4** (sidecar-owned uniqueness plus two-phase moves) and **M2** (inbound of deleted targets).
4. **H2** and **H3** together: seed on create, and project the catalog into the workspace docs and `docMetas`. Extend `m4-link` to check the target file, the title, and the style after a reload.
5. **H6** reconnect before step 8. The SharedWorker in step 8 will reuse this provider, so it should be fixed there first.
6. The remaining Medium and Low items as cleanup before M4 exits.
