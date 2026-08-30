# Runbook

Commands assume the **repo root**. pnpm workspace: `apps/*` (today: `@venus/web` only).

M0 contract: [design/M0](./design/M0/README.md). M1 contract: [design/M1](./design/M1/README.md) (done 2026-08-30). M2 contract: [design/M2](./design/M2/README.md) (not started). Product design: [design/venus-design.md](./design/venus-design.md). Dataflow: [design/architecture.md](./design/architecture.md). Deploy: [devops](./devops/README.md). Tests: [scenarios](./scenarios/README.md).

## Prerequisites

| | |
|---|---|
| Node | `>=22` (`engines` in root `package.json`) |
| pnpm | `10.19.0` (`packageManager` field; Corepack: `corepack enable`) |
| Docker | Compose v2. Needed for M1 (`postgres` + `octobase`; `web` for the three-service loop). Not needed for `pnpm dev`. |

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

M0 is **done** (2026-08-29). M1 is **done** (2026-08-30). `pnpm dev` serves a **full-viewport page editor** (title “Venus”, seeded H1 “Why Venus” / H2 “Empty host”) with BlockSuite’s in-page outline on the right. Type, slash menu, and undo work. Click a heading in the outline to scroll. Without `VITE_SYNC_URL`, refresh drops typed text (`MemoryNoopProvider`). With Compose **postgres** + **octobase** + **web** (http://127.0.0.1:8080), refresh and a second tab keep the page. `pnpm dev` stays memory-only until `VITE_SYNC_URL` is set. Next: [M2 markdown projection](./design/M2/README.md) ([plan](./design/M2/plan.md)). Adapter contract: [MDGate](./design/MDGate/README.md).

Browser console noise from extensions (`contentscript.js`, MetaMask, ObjectMultiplex) is not Venus.

## Sync (M1)

Postgres and OctoBase keck are **separate** Compose services. keck is AGPL-3.0 (`deploy/NOTICE`). The browser talks to keck on `:3000`; Postgres is not published. `pnpm dev` does **not** start these. Map of the stack: [devops/compose](./devops/compose.md).

```bash
docker compose up --build postgres octobase
# same as: pnpm sync:up
```

Wait until keck logs `listening on 0.0.0.0:3000` and `docker compose ps` shows both services running. Health (no swagger; `JWST_DEV` off):

```bash
curl -sSSf -X POST http://127.0.0.1:3000/collaboration/venus-m0
# {"protocol":"AFFiNE"}
```

**Doc export** (current Y.Doc as Yjs update v1; no browser; not `T0`):

```bash
curl -sSSf http://127.0.0.1:3000/api/block/venus-m0/export -o /tmp/venus-m0.yjs
```

Same command: api-map **Export command**. After the page has been hydrated at least once, the file should be more than a trivial empty update. Vitest: `apps/web/src/host/snapshot.test.ts` (Reachable / Decodes). [Doc export scenarios](./scenarios/doc-export.md). [M1 step 7](./design/M1/plan.md#7-step-snapshot).

Stop (keeps the named volume `pg-data` — the doc survives):

```bash
docker compose down
# same as: pnpm sync:down / pnpm compose:down
```

`docker compose down -v` **deletes** `pg-data`. Do not use `-v` if you need the workspace.

Host `cargo run --bin keck` is recon history only (used SQLite). It is not the product server.

Point Vite at keck (does not start Compose; `pnpm dev` stays memory-only without this):

```bash
# apps/web/.env — not required; copy from .env.example
VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/venus-m0
```

Or one-shot: `VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/venus-m0 pnpm dev`.

M1 Playwright against host Vite (Compose keck must already be up):

```bash
pnpm test:e2e:m1
```

That starts Vite on `127.0.0.1:5174` with `VITE_SYNC_URL` so it does not reuse memory-only `:5173`. Specs: [scenarios](./scenarios/README.md) (`m1-provider`, [hydrate](./scenarios/hydrate-persist.md) including `m1-smoke`, [two-tabs](./scenarios/collaboration.md), [blobs](./scenarios/blobs.md)). Doc export: [doc-export](./scenarios/doc-export.md) via `pnpm test`. Vite proxies `/api` to keck. Do not `docker compose down -v` between those tests.

## Deploy (Compose web)

Three services, one command. **Web service URL:** http://127.0.0.1:8080

```bash
docker compose up --build
# same as: pnpm compose:up
```

Wait until `docker compose ps` shows `postgres`, `octobase`, and `web` running (web healthy). Open that URL — not Vite `:5173`. Two tabs on it is the M1 loop ([plan](./design/M1/plan.md), [scenarios/compose](./scenarios/compose.md)). Person-in-browser close-out: [Manual testing (M1)](#manual-testing-m1-close-out).

Playwright against nginx instead of Vite:

```bash
PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080 pnpm --filter @venus/web test:e2e:m1
```

Kubernetes is not in M1: [devops/kubernetes](./devops/kubernetes.md).

## Test

**What** each command asserts: [scenarios](./scenarios/README.md). Below is **how to invoke** the runners.

```bash
pnpm test
```

Same as `pnpm --filter @venus/web test` → `vitest run`. Node only (no browser). `snapshot.test.ts` Reachable / Decodes **skip** when nothing listens on `127.0.0.1:3000`, so machines without Docker still get a green `pnpm test`. With `pnpm sync:up` and `doc:home` hydrated, those tests run and must pass. `compose.test.ts` parses `docker-compose.yml` always; `docker compose config --services` runs when the daemon is up.

Watch mode (from `apps/web`, or via filter):

```bash
pnpm --filter @venus/web exec vitest
```

Browser e2e (Playwright; starts Vite on `127.0.0.1:5173` unless that port is already a dev server):

```bash
pnpm test:e2e
```

Memory-only: `e2e/m0-*.spec.ts`. Ignores `m1-*.spec.ts`. If a stale Vite is bound to 5173, kill it first — `reuseExistingServer` will reuse a broken process.

M1 e2e (`e2e/m1-*.spec.ts` including `m1-smoke.spec.ts`) needs Compose keck up, then `pnpm test:e2e:m1`. Doc export is Vitest (`snapshot.test.ts`), not Playwright. Compose `web` on `:8080` is [scenarios/compose](./scenarios/compose.md). Person-in-browser close-out is [Manual testing (M1)](#manual-testing-m1-close-out).

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

## Manual testing (M1 close-out)

**Passed 2026-08-30.** Keep this checklist for regression. Playwright does **not** replace it. Use Chrome or Firefox yourself (not a screenshot, not Playwright headed mode). Fail on uncaught exceptions from the host or BlockSuite. Ignore extension noise (`contentscript.js`, MetaMask, ObjectMultiplex).

First-paragraph `hello` / `from-a-…` / `from-b-…` mash on an old `pg-data` volume is leftover e2e typing (persist working). Title **Venus** and outline H1/H2 still count as seed. Wipe with `docker compose down -v` only if you want a clean note.

1. From the repo root: `pnpm compose:up` (or confirm `docker compose ps` shows `postgres`, `octobase`, and `web` healthy). Open **http://127.0.0.1:8080** (Compose `web`, not Vite `:5173`).
2. **Seed.** Do not type yet. Title is **Venus**. Body has H1 **Why Venus** and H2 **Empty host**. The right-hand outline lists those headings only — no folders or other pages.
3. **Type.** Click the **empty paragraph at the top of the note**, not the title. Type `hello`. It appears in the note.
4. **Refresh.** Reload. `hello` is **still there**. Outline still matches (this fails M1 if it behaves like M0).
5. **Second tab.** Open the same URL in a second tab. It shows `hello` without typing. Type `tab-b` in B; A shows `tab-b` without reload.
6. **Image.** In A, insert an image (slash **Image** or paste). It renders. B shows the same image. Reload A; the image remains.
7. **WS.** DevTools → Network → **WS**: a sync socket is open to `/collaboration/venus-m0` (same origin `:8080`, or `:3000` if you used host Vite). Not “no WS” like M0.
8. **Export.** From a terminal (no tab required):

```bash
curl -sSSf http://127.0.0.1:3000/api/block/venus-m0/export -o /tmp/venus-m0.yjs
```

Exit 0. File length **> 2** bytes.
9. **No AFFiNE shell.** `apps/web/package.json` still has no `@affine/core`. `mount-editor.js` still has no sync imports.
10. **Memory mode (optional sanity).** Unset sync env, `pnpm dev`: refresh **drops** text (M0 still works for people without Docker).

If any required step fails, M1 is not done. Fix provider, hydrate, blobs, or the server; do not fake two tabs with `localStorage`. Full contract: [M1 step 9](./design/M1/plan.md#9-step-verify).

## Build

```bash
pnpm build
```

Same as `pnpm --filter @venus/web build` → `tsc --noEmit && vite build`. Output: `apps/web/dist/`. Production minify stays **off** (lit-html / esbuild; see api-map pin notes).

Preview the production build:

```bash
pnpm --filter @venus/web preview
```
