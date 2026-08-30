# Runbook

Commands assume the **repo root**. pnpm workspace: `apps/*` (today: `@venus/web` only).

M0 contract: [design/M0](./design/M0/README.md). M1 contract: [design/M1](./design/M1/README.md) (in progress: steps 1–3). Product design: [design/venus-design.md](./design/venus-design.md).

## Prerequisites

| | |
|---|---|
| Node | `>=22` (`engines` in root `package.json`) |
| pnpm | `10.19.0` (`packageManager` field; Corepack: `corepack enable`) |
| Docker | Compose v2. Needed for M1 sync (`postgres` + `octobase`). Not needed for `pnpm dev`. |

Do not mix two BlockSuite or `yjs` majors. `yjs` is pinned to `13.6.32` via root `pnpm.overrides`.

## Install

```bash
pnpm install
```

Keep `pnpm-lock.yaml` committed.

## Dev

```bash
pnpm dev
```

Same as `pnpm --filter @venus/web dev` (Vite). Default URL is the Vite printout, usually `http://localhost:5173`.

M0 is **done** (2026-08-29). `pnpm dev` serves a **full-viewport page editor** (title “Venus”, seeded H1 “Why Venus” / H2 “Empty host”) with BlockSuite’s in-page outline on the right. Type, slash menu, and undo work. Click a heading in the outline to scroll. Refresh drops typed text (`MemoryNoopProvider` — memory-only). Next: [M1 — OctoBase loop](./design/M1/README.md) (sync over Compose **postgres** + **octobase**; refresh and a second tab keep the page). `pnpm dev` stays memory-only until `VITE_SYNC_URL` is set.

Browser console noise from extensions (`contentscript.js`, MetaMask, ObjectMultiplex) is not Venus.

## Sync (M1)

Postgres and OctoBase keck are **separate** Compose services. keck is AGPL-3.0 (`deploy/NOTICE`). The browser talks to keck on `:3000`; Postgres is not published. `pnpm dev` does **not** start these.

```bash
docker compose up --build postgres octobase
# same as: pnpm sync:up
```

Wait until keck logs `listening on 0.0.0.0:3000` and `docker compose ps` shows both services running. Health (no swagger; `JWST_DEV` off):

```bash
curl -sSSf -X POST http://127.0.0.1:3000/collaboration/venus-m0
# {"protocol":"AFFiNE"}
```

Stop (keeps the named volume `pg-data` — the doc survives):

```bash
docker compose down
# same as: pnpm sync:down
```

`docker compose down -v` **deletes** `pg-data`. Do not use `-v` if you need the workspace.

Host `cargo run --bin keck` is recon history only (used SQLite). It is not the product server.

Point Vite at keck (does not start Compose; `pnpm dev` stays memory-only without this):

```bash
# apps/web/.env — not required; copy from .env.example
VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/venus-m0
```

Or one-shot: `VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/venus-m0 pnpm dev`.

M1 Playwright (Compose must already be up):

```bash
pnpm test:e2e:m1
```

That starts Vite on `127.0.0.1:5174` with `VITE_SYNC_URL` so it does not reuse memory-only `:5173`.

## Test

```bash
pnpm test
```

Same as `pnpm --filter @venus/web test` → `vitest run`.

That is **Node Vitest**, not a browser. Current coverage:

