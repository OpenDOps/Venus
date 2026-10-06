# M4 — Implementation review findings (steps 1–7)

This review covers the M4 work that is already marked `done` in [plan.md](./plan.md): the catalog schema and operations, the per-document hub sockets, the tree, the header, `git mv` on publish, and linked-doc export with `.venus/links.json`. The SharedWorker step and the recon step are out of scope because they are not implemented yet.

**Review date:** 2026-10-06. It was run against the uncommitted working tree on branch `M4`.
**Method:** I read the code paths end to end (host → hub → SQL → sidecar cut → convert → git) and checked them against the live `wiki/` repo, which has 36 Flush commits.
**Scope:** logic gaps, leftover code and documentation, logic mistakes, performance, and security. The review itself changed no product code. Follow-up on this file: **C1** and **L8** are deferred to the gateway, **L4** is accepted (commit text is written by an AI that analyzes the changes, not by the sidecar), **L9** is accepted (this wiki never runs on Windows), and **H1**, **H2**, **H3**, **H4**, **H5**, **H6**, **M1**, **M2**, **M3**, **M4**, **M6**, **M7**, **M8**, **L1**, **L2**, **L3**, **L5**, **L6**, and **L7** are fixed.

| Severity | Meaning |
|---|---|
| **Critical** | An untrusted hub client can write or delete files outside the wiki, or get code to run. |
| **High** | Silent data loss, a Flush that is permanently stuck, wrong content in git, or broken product behaviour on a normal user path. |
| **Medium** | Wrong output in edge cases, or a concurrency gap that needs two tabs or unusual input. |
| **Low** | Performance, hygiene, leftover code or docs, or test gaps with little user impact. |

## Summary

