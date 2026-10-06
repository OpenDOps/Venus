# M4 — Folder tree + links + product header

A **catalog CRDT** on the hub (folders, reorder, rename, `gitPath`), a **tree UI** that reparents live, a **thin product header** (undo / redo / current page), **`git mv` on publish**, **`affine:embed-linked-doc`** that survives a move (Flush reverse index `.venus/links.json` — not `fromDoc` every page), and a last-step **SharedWorker** in front of wire A when the browser has it. **No** lease, comment-commit, or product/wiki remotes as this exit.

**Status: [step-links](./plan.md#7-step-links) done** (7.1 + 7.2) — next [step-shared-worker](./plan.md#8-step-shared-worker). **Gate:** [M3](../M3/README.md) **closed** (board steps 1–9 `done`). **Wire:** N sockets, one Room, `?doc=<sql uuid>` on `/collaboration/:workspace_id` (omit = home). SharedWorker is [step 8](./plan.md#8-step-shared-worker); per-tab sockets must already work. This folder existing is not permission to mint a second space or ship a wiki TOC.

This folder is the implementation contract for the M4 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). Catalog shape: [datamodel catalog](../datamodel/crdt.md#catalog). Tree component (CRDT + React DnD): [CRDT tree](../components/frontend/crdt-tree/). Git folders + `git mv`: [datamodel git](../datamodel/git.md). Create / uuid.md / rename / map: [page-identity](../datamodel/page-identity.md) ([hub](../components/backend/hub/page-identity.md)). Pin then convert (catalog in the same cut): [LiveSnapshot](../LiveSnapshot/README.md). Linked-doc export form: [MDGate subset](../MDGate/subset.md#linked-doc-stable-form). Header is host chrome, not `@affine/core`: [implementation plan — product header](../venus-implementation-plan.md#product-header--venus-chrome-with-the-folder-tree).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M4.state.yaml](./M4.state.yaml) | Board: step status only |
| [create-rename-map.md](./create-rename-map.md) | Pointer. Canonical: [page-identity](../datamodel/page-identity.md). Hub SQL: [hub page-identity](../components/backend/hub/page-identity.md). |
| [step-3-findings.md](./step-3-findings.md) | Catalog CRDT revalidation (logic data-flows + fix proposals; security; perf already fixed) |
| [react-tree-issues.md](./react-tree-issues.md) | Step-tree React / network review: problem, flow, proposed fix per issue |
| [implementation-findings.md](./implementation-findings.md) | Steps 1–7 implementation review: findings by severity, data flow, proposed fixes |

Shared: [api-map.md](../api-map.md) (catalog / tree / header Actuals filled in `step-recon-catalog`). Words: [glossary.md](../glossary.md). Hub (live CRDT, still wiki sticky): [M3.0](../M3.0/README.md). Snapshotter: [M3](../M3/README.md).

**Depends on:** [M3](../M3/README.md) closed. Host already syncs `doc:home` on the hub; Flush writes `wiki/spec/home.md`. `mount-editor` must stay unaware of catalog, tree, and header.

**Exit:** home plus a page **created through the API** (not a pre-seeded `doc:protocol`), renamed in the tree, one link, move, git matches (`uuid.md` then `{filename}.md`), inbound hrefs rewritten via `.venus/links.json` (not `fromDoc` every page), delete of a non-empty folder rejected and a leaf page `git rm`’d, [page identity](../datamodel/page-identity.md) YAML restored from the catalog pin (`pages:` + `folders:`). Header undo/redo matches keyboard undo on the open page. SharedWorker is an optimization; two pages must work without it.

**Next:** [M5 — Lease + freeze](../venus-implementation-plan.md#m5--lease--freeze-week). After this exit (not this board): record wiki remote + product remote@branch ([code-bind](../Agents/code-bind.md)).
