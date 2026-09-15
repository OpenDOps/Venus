# TMP — still open before M4 step 1

**Scratch.** Decided product is in [api-map.md](../api-map.md) and [page-identity](../datamodel/page-identity.md), not here. Delete this file when these forks have Actuals (or are recorded as coding pins).

Do not reopen wire A, catalog guid, export, tree pin `1.7.0`, header undo APIs, header title (catalog `name` only), linked-doc git form, create/rename map, folder ids / wiki root, seed `folder:spec` + `doc:home`, delete (reject if children; **open page → home**; `node_not_empty` / `home_protected` = HTTP 409 / gRPC `FAILED_PRECONDITION`), sibling order (catalog field, **not git**), Flush autocomment (one dirty page = M3 `snapshot: <H1>`; several / catalog-only = `snapshot:`), create parent (`createAt` = selected / right-clicked folder), YAML map (pages **and** folders), or `page_identity` transport (**HTTP JSON** from the web; gRPC later if sidecar needs it).

---

## 1. Create-folder / delete testids

Page has `venus-create-page`. Folder create is “as needed.” Delete has no testid.

**Propose:** `venus-create-folder` in M4. `venus-delete-node`, hidden for home and nodes with children.

---

## 2. Open `?doc=` after delete

Wire A still hydrates an empty uuid. `crdt_*` may remain (no GC in M4).

**Propose:** host: if uuid is not in catalog / `page_identity`, **do not open** — bounce home. Hub stays bind-by-uuid.

---

## 3. Home rename / reparent

Delete home is forbidden. Plan tests **reparent home**. Rename home to `{uuid}.md` is forbidden.

**Propose:** **reparent allowed.** **Rename docname allowed** (filename filter; Flush `git mv`). Identity stays `doc:home` / `PAGE_SQL_ID`. Delete still forbidden.

---

## 4. Linked-doc card title (`docMetas`)

Step 7: `titleMiddleware` / `workspace.meta.docMetas` “when available.” Catalog `name` can be a uuid until rename. Export already uses catalog `name`.

**Propose:** on create/rename, set `docMetas` title = catalog `name`. Export still uses catalog `name`, not affine title.

---

## 5. `GET /git/log` path

M3 is hardcoded `spec/home.md`.

**Propose:** `?path=` = **open page `gitPath`**. Default remains `spec/home.md`.

---

## 6. Flush / git-log chrome

Step 4: “header or existing bar — Actual.”

**Propose:** leave **M3 chrome in App** (`venus-flush`, `venus-git-log`). Product header is undo/redo + title only.

---

## 7. Tree a11y

Step 5: “keyboard focus / aria tree Actual.”

**Propose:** headless-tree defaults (`role="tree"`). No extra a11y library.

---

## 8. Persist: one loop vs per-doc

Step 2 Actual. One persist task per doc, or one task that drains every buffer.

**Propose:** **one persist tick per Room** that drains every doc buffer (today’s hub). Per-doc tasks wait.

---

## 9. ListDocs vs advertisement

GET advertisement lists home + catalog bind facts. ListDocs “may add” other SQL uuids. Hub must not parse catalog to name pages.

**Propose:** GET advertisement **unchanged**. **ListDocs** = those two + `page_identity` rows.

---

## 10. `packages/catalog`

Implementation-plan layout has `packages/catalog/`. M4 plan default is `apps/web/src/host/catalog/`.

**Propose:** **host folder only.** No new workspace package in M4.
