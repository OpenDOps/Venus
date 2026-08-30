# Editor host

**Feature:** one page editor with seed and in-page outline. No sync required.

**Boxes:** Host, BlockSuite Store, outline ([architecture](../design/architecture.md#elements)).

## Run

```bash
pnpm test
pnpm test:e2e
```

Manual: [runbook — Manual testing (M0)](../runbook.md#manual-testing-m0-close-out).

## Vitest

| Spec | Proves |
|---|---|
| `apps/web/src/host/recon.test.ts` | api-map Actual specifiers resolve in Node |
| `workspace.test.ts` — single page default tree | one doc `doc:home`; `affine:page` → note + surface |
| `workspace.test.ts` — no sync socket on create | no `WebSocket` unless a live provider is passed |
| `seed.test.ts` | title `Venus`, H1 `Why Venus`, H2 `Empty host`; constructor undo does not delete the tree |
| `outline.test.ts` | `OutlinePanel` is heading TOC, not a folder tree |
| `editor.test.ts` | no `@affine/core` in the web package or host imports |

## Playwright (`pnpm test:e2e`)

| Spec | Proves |
|---|---|
| `e2e/m0-seed.spec.ts` | fresh load shows title + H1 + H2; one undo keeps the seed tree |
| `e2e/m0-editor.spec.ts` | type into the note; slash menu; undo reverts typed text |
| `e2e/m0-outline.spec.ts` | outline lists seed headings; H1 edits track; click H2 scrolls; not a wiki tree |
| `e2e/m0-smoke.spec.ts` | outline H1 + type; refresh discards type (also [hydrate](./hydrate-persist.md)) |
