# Frontend components

Host chrome in `apps/web`. BlockSuite stays the page editor. These pieces wrap **Yjs spaces** that already sync through the [hub](../backend/hub/).

Do **not** import `@affine/core`. Do **not** put server URLs in `mount-editor.js`.

| Component | Role |
|---|---|
| [CRDT tree](./crdt-tree/) | Wiki folders/docs. Catalog CRDT is the data; React tree is the view. Drop reparents live. **name** vs POSIX **filename** / `gitPath`: [page-identity](../../datamodel/page-identity.md). |

Markdown pane, Flush, and product header stay milestone host files until they earn a folder here.