| # | Sev | Kind | Finding |
|---|---|---|---|
| [C1](#c1) | Critical | Security | **Deferred (gateway).** The sidecar trusts catalog `gitName`, which allows path traversal into the filesystem and into `wiki/.git` |
| [H1](#h1) | High | Security / logic | **Fixed.** All workspaces flush into the same `wiki/` directory |
| [H2](#h2) | High | Logic | **Fixed.** A page that is created but never opened is dead everywhere |
| [H3](#h3) | High | Logic | **Fixed.** After a reload, linked-doc cards render as deleted and write that state back to the CRDT |
| [H4](#h4) | High | Logic | **Fixed.** Path collisions inside one Flush cause a permanent `git mv would overwrite` error or overwrite a file |
| [H5](#h5) | High | Logic / ops | **Fixed.** A Flush that fails the same way every time is retried every 2 minutes forever, and nobody sees it |
| [H6](#h6) | High | Logic | **Fixed.** `VenusHubProvider` does not reconnect, so edits after a hub restart are silently lost |
| [M1](#m1) | Medium | Logic | **Fixed.** Concurrent delete or reparent leaves orphan or cyclic nodes that are hidden in the tree but published to git |
| [M2](#m2) | Medium | Logic | **Fixed.** Deleting a link target leaves a dangling href in git, later followed by a `./workspace/` URL |
| [M3](#m3) | Medium | Logic | **Fixed.** Exported hrefs are not URL-encoded, and JS and Rust escape link text differently |
| [M4](#m4) | Medium | Logic | **Fixed.** The `pages.yaml` writer can emit invalid YAML, and the reader that parses it is line-based |
| [M5](#m5) | Medium | Logic | **Decided (b).** Home and `spec` can be renamed, but the sidecar hard-codes `spec/home.md` |
| [M6](#m6) | Medium | Logic | **Fixed.** Flush edits the working tree before committing, and its database updates are not atomic |
| [M7](#m7) | Medium | Concurrency | **Fixed.** The job lease is never renewed, and `delete_job` does not check the owner |
| [M8](#m8) | Medium | Perf / DoS | **Fixed.** The hub keeps every opened `?doc=` in memory for the whole life of the room |
| [L1](#l1) | Low | Perf / logic | **Fixed.** A failed link index becomes an empty convert set, and the fallback still reads the working tree |
| [L2](#l2) | Low | Perf | **Fixed.** The catalog is decoded and walked several times in one Flush |
| [L3](#l3) | Low | Perf | **Fixed.** The debug bar triggers a full git history walk every 2 seconds |
| [L4](#l4) | Low | Logic | **Accepted.** Commit messages are `snapshot: Venus` or an empty `snapshot:` for most catalog Flushes |
| [L5](#l5) | Low | Security | **Fixed.** Test hooks on `window.__VENUS_*` are shipped in every build |
| [L6](#l6) | Low | Perf | **Fixed.** Every catalog event rebuilds the whole tree, and the header renames on every keystroke |
| [L7](#l7) | Low | Security | **Fixed.** Untrusted names and `pageId`s are written into markdown without full escaping |
| [L8](#l8) | Low | Security | **Deferred (gateway).** `POST /flush` can be triggered cross-site |
| [L9](#l9) | Low | Residual | **Accepted.** `.venus/ids/doc:home.json` contains a colon, which Windows does not allow in filenames |
| [L10](#l10) | Low | Residual | Test and documentation gaps (assertions in `m4-link`, `tsc` errors, a dangling README link, stale `links.json` edges) |

---

## Critical

### C1

**Catalog `gitName` is trusted by the sidecar: path traversal into the filesystem and into `wiki/.git`**

[↑ Summary](#summary)

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

### H1

**Every workspace flushes into the same `wiki/` directory**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). One sidecar process publishes one workspace. `WIKI_WORKSPACE_ID` (unset → the M0 uuid `77e4a2b1-8b40-5979-a73c-fd4477216d00`) is the only id the observer enqueues and the only id a worker claims. `POST /flush` returns 400 for a bad shape and 404 for any other uuid. The first Flush commits `.venus/workspace` and checkout writes it; a later Flush for a different id refuses before it writes pages. (Until 2026-10-06 the stamp was written to the working tree before the commit, so checkout left it out of the index and `git status` showed it deleted and untracked. Checkout now also resets the index to HEAD, which heals such a wiki on its next Flush.) The hub allowlist is `HUB_WORKSPACES` (same default): HTTP and gRPC reject other ids with 404 / NotFound before a room, a lease, or a blob row. `Hub::new` and `WikiConfig::new` stay unbound so tests can still use a fresh uuid per case. Compose sets both env vars on `hub` and `sidecar`.

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

### H2

**A page that is created but never opened is dead everywhere**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). The creating tab connects the new page, drops the handshake's late sync step 2, seeds, and waits until a following step 2 contains that page. Only then does it insert the catalog node and disconnect. A failed seed leaves no tree row. `createDoc` in `ops.js` still seeds only in memory for unit tests. The tree and the Playwright hook call `createPublishedDoc`. Pages created before the fix stay dead: on 2026-10-06 `pnpm wiki:verify` reports the four listed below as `H2 legacy page` (no `.md`, home links three). Delete them in the tree and Flush ([runbook M4 Baseline](../../runbook.md#manual-testing-m4-close-out)). Proposed fix 3 (the sidecar leaves a body-less page out of `pages.yaml` and hrefs) is not done.

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

### H3

**After a reload, linked-doc cards render as deleted and write that state back to the CRDT**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `listenCatalogHost` projects every catalog `kind:doc` into the local workspace: `createDoc` with no seed and no `load`, then `setDocMeta` title = catalog name. A doc that leaves the catalog is removed, except the page that is open. The card's `getStore({ id })` on an empty stub reports a stand-in root and a `load` that does not write blocks, so the card is not deleted and does not push a blank page onto the hub. `getStore()` with no arguments stays the real store.

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

### H4

**Path collisions inside one Flush: permanent `git mv would overwrite` or a silent overwrite**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `walk_doc` keeps published paths unique by case-folded path: the lowest `doc_id` keeps the name, and the others are published as `name-<id8>.md` without editing the CRDT. Flush deletes removed pages first (leaving a markdown file that a live page still has to move), parks every rename under `.venus/tmp/<uuid>`, then moves each file to its final path and deletes the temp directory. The host compares sibling names case-insensitively, and `listenCatalogHost` repairs a merged duplicate `gitName` on a microtask with the same suffix.

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

### H5

**A Flush that fails the same way every time is retried every 2 minutes forever, and nobody sees it**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). A failed attempt stores `last_error`, increments `attempts`, and clears the lease. Database errors and I/O timeouts back off (2s, doubling, capped at 2 minutes) and stop after 5 attempts. Validation and collision errors (`no home doc`, `git mv would overwrite`, and the same class) mark the job `failed` on the first attempt. `GET /flush/status` returns the last commit, the error, and the attempt count. `POST /flush` returns `409` with that error while the job is failed. The debug bar shows the error. The observer reopens a failed job only when a dirty clock is newer than the one recorded at the failure. Per-page quarantine of a bad path stays with the gateway (C1).

**Problem.** `flush_claimed` deletes the job only when it succeeds. Any error that repeats on every attempt leaves the job in place, and it is claimed again when the 2-minute lease runs out. Examples: H4's `git mv would overwrite`, `catalog pin has no home doc` (a client deleted the home node), `ENAMETOOLONG` (C1), a broken YAML parse, or a disk-full error. The browser's Flush button gets `204` from `POST /flush` regardless, and the debug bar's git log simply stops changing. No new commit will ever be made for that workspace.

**Data flow.** Job inserted → `claim_one` (lease 2 min) → `cut_workspace` ✓ → `commit_pins` ✗ → `Err` propagates → worker logs `warn` → job stays with `lease_until` in the future → after 2 min it is claimed again → the same error.

**Where.** `crates/venus-sidecar/src/queue.rs` (`flush_claimed`, `CLAIM_SQL`, no attempt count). `crates/venus-sidecar/src/http.rs` `flush` (always `204`). `apps/web/src/host/chrome/VenusDebugBar.tsx` (no error state).

**Proposed fixes.**
1. Add `attempts int`, `last_error text`, and `last_error_at timestamptz` to `jobs`. On error, `UPDATE jobs SET attempts = attempts + 1, last_error = $e, not_before = now() + backoff(attempts), lease_until = NULL`. After N attempts, move the job to `failed` (or `dead_jobs`) and stop retrying until the CRDT changes again (the observer clears `failed` when a new dirty clock appears).
2. Add `GET /flush/status?workspace=` (last success sha, last error, attempts). The debug bar shows an error badge. `POST /flush` should return `409` with `last_error` when the job is in `failed`.
3. Split errors into two groups: **transient** (database or I/O timeouts, which keep retrying) and **deterministic** (validation or collision errors). Deterministic errors fail fast, and where possible they are skipped per page instead of per workspace, as in C1 fix 1 and H4 fix 1.

### H6

**`VenusHubProvider` does not reconnect: edits after a hub restart are silently lost**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). After the first sync, a dropped socket sets that session and the provider unsynced and opens the same URL again with jittered backoff. The reopen sends sync step 1, then any updates typed while the socket was down. The header shows `reconnecting` and `beforeunload` warns until the hub's next step 2. Edits are not written to IndexedDB, so a reload before the socket is back can still drop them.

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

### M1

**Concurrent delete or reparent leaves orphan or cyclic nodes that are hidden in the tree but published to git**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `listenCatalogHost` moves an orphan to the root, and breaks each parent cycle by moving the greatest id to the root. Every tab computes the same parents, so the repairs converge. Until that write lands, the tree lists orphans under **Unfiled** instead of omitting them. The sidecar publishes the same parents and logs each repair. It does not edit the catalog CRDT.

**Problem.** `deleteNode` only refuses a non-empty folder based on the **local** `hasChild`. `reparent` only refuses a cycle based on the **local** `assertNotCycle`. Neither check holds across tabs once the CRDT merges.

**Data flows.**

- Orphan: tab A deletes the empty folder F while tab B creates a doc inside F. After the merge, the doc's `parentId` points to a node that no longer exists. Host `tree-prune` hides it, so it is **invisible in the tree**. Rust `git_path_of` → missing parent → `""` → `join_git_names("", name)` publishes it at **the wiki root** (`notes.md`).
- Cycle: tab A moves X under Y while tab B moves Y under X. After the merge, X ↔ Y. JS `gitPathOf` and Rust `git_path_of` both cut the cycle by returning only the leaf name, so the published path is a different, unrelated location. The tree hides both nodes.

**Where.** `apps/web/src/host/catalog/ops.js` (`deleteNode` line ~375, `reparent` line ~320, `assertNotCycle`). `apps/web/src/host/catalog/tree-prune.js`. `crates/venus-sidecar/src/catalog.rs` (`git_path_of`).

**Proposed fixes.**
1. Add a deterministic **repair pass** in `listenCatalogHost` (only one host repairs: the tab with the lowest client id, or each tab idempotently). Move orphans to the root (or a `lost+found` folder). Break each cycle at the node with the greatest id by moving it to the root. Because the result does not depend on which tab runs it, concurrent repairs converge.
2. In the sidecar, apply the same rule (orphan → root, cycle broken at the greatest id) so git agrees with the repaired tree, and log each repair.
3. Show orphans in the tree under "Unfiled" instead of pruning them.

### M2

**Deleting a link target leaves a dangling href in git, later followed by a `./workspace/` URL**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `convert_set` also converts inbound sources of every page in `old_pages` that the walk dropped, so the delete Flush rewrites those files. A missing target exports as `~~name~~` plus `<!-- venus:doc:<id> missing -->` (the page id is the label when the last name is unknown). JS and Rust emit the same bytes. A catalog-less export still uses the M2 `./workspace/` golden.

**Problem.** The convert set is: dirty bodies ∪ inbound sources of pages whose path or name changed ∪ pages whose own directory changed and that have outbound links. It does **not** include inbound sources of a **deleted** target.

**Data flow.** Home links to P. P is deleted, and Flush runs `git_rm` on `P.md`, but `home.md` keeps `[P](P.md)`, which is now dangling. Later, home is edited and converted again. `catalog_ctx` no longer has P, so `linked_doc_inline` falls back to `[untitled](./workspace/<ws>/<P>)`. That form is explicitly forbidden in git ([subset](../MDGate/subset.md#linked-doc-stable-form)). The card itself still exists in home's CRDT.

**Where.** `crates/venus-sidecar/src/links.rs` (`convert_set`). `crates/venus-sidecar/src/from_doc/markdown.rs` (`linked_doc_inline` fallback). `apps/web/src/host/mdgate/from-doc.js` (`catalogLinkedDocLink` returns `null`, so the adapter default is used).

**Proposed fixes.**
1. In `convert_set`, add `inbound[d]` for every `d` in `old_pages − walk`.
2. Define a stable export for a missing target, for example the plain text `~~name~~ <!-- venus:doc:<id> missing -->`, or `[name](<last known path>)` with a `missing` marker. Never write `./workspace/`. Use the same form in JS and Rust and add a parity test for it.

### M3

**Exported hrefs are not URL-encoded, and JS and Rust escape link text differently**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). Each href segment is percent-encoded (` ` → `%20`, `(` `)` → `%28` `%29`, other bytes outside the unreserved set too, including non-ASCII). `..` stays literal. Link text in both languages escapes `\ ` * _ [ ] < >`. The shared fixture is `apps/web/src/host/mdgate/goldens/catalog-link-escape.md`. Comment placement reads a percent-encoded destination, so `%29` is not treated as the end of the URL.

**Problem.** Filenames can contain spaces and parentheses (`spec/Renamed venus page.md` is in the live wiki). `posix_relative` / `posixRelativeFromFiles` output is inserted into `[text](href)` as-is. CommonMark does not allow spaces in a bare link destination, and an unbalanced `)` ends it early. As a result, a link to `Renamed venus page.md` renders as literal text on any markdown viewer, and M6 apply (remark) will not parse it as a link. Separately, Rust `escape_text` escapes `` \ ` * _ [ ] < > `` while JS `escapeLinkText` only escapes `\ [ ]`. For a name like `my_page`, the pane (JS) and git (Rust) produce different text.

**Where.** `crates/venus-sidecar/src/links.rs` `posix_relative`. `crates/venus-sidecar/src/from_doc/markdown.rs` (`linked_doc_inline`, `escape_text`). `apps/web/src/host/mdgate/from-doc.js` (`posixRelativeFromFiles`, `escapeLinkText`, `findLinkedDocInsert`, which also stops at the first `)`).

**Proposed fixes.**
1. Percent-encode each path segment (space → `%20`, `(` `)` → `%28` `%29`, plus non-ASCII if needed), or use the angle-bracket form `[text](<path with spaces.md>)`. Do this in both JS and Rust, and make `findLinkedDocInsert` understand the chosen form.
2. Use one escaping table for link text in both languages, and extend the JS↔Rust parity fixture with names containing `_ * ( ) [ ]` and spaces.

### M4

**The `pages.yaml` writer can emit invalid YAML, and the reader that parses it is line-based**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `pages.yaml` is written and read with `serde_yaml`. The Flush cut builds its path-to-doc map from `page_identity` and does not read the wiki while the database transaction is open. A catalog `name` loses control characters before it is published. The full C1 path check stays with the gateway.

**Problem.** `needs_yaml_quotes` misses newlines, a leading `-` / `?` / `!`, YAML 1.1 booleans and nulls (`yes`, `no`, `on`, `off`, `true`, `null`, `~`), and values that look numeric (`1e3`, `0x10`). `yaml_quoted` does not escape `\n` either. A catalog `name` such as `"a\nb"` breaks the document. A name `true` is read back as a boolean. `links::path_to_doc_id_from_pages_yaml` parses this file **line by line**, so a quoted key or a multi-line value gives the wrong `path → doc` map, and the convert set then misses inbound rewrites.

**Where.** `crates/venus-sidecar/src/catalog.rs` (`pages_yaml`, `needs_yaml_quotes`, `yaml_quoted`). `crates/venus-sidecar/src/links.rs` (`path_to_doc_id_from_pages_yaml`).

**Proposed fixes.**
1. Write the file with `serde_yaml` (or always use double quotes with full JSON-style escaping, since JSON strings are valid YAML).
2. Read it with the same library. Even better, build `path_to_doc` from `page_identity` (SQL, already loaded as `old_pages`) and stop reading the working tree inside the cut.
3. Reject control characters in `name` on the sidecar side (same check as C1).

### M5

**Home and `spec` can be renamed, but the sidecar hard-codes `spec/home.md`**

[↑ Summary](#summary)

**Decision:** (b), matching [page-identity — Home](../datamodel/page-identity.md#home). Home’s identity stays `doc:home`. Delete and reparent stay forbidden, so `parentId` stays `folder:spec`. Rename still changes the display `name` and the filename, and renaming the `spec` folder may change home’s derived path. The published path is whatever the catalog walk produces. `spec/home.md` is only the seed path. `git/log?doc=doc:home` must resolve the current path through `page_identity` and follow history across renames. `GIT_PATH` comes out of `cut` and `ensure_repo`. This is not implemented yet.

**Problem.** `reparent` and `deleteNode` refuse to touch home (`home_protected`), but `rename` does not. The tree and the header title input both allow renaming home or the `spec` folder. The sidecar still assumes `GIT_PATH = spec/home.md` in several places: `cut` sets `pins.git_path`, `commit_home_only` uses it, `ensure_repo` always creates `spec/`, `is_catalog_log_path` only accepts `spec/home.md`, and the debug bar polls `CATALOG_GIT_LOG_PATH`.

**Data flow.** The user renames home to "Start" → Flush → `git_mv(spec/home.md → spec/Start.md)` → the debug bar's git log (`path=spec/home.md`) stops at the rename. The M3 e2e tests and the `git log` endpoint lose track of home, and `ensure_repo` recreates an empty `spec/`.

**Where.** `apps/web/src/host/catalog/ops.js` (`rename`). `apps/web/src/host/catalog/CatalogTree.tsx` (rename allowed on home). `apps/web/src/App.tsx` header `onTitleChange`. `crates/venus-sidecar/src/git.rs` (`GIT_PATH`, `ensure_repo`, `is_catalog_log_path`, `log_path`). `crates/venus-sidecar/src/cut.rs` (`pins.git_path`).

**Proposed fixes.**
1. Decide on the product rule. Either (a) home's `gitName` is fixed (rename only changes `name`, and the sidecar always publishes home to `spec/home.md`), or (b) home can move and the sidecar resolves its path from the walk.
2. For (b): `git/log?doc=doc:home` resolves the path through `page_identity` and uses `--follow` semantics (walk the history across renames). Remove `GIT_PATH` from `cut` and `ensure_repo`.

### M6

**Flush edits the working tree before committing, and its database updates are not atomic**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). A Flush builds the next tree in memory from HEAD (`Index::read_tree`), commits that tree, and then `checkout_head(force)`, including when the tree already matches HEAD so a crash between the commit and checkout is healed on the next Flush. Stray files and dirty working-tree edits are not staged. `page_identity` and `last_flushed` commit in one SQL transaction. If HEAD `.venus/pages.yaml` disagrees with `page_identity`, the table is rebuilt from that blob before `old_pages` is loaded. A file that does not parse leaves the table as it is.

**Problem.** `commit_catalog_walk` runs `git_mv`, `write_page`, `git_rm_page`, `pages.yaml`, `links.json`, and blob writes directly in `wiki/`, then `add_all` + `update_all` + commit. If any step fails part-way, the half-applied tree stays on disk. The next successful Flush commits it with `add_all`, together with any stray file in `wiki/`. After the commit, `replace_page_identity` and `upsert_last_flushed` run as separate statements. If the process crashes between the git commit and those writes, `old_pages` no longer matches git. A later rename back to the old path then leaves the moved file behind (`git_mv` skips because `src` does not exist), and it stays tracked forever.

**Where.** `crates/venus-sidecar/src/git.rs` (`commit_catalog_walk`, `commit_tree` `add_all`). `crates/venus-sidecar/src/queue.rs` (`flush_claimed` after `commit_pins`).

**Proposed fixes.**
1. Build the commit from the git **index or tree** (`git2::TreeBuilder` based on HEAD) instead of mutating the working tree, then `checkout_head(force)` once the commit succeeds. Alternatively, stage only the paths this Flush touched (no `add_all ["."]`) and run `git reset --hard HEAD` + `clean` at the start of every Flush.
2. Write `page_identity` and `last_flushed` in **one** SQL transaction. At the start of a Flush, if HEAD's `.venus/pages.yaml` and `page_identity` disagree, rebuild `page_identity` from HEAD (git is the record of what was published).

### M7

**The job lease is never renewed, and `delete_job` does not check the owner**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). The lease is not extended. A Flush has the same two-minute budget as `lease_until`. When that budget is spent, the Flush returns `flush timed out after 2 minutes` and the job is `failed` until a newer dirty clock reopens it. The in-flight database step is dropped, and the git commit is not started. A second worker waits until the current commit has returned, and does not start its own if its budget is already spent. The failure update clears `owner` and `lease_until` only while this worker still owns the row. A successful finish deletes the job only for that same owner.

**Problem.** `CLAIM_SQL` sets `lease_until = now() + 2 min` and nothing extends it. A Flush that takes longer than 2 minutes (a large wiki, `rebuild_from_wiki`, a slow disk) is claimed by a second worker (`SNAPSHOT_WORKERS=2`) **while the first is still writing to the same `wiki/`**, and the two corrupt the working tree and index. `delete_job` deletes by `workspace_id` only, so the slower worker deletes the newer job. A `POST /flush` that arrives during a running Flush updates `not_before` on the same row, and that row is then deleted on success, so the request is lost until the observer's idle timer runs out.

**Where.** `crates/venus-sidecar/src/queue.rs` (`CLAIM_SQL`, `FLUSH_SQL`, `delete_job`).

**Proposed fixes.**
1. Renew the lease periodically from the worker (`UPDATE jobs SET lease_until = now() + 2 min WHERE workspace_id = $1 AND owner = $2`) and abort the Flush if the renewal affects 0 rows.
2. `DELETE FROM jobs WHERE workspace_id = $1 AND owner = $2 AND requested_at <= $cut_time`. A `POST /flush` during a run bumps `requested_at`, so the row survives and runs again.
3. Hold a per-wiki filesystem lock (`flock` on `wiki/.venus/lock`) for the whole commit as a second safeguard.

### M8

**The hub keeps every opened `?doc=` in memory for the whole life of the room**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). A non-home document is dropped when no client is attached, its persist buffer is empty, nothing else still holds it, and it has been idle for the room's lease TTL. The heartbeat does this while the room itself stays up. Home is never dropped that way. A room keeps at most 64 non-home documents and 32 sockets from one address. At the document cap, idle empty documents are dropped even if the TTL has not elapsed; a document with a client, unflushed updates, or a hydration still waiting for its socket (15 seconds) is kept, and the next open is refused (`room_full`, HTTP 429). The optional catalog-membership check was not implemented: a well-formed uuid is still accepted, bounded by those caps.

**Problem.** `Room::ensure_doc` / `extra_or_empty` insert an `ExtraSpace` (a hydrated `Doc` plus a persistence buffer) for every `doc_id` any client connects to. They are removed only when the whole room is idle (`is_idle`). Any well-formed uuid is accepted, including ones that are not in the catalog. One client can loop over random `?doc=` uuids and each one stays in memory. In normal use, every page a user ever opened during the room's lifetime also stays resident.

**Where.** `crates/venus-hub/src/room.rs` (`attach_doc`, `extra_or_empty`, `ensure_doc`). `crates/venus-hub/src/http.rs` (`bind_doc_id`).

**Proposed fixes.**
1. Evict each document separately: track client count and last activity per `doc_id`, and evict when idle and the persistence buffer is empty.
2. Cap the number of documents per room and the number of connections per client IP.
3. Optional: refuse `?doc=` uuids that are not the catalog id, home, or a `kind:doc` node in the room's catalog. This needs the hub to read the catalog.

---

## Low

### L1

**A failed link index becomes an empty convert set, and the fallback still reads the working tree**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `load_old_pages` and a git read error fail the cut. A missing or unparsed `.venus/links.json` blob is rebuilt from HEAD markdown. An empty index is returned only when that blob is absent and there are no pages to index. The catalog walk error and the `page_identity` error fail the cut instead of skipping the convert set.

**Problem.** The cut loads the link index before `BEGIN`, which is the right place. What it does with a failure is still wrong. `load_old_pages(...).unwrap_or_default()` turns a database error into no previous pages. If `.venus/links.json` is missing, `index_from_committed` then rebuilds by reading every markdown file in the working tree, and a read error is logged and replaced with an empty index. `pin_convert_set_extras` also returns success when the catalog pin does not walk or when `page_identity` fails to load. The Flush continues, the convert set misses inbound pages, and git keeps stale hrefs. Nothing is recorded on the job.

**Where.** `crates/venus-sidecar/src/cut.rs` (`cut_workspace`, `pin_convert_set_extras`). `crates/venus-sidecar/src/links.rs` (`index_from_committed`, `rebuild_mapped`).

**Proposed fixes.**
1. A `load_old_pages` error fails the cut. An empty `page_identity` is a real empty history. A database error is not.
2. When `links.json` is missing or does not parse, rebuild the index from the HEAD tree blobs (the same source `compose_flush_bytes` already uses), not from the working tree. A rebuild I/O error fails the Flush. The only successful empty index is a repository with no committed pages and no `links.json`.
3. `pin_convert_set_extras` returns the catalog walk error and the `page_identity` error. It does not skip the extras and continue. H5 then stores `last_error` and backs off. Do not add a separate `degraded` state.

### L2

**The catalog is decoded and walked several times in one Flush**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). The cut stores its `CatalogWalk` on the pin map. Flush uses that walk, and calls `walk_pin` only when a catalog pin is present and the cut did not store one. Convert builds the docId → name/path map once and shares it with every page.

**Problem.** One Flush decodes the catalog pin to decide the convert set, then `flush_claimed_inner` calls `walk_pin` on the same bytes again. Each converted page then builds its own `catalog_ctx`, which clones every page's name and path. The cost is the catalog size times the number of converted pages. It is small on this wiki and grows with it.

**Where.** `crates/venus-sidecar/src/cut.rs` (`pin_convert_set_extras`). `crates/venus-sidecar/src/queue.rs` (`flush_claimed_inner`, `convert_pins_stopping`). `crates/venus-sidecar/src/from_doc/mod.rs` (`catalog_ctx`).

**Proposed fixes.**
1. Keep the `CatalogWalk` from the cut on `PinMap`. `flush_claimed_inner` uses that walk. It calls `walk_pin` only when the cut did not have a catalog pin.
2. Build `catalog_ctx.pages` once per Flush and pass `Arc` into each page. `from_pinned_bytes_catalog_missing` stops cloning the map per pin.
3. Do not walk again inside `convert_pins_stopping`. The walk is an argument, as it is today, but it is the same value the cut stored.

### L3

**The debug bar triggers a full git history walk every 2 seconds**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). `GET /git/log` stops after 50 commits that touch the path (or `?limit=`). The sidecar caches that list by HEAD and limit, and an unchanged HEAD does not walk. The debug bar still polls `/flush/status` every 2 seconds. It loads the git log once on mount, then again only when that status reports a new sha.

**Problem.** `VenusDebugBar` polls `GET /git/log?path=spec/home.md` every 2 seconds, and polls `/flush/status` on the same timer. `git::log_path` revwalks from HEAD and diffs every commit against its parent. The debug bar only needs the recent subjects, and it already ignores a response whose shas did not change.

**Where.** `apps/web/src/host/chrome/VenusDebugBar.tsx`. `crates/venus-sidecar/src/git.rs` (`log_path`). `crates/venus-sidecar/src/http.rs` (`git_log`).

**Proposed fixes.**
1. Add `limit` to `GET /git/log` (default 50). Stop the revwalk once that many commits touch the path. Keep the path check that allows only `spec/home.md` until M5's rename work makes the path come from `page_identity`.
2. Cache the last result in the sidecar process, keyed by HEAD oid and limit. An unchanged HEAD returns the cached list and does not walk.
3. The debug bar keeps the 2-second `/flush/status` poll. It refetches `/git/log` only when that status reports a new sha, plus once on mount. No server-sent event.

### L4

**Commit messages are `snapshot: Venus` or an empty `snapshot:`**

[↑ Summary](#summary)

**Status:** `accepted` (2026-10-06). Autocomment is not changed in this slice. Commit text is written by an AI that analyzes the changes, not by the sidecar from the catalog name. The subject rules below are not implemented.

**Problem.** `autocomment` uses the page H1 only when exactly one pin was converted. Inbound rewrites make that rare, so most commits get `snapshot:` with an empty title. A newly created page has no H1, so the one-page case falls back to the seed word `Venus`. The catalog name the user typed is not consulted.

**Where.** `crates/venus-sidecar/src/git.rs` (`autocomment`, `snapshot_title`). The converted list and the catalog walk are both in hand at `commit_pins`.

**Proposed fixes.**
1. Pass the catalog walk and the pre-extra dirty doc ids into `autocomment`. The subject is built from those dirty pages only. Convert-set extras are a count, not titles.
2. One dirty page: `snapshot: {catalog name}`. Several: `snapshot: {first name} (+N)`. When extras were also converted, append ` (+M link rewrites)`. An empty catalog name uses that page's published path. The subject is never `snapshot:` and never the bare seed word `Venus`.
3. Replace newlines and other control characters in the name with a space, trim, and cap the subject at 72 bytes so a catalog name cannot add a second commit-message line.
4. Leave `snapshot_title` for callers that only have markdown. The Flush subject does not use it.

### L5

**Test hooks on `window.__VENUS_*` are shipped in every build**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). Catalog ops and the page-open hooks are installed only when `VITE_TESTIDS` is set. Unmount, and a later call with the flag off, deletes them. `__VENUS_FROM_DOC__` stays behind `__VENUS_E2E__`. Provider kind, hub transport, and page flavour stay available for the M1 checks.

**Problem.** `App.tsx` always installs `__VENUS_OPEN_DOC__`, `__VENUS_OPEN_VECTOR__`, and `__VENUS_INSERT_LINKED_DOC__`. Catalog open always installs `__VENUS_CATALOG_OPS__` and `__VENUS_CATALOG_READY__`. Any script on the page can create, delete, or move nodes and insert blocks without going through the UI. That is close to what the DOM already allows, and it makes the script one call. `__VENUS_FROM_DOC__` is already limited to `__VENUS_E2E__`.

**Where.** `apps/web/src/App.tsx`. `apps/web/src/host/catalog/open.js` (`attachCatalogTestHooks`).

**Proposed fixes.**
1. Install those hooks only when `testidsFromEnv()` is true (`VITE_TESTIDS`). Playwright already sets that flag. A production build leaves the properties unset. Do not add a second env flag.
2. The cleanup that deletes the properties on unmount stays. A flag that turns off at runtime must not leave the previous functions in place.
3. Test: with the flag off, `window.__VENUS_OPEN_DOC__` and `window.__VENUS_CATALOG_OPS__` are undefined. With the flag on, the existing e2e helpers still find them.

### L6

**Every catalog event rebuilds the whole tree, and the header renames on every keystroke**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). The title input keeps a local draft and writes the catalog on blur, on Enter, and after 300 ms idle. Blur commits that draft. One committed name is one catalog transaction, and `gitName` is derived once inside it. Catalog `onChange` schedules a single tree rebuild per animation frame. Updating one row in place when a deep event is only `name`, `gitName`, or `tags` is still the follow-up in fix 4.

**Problem.** `listenCatalogHost` runs one `observeDeep`. Each event rebuilds the whole tree, reprojects catalog docs, and refreshes the header. The header title input calls `onTitleChange` on every `change`, and that calls `ops.rename`. One keystroke is one catalog transaction (`name` and `gitName`), one hub frame, one rebuild in every tab, and one `uniqueGitName` probe. Each intermediate name stays in the CRDT history.

**Where.** `apps/web/src/host/catalog/listen.js` (`listenCatalogHost`). `apps/web/src/host/chrome/VenusHeader.tsx` (the title `onChange`). `apps/web/src/App.tsx` (`onPageTitleChange`).

**Proposed fixes.**
1. The title input updates local draft state on `change`. It writes the catalog on blur, on Enter, and after 300 ms without another key. Blur must commit the draft. Today blur copies the last catalog title back over the input, which would throw away the name if the write moved off `change`.
2. One committed name is one catalog transaction. `gitName` is derived once from that name.
3. `onChange` from the catalog schedules a single `rebuildTree` per animation frame. Events in the same frame share that rebuild.
4. When the `observeDeep` events touch one node and the keys are `name`, `gitName`, or `tags`, update that row in place. A parent change, a create, or a delete still rebuilds the whole tree. Land the frame batch first. The in-place update is the follow-up, because a deep event is not always a single key.

### L7

**Untrusted names and `pageId`s are written into markdown without full escaping**

[↑ Summary](#summary)

**Status:** `fixed` (2026-10-06). Link labels in Rust and the host escape markdown markers, turn control characters into spaces, and trim. A page id that fails the safe-id check publishes no URL. A safe id with no catalog still uses `./workspace/`.

**Problem.** `escape_link_text` and `escape_text` escape markdown markers and leave newlines and other control characters in place. A catalog `name` of `\n# Injected` breaks out of the link into a new block. The missing-target path already turns controls into spaces. The normal catalog link does not. When the page id is not a safe id, `linked_doc_inline` still interpolates it into a `./workspace/…` URL.

**Where.** `crates/venus-sidecar/src/links.rs` (`escape_link_text`, `catalog_linked_doc_link`). `crates/venus-sidecar/src/from_doc/markdown.rs` (`linked_doc_inline`, `escape_text`). The host copy is `escapeLinkText` in `apps/web/src/host/mdgate/from-doc.js`.

**Proposed fixes.**
1. One escape used by both languages. After the marker escapes, replace every control character, including newline and tab, with a space, then trim. `missing_linked_doc` uses that same function instead of its own control scan.
2. `linked_doc_inline` emits a link only for a page id that passes `is_safe_page_id` (the same character rule the host uses). Anything else emits no URL. It does not fall through to `./workspace/{workspace}/{pageId}`.
3. Test: a catalog name of `a\n# Injected` publishes as a single-line link label. A `pageId` of `../x` publishes no `./workspace/` href.

### L8

**`POST /flush` can be triggered cross-site**

[↑ Summary](#summary)

**Status:** `deferred` — gateway. Cross-site access to `POST /flush` belongs to the standalone gateway, the same component as C1. The custom flush header is not added to the sidecar or the debug bar in this M4 slice. The problem and the checks below stay as the brief for that component.

**Problem.** An empty `POST` is a CORS simple request, so any site the developer visits can POST to the sidecar. H1 already returns 404 when `?workspace=` is not this wiki. A request that omits `?workspace=` still flushes the bound wiki and returns 204, with no job body.

**Where.** `crates/venus-sidecar/src/http.rs` (`flush`, `cors_layer`). `apps/web/src/host/chrome/VenusDebugBar.tsx` (`onFlush`).

**Proposed fixes.**
1. Require `X-Venus-Flush: 1`. A POST without it returns 400 and does not enqueue. Add that header to `allow_headers` so the app's preflight succeeds. A cross-site page cannot set it without a preflight the sidecar will not allow for an unlisted origin.
2. The debug bar sends the header. No other product caller posts `/flush`.
3. Return `202` and the same JSON body as `GET /flush/status` (sha, last error, attempts, failed). Keep `409` when the job is already `failed`. A bound wiki with no `?workspace=` is still the one that flushes.

### L9

**`.venus/ids/doc:home.json` contains a colon**

[↑ Summary](#summary)

**Status:** `accepted` (2026-10-06). This wiki does not run on Windows, and it will not. The home sidecar file stays `.venus/ids/doc:home.json`. The rename below is not implemented.

**Problem.** Home's sidecar file is `.venus/ids/doc:home.json`. Windows, and some sync tools, reject `:` in a file name, so `git clone` of the wiki fails the checkout on those systems. Other pages already use a uuid in that directory.

**Where.** `crates/venus-sidecar/src/git.rs` (`sidecar_rel`). The home doc id is `doc:home`. Its SQL uuid is `395cd07b-bdb1-5f54-ada8-e9a3fabb6a20`.

**Proposed fixes.**
1. Name every sidecar file by the page's SQL uuid: `.venus/ids/395cd07b-bdb1-5f54-ada8-e9a3fabb6a20.json` for home, and the page uuid for every other page. Do not percent-encode the colon. The uuid is already a safe file name.
2. On the next Flush, if HEAD has `.venus/ids/doc:home.json` and not the uuid path, copy that blob onto the uuid path and remove the colon path in the same index commit M6 already builds. Do not `git mv` in the working tree before the commit.
3. Test: a repository whose tree contains the colon path gets a commit that has the uuid path and does not have `doc:home.json`. A checkout of that commit contains no colon in `.venus/ids/`.

### L10

**Test, documentation, and leftover-edge gaps**

[↑ Summary](#summary)

**Problem.** Five leftovers. The link e2e never checks that the target file is in git, that the card title survives a reload, or that the card is the linked-doc style. `pnpm tsc` still reports `drop.test.ts(102)` (`o1` / `o2` possibly undefined), from an earlier step. [README.md](./README.md) links to `step-3-findings.md`, which is not in the tree or in git history. `compose_flush_bytes` and `persist_on_flush` upsert outbound edges for converted pages that are not in the walk (created and deleted between two Flushes, so absent from `old_pages` too). Those edges are never removed. Inbound lookup ignores a source that is not in `doc_to_sql`, so they are unused, and they grow. Deleted pages keep their CRDT rows in SQL and can still be opened by uuid. M5 decided how home is renamed. It did not decide how long a deleted page is kept.

**Where.** `apps/web/e2e/m4-link.spec.ts`. `apps/web/src/host/catalog/drop.test.ts`. `docs/design/M4/README.md`. `crates/venus-sidecar/src/links.rs` (`compose_flush_bytes`, `persist_on_flush`). `page_identity` and `Room::ensure_doc`.

**Proposed fixes.**
1. Extend `m4-link.spec.ts` with three assertions: the target path exists in the wiki git tree after Flush, the card title after reload is the catalog name, and the card has the linked-doc style rather than the deleted style.
2. In `drop.test.ts`, bind `o1` and `o2` only after the lookup has found them, so `tsc` sees a definite value. Do not change the drop behavior.
3. Remove the `step-3-findings.md` row from `docs/design/M4/README.md`. This file is the review record for steps 1–7.
4. In both `compose_flush_bytes` and `persist_on_flush`, ignore converted ids that are not in the walk before upserting outbound edges. A source that is not published does not get an edge. Add a test where a page is converted and absent from the walk, and the written `links.json` has no outbound entry for it.
5. Add a tombstone on `page_identity` when a page leaves the walk. `ensure_doc` returns 404 for a tombstoned uuid. Do not delete the CRDT rows in this change. A GC job waits until a retention rule says how long a deleted page can still be opened.

---

## Suggested order

1. **C1** and **L8** (cross-site `POST /flush`) are deferred to the gateway (separate component). **H1** (workspace binding) is fixed.
2. **H5** (job attempts, status, backoff) is fixed.
3. **H4** (sidecar-owned uniqueness plus two-phase moves) is fixed. **M2** (inbound of deleted targets, missing-target export) is fixed.
4. **H2** and **H3** are fixed: seed on create, and project the catalog into the workspace docs and `docMetas`.
5. **H6** (reconnect) is fixed. Step 8's SharedWorker should reuse this provider.
6. **M1** (orphan and cycle repair) is fixed. **M3** (encoded hrefs and one link-text escape table) is fixed. **M4** (`serde_yaml` for `pages.yaml`, path-to-doc from `page_identity`) is fixed. **M5** is decided: home may be renamed, and git follows the walk (b). **M6** (commit from HEAD, then one SQL transaction for identity and `last_flushed`) is fixed. **M7** (a Flush that reaches the two-minute lease fails, stops, and releases that lease) is fixed. **M8** (per-document eviction, plus caps on open documents and sockets per address) is fixed. **L1** (a failed link index fails the Flush; a missing blob is rebuilt from HEAD) is fixed. **L2** (one catalog walk per Flush, shared with every converted page) is fixed. **L3** (`/git/log` is capped and cached; the debug bar refetches it when the flush sha changes) is fixed. **L4** (commit subjects) is accepted: autocomment stays as it is, and an AI that analyzes the changes writes the commit text. **L5** (catalog and page-open hooks install only when `VITE_TESTIDS` is set) is fixed. **L6** (the title commits on blur, Enter, and 300 ms idle; one catalog rebuild per animation frame) is fixed. The in-place tree row update is still the follow-up. **L7** (link labels fold control characters, and an unsafe page id publishes no URL) is fixed. **L9** (the colon in `doc:home.json`) is accepted: this wiki never runs on Windows, so the file is not renamed. The remaining Medium and Low items are cleanup before M4 exits. M5's rename is decided and not implemented. M8's optional catalog-membership check is not implemented.