1. **Actual imports resolve** (`apps/web/src/host/recon.test.ts`) — `import.meta.resolve` for each specifier in the [api-map](./design/api-map.md) Actual column (does not execute view/outline modules).
2. **Single page** (`apps/web/src/host/workspace.test.ts`) — `createM0Workspace()` has exactly one doc (`doc:home`) whose store is `affine:page` → `affine:note` (plus `affine:surface` under the page). Note starts with an empty paragraph; seed headings follow. `store.canUndo` is false after `resetHistory()`.
3. **No network** — creating the workspace does not construct `WebSocket`; `@venus/web` does not depend on `y-websocket`, `y-indexeddb`, `hocuspocus`, or `@affine/core`.
4. **No AFFiNE app shell** (`apps/web/src/host/editor.test.ts`) — `@affine/core` and `@blocksuite/integration-test` are absent from `@venus/web` dependencies and from host imports.
5. **Seed** (`apps/web/src/host/seed.test.ts`) — title `Venus`, H1 `Why Venus`, H2 `Empty host`; undo does not delete the tree.
6. **Outline is not a wiki TOC** (`apps/web/src/host/outline.test.ts`) — host mounts `OutlinePanel` from `@blocksuite/affine/fragments/outline`.
7. **Sync seam** (`apps/web/src/host/sync-provider.test.ts`) — default `MemoryNoopProvider`; a second `SyncProvider` is connected with `doc:home` + `store.spaceDoc`; `mount-editor.js` / `editor-container.js` / `boot.js` do not import the live client. `VITE_SYNC_URL` selects `OctoBaseKeckProvider` without opening a socket until `connect`.

Watch mode (from `apps/web`, or via filter):

```bash
pnpm --filter @venus/web exec vitest
```

Browser e2e (Playwright; starts Vite on `127.0.0.1:5173` unless that port is already a dev server):

```bash
pnpm test:e2e
```

That is Playwright vs Vite on `127.0.0.1:5173`, including `m0-smoke.spec.ts` (outline H1 + type + refresh). Also `m0-editor.spec.ts`, `m0-seed.spec.ts`, `m0-outline.spec.ts`, `m0-provider.spec.ts`. Ignores `m1-*.spec.ts`. If a stale Vite is bound to 5173, kill it first — `reuseExistingServer` will reuse a broken process.

M1 provider e2e (`e2e/m1-provider.spec.ts`) needs Compose keck up, then `pnpm test:e2e:m1`.

## Manual testing (M0 close-out)

Passed 2026-08-29. Keep this checklist for regression. Playwright does **not** replace it. Use Chrome or Firefox yourself (not a screenshot, not Playwright headed mode). Fail on uncaught exceptions from the host or BlockSuite. Ignore extension noise (`contentscript.js`, MetaMask, ObjectMultiplex).

1. From the repo root: `pnpm install` if needed, then `pnpm dev`. Open the Vite URL (usually `http://localhost:5173`).
2. **Seed.** Do not type yet. Title is **Venus**. Body has H1 **Why Venus** and H2 **Empty host**. The right-hand outline lists those headings only — no folders or other pages.
3. **Type.** Click the **empty paragraph at the top of the note**, not the title. Type `hello`. It appears in the note.
4. **Slash list.** Press Enter for a new empty paragraph (or click an empty one). Type `/`. The slash menu opens. Insert a **bulleted or numbered list**. The list is in the note (BlockSuite widget, not a Venus toolbar).
5. **H3.** Via `/` or the format bar, insert **Heading 3**. Type `Verify H3`. It appears in the outline without reload.
6. **Outline edit.** Change the H1 text in the editor. The outline label updates without reload.
7. **Scroll.** The page is long enough to scroll. Click **Empty host** in the outline. The editor scrolls so that H2 is in view.
8. **Undo.** Undo until `hello` is gone. Title **Venus** and seeded H1/H2 remain.
9. **Refresh.** Reload. `hello`, the list, and `Verify H3` are gone. Seed title + H1 + H2 are back. Outline matches the seed.
10. **No sync.** DevTools → Network → **WS**: no sync socket. `apps/web/package.json` still has no `@affine/core`.

If any step fails, M0 is not done. Fix the host; do not fake UI.

## Build

```bash
pnpm build
```

Same as `pnpm --filter @venus/web build` → `tsc --noEmit && vite build`. Output: `apps/web/dist/`. Production minify stays **off** (lit-html / esbuild; see api-map pin notes).

Preview the production build:

```bash
pnpm --filter @venus/web preview
```
