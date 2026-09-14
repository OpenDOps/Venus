# M4 — Folder tree + links + product header

A **catalog CRDT** on the hub (folders, reorder, rename, `gitPath`), a **tree UI** that reparents live, a **thin product header** (undo / redo / current page), **`git mv` on publish**, and **`affine:embed-linked-doc`** that survives a move. **No** lease, comment-commit, or product/wiki remotes as this exit.

**Status: not started** — board at [step-recon-catalog](./plan.md#1-step-recon-catalog). **Gate:** [M3](../M3/README.md) **closed** (board steps 1–9 `done`). M3 still one page and a constant `gitPath`. This folder existing is not permission to mint a second space or ship a wiki TOC.

This folder is the implementation contract for the M4 slice in [venus-implementation-plan.md](../venus-implementation-plan.md). Catalog shape: [datamodel catalog](../datamodel/crdt.md#catalog). Tree component (CRDT + React DnD): [CRDT tree](../components/frontend/crdt-tree/). Git folders + `git mv`: [datamodel git](../datamodel/git.md). Pin then convert (catalog in the same cut): [LiveSnapshot](../LiveSnapshot/README.md). Linked-doc export form: [MDGate subset](../MDGate/subset.md#linked-doc-stable-form). Header is host chrome, not `@affine/core`: [implementation plan — product header](../venus-implementation-plan.md#product-header--venus-chrome-with-the-folder-tree).

| File | Role |
|---|---|
| [plan.md](./plan.md) | Story, [steps summary](./plan.md#steps-summary), work, exact test scenarios |
| [M4.state.yaml](./M4.state.yaml) | Board: step status only |

Shared: [api-map.md](../api-map.md) (catalog / tree / header Actuals filled in `step-recon-catalog`). Words: [glossary.md](../glossary.md). Hub (live CRDT, still wiki sticky): [M3.0](../M3.0/README.md). Snapshotter: [M3](../M3/README.md).

**Depends on:** [M3](../M3/README.md) closed. Host already syncs `doc:home` on the hub; Flush writes `wiki/spec/home.md`. `mount-editor` must stay unaware of catalog, tree, and header.

**Exit:** two pages, one link, move a page to another folder, git tree matches, link still resolves. Header undo/redo matches keyboard undo on the open page.

**Next:** [M5 — Lease + freeze](../venus-implementation-plan.md#m5--lease--freeze-week). After this exit (not this board): record wiki remote + product remote@branch ([code-bind](../Agents/code-bind.md)).
