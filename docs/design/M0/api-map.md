# M0 API map

BlockSuite renames often. [plan.md](./plan.md) uses **design names** from [venus-implementation-plan.md](../venus-implementation-plan.md). This file records the **installed** symbols after `step-recon`.

Fill the **Actual** column from the pinned packages. Do not keep coding against names that are not in this table.

Pinned (fill in `step-pin`):

| Package | Version |
|---|---|
| `@blocksuite/affine` | |
| `@blocksuite/store` (if separate from affine re-export) | |
| `yjs` | |
| `@toeverything/theme` | |
| AFFiNE git SHA used as the copy source (editor container) | |

## Names

| Design name | Likely 0.27 (unverified) | Actual import | Notes |
|---|---|---|---|
| Collection / workspace | `TestWorkspace` or `Workspace` from `@blocksuite/affine/store` / `@blocksuite/store` / `@blocksuite/store/test` | | One instance. Id is a string. |
| Schema | `Schema` + `AffineSchemas` from `@blocksuite/affine/schemas` | | Register before creating docs. |
| Page / doc (Y.Doc wrapper) | `collection.createDoc(id)` then `.getStore()` → `Store` | | Design doc still says `DocCollection.createDoc()`. |
| Y.Doc | `store.spaceDoc` or equivalent | | Needed for the M1 provider stub. Confirm the getter name. |
| Store extensions | `StoreExtensionManager` + `getInternalStoreExtensions()` from `@blocksuite/affine/ext-loader` and `@blocksuite/affine/extensions/store` | | `workspace.storeExtensions = manager.get('store')` |
| View / page specs | `ViewExtensionManager` + `getInternalViewExtensions()` from `@blocksuite/affine/ext-loader` and `@blocksuite/affine/extensions/view` | | `editor.pageSpecs = [...viewManager.get('page'), FontConfigExtension(...)]` |
| Effects (custom elements) | `import '@blocksuite/affine/effects'` (and outline view effects if not pulled by the manager) | | Call once at app boot. |
| Editor container | Copy of AFFiNE `TestAffineEditorContainer` (tag `affine-editor-container`). **Not** `@affine/core`. | | Properties: `doc`, `mode`, `pageSpecs`, `host`, `std`. |
| EditorHost | `editor.host` (`@blocksuite/affine/std`) | | Outline takes **host**, not the container. |
| Outline | `OutlinePanel` from `@blocksuite/affine/fragments/outline` | | `panel.editor = host`; `fitPadding`. |
| Fonts | `FontConfigExtension(CommunityCanvasTextFonts)` from `@blocksuite/affine/shared/services` | | Without this, glyphs look wrong or missing. |
| Theme CSS | `@toeverything/theme/style.css` + `fonts.css` | | |
| Page title | `affine:page` + `Text` from store | | |
| Seed tree | `affine:page` → `affine:surface` + `affine:note` → `affine:paragraph` / `affine:heading` | | Same tree as [venus-design.md](../venus-design.md#published-document). |

## Forbidden imports

These must not appear in `apps/web`:

- `@affine/core`, `@affine/graphql`, any copilot / AI package
- OctoBase, y-websocket, Hocuspocus, `y-indexeddb` (those are M1+)
- Docusaurus / VitePress / AFFiNE explorer as a TOC
