# Runbook

Commands assume the **repo root**. pnpm workspace: `apps/*` (today: `@venus/web` only).

M0 contract: [design/M0](./design/M0/README.md). M1 contract: [design/M1](./design/M1/README.md) (done 2026-08-30). M2 contract: [design/M2](./design/M2/README.md) (done 2026-08-30). Product design: [design/venus-design.md](./design/venus-design.md). Dataflow: [design/architecture.md](./design/architecture.md). Deploy: [devops](./devops/README.md). Tests: [scenarios](./scenarios/README.md).

## Prerequisites

| | |
|---|---|
| Node | `>=22` (`engines` in root `package.json`) |
| pnpm | `10.19.0` (`packageManager` field; Corepack: `corepack enable`) |
| Docker | Compose v2. Needed for M1/M3.0 (`postgres` + `hub`; `web` for the three-service loop) and M3 Flush (`--profile snapshot` sidecar). Not needed for `pnpm dev`. |

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

M0 is **done** (2026-08-29). M1 is **done** (2026-08-30). M2 is **done** (2026-08-30). **M3.0 hub** is the product collab server ([hub](./design/components/backend/hub/)). **M3 git snapshotter is done** (2026-09-14): Flush writes nested `wiki/`; clone it elsewhere and read markdown ([M3](./design/M3/README.md)). `pnpm dev` serves a **full-viewport page editor** (title “Venus”, seeded H1 “Why Venus” / H2 “Empty host”) with a read-only **markdown source** pane on the left (highlight.js) and BlockSuite’s in-page outline on the right. Type, slash menu, and undo work. The markdown pane follows WYSIWYG without reload. Click a heading in the outline to scroll. Without `VITE_SYNC_URL`, refresh drops typed text (`MemoryNoopProvider`). With Compose **postgres** + **hub** + **web** (http://127.0.0.1:8080), refresh and a second tab keep the page. Host Flush + git log need `VITE_SIDECAR_URL` and Compose **sidecar** (`--profile snapshot`). `pnpm dev` stays memory-only until `VITE_SYNC_URL` is set. Next is **[M4 — folder tree](./design/M4/README.md)** (gated on M3 closed). Adapter: [MDGate](./design/MDGate/README.md).

Browser console noise from extensions (`contentscript.js`, MetaMask, ObjectMultiplex) is not Venus.

## Sync (hub)

Postgres and the Venus hub are **separate** Compose services. The hub is MIT/Apache (`deploy/NOTICE`, [hub](./design/components/backend/hub/)). The browser talks to the hub on `127.0.0.1:3000`; Postgres is not published. Host ports bind localhost only. `pnpm dev` does **not** start these. Map of the stack: [devops/compose](./devops/compose.md).

```bash
docker compose up --build postgres hub
# same as: pnpm sync:up
```

Wait until hub logs `listening on 0.0.0.0:3000` and `docker compose ps` shows both services running. Health:

```bash
curl -sSSf -X POST http://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00
# {"protocol":"AFFiNE"}
curl -sSSf http://127.0.0.1:3000/
# venus-hub
```

Persist across `restart hub` and `down` without `-v`: `pnpm compose:dod`. Second owner / drain: `pnpm compose:ha`.

**Doc export** (current Y.Doc as Yjs update v1; no browser; not `T0`):

```bash
curl -sSSf http://127.0.0.1:3000/api/block/77e4a2b1-8b40-5979-a73c-fd4477216d00/export -o /tmp/venus-page.yjs
```

Same command: api-map **Export command**. After the page has been hydrated at least once, the file should be more than a trivial empty update. Vitest: `apps/web/src/host/snapshot.test.ts` (Reachable / Decodes). [Doc export scenarios](./scenarios/doc-export.md). [M1 step 7](./design/M1/plan.md#7-step-snapshot).

Stop (keeps the named volume `pg-venus-data` — the doc survives):

```bash
docker compose down
# same as: pnpm sync:down / pnpm compose:down
```

`docker compose down -v` **deletes** `pg-venus-data`. Do not use `-v` if you need the workspace.

Host `cargo run -p venus-hub` is recon only (still needs Postgres). It is not the product server.

Point Vite at the hub (does not start Compose; `pnpm dev` stays memory-only without this):

```bash
# apps/web/.env — not required; copy from .env.example
VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00
```

Or one-shot: `VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00 pnpm dev`.

M1 Playwright against host Vite (Compose hub must already be up):

```bash
pnpm test:e2e:m1
```

That starts Vite on `127.0.0.1:5174` with `VITE_SYNC_URL` so it does not reuse memory-only `:5173`. Specs: [scenarios](./scenarios/README.md) (`m1-provider`, [hydrate](./scenarios/hydrate-persist.md) including `m1-smoke`, [two-tabs](./scenarios/collaboration.md), [blobs](./scenarios/blobs.md)). Doc export: [doc-export](./scenarios/doc-export.md) via `pnpm test`. Vite proxies `/api` to the hub. Do not `docker compose down -v` between those tests.

## Deploy (Compose web)

Three services, one command. **Web service URL:** http://127.0.0.1:8080

```bash
docker compose up --build
# same as: pnpm compose:up
```

Wait until `docker compose ps` shows `postgres`, `hub`, and `web` running (web healthy). Open that URL — not Vite `:5173`. Two tabs on it is the M1 loop on the hub ([plan](./design/M3.0/plan.md), [scenarios/compose](./scenarios/compose.md)). Person-in-browser close-out: [Manual testing (M1)](#manual-testing-m1-close-out).

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

Memory-only: `e2e/m0-*.spec.ts` and `e2e/m2-*.spec.ts`. Ignores `m1-*.spec.ts`. If a stale Vite is bound to 5173, kill it first — `reuseExistingServer` will reuse a broken process.

M1 e2e (`e2e/m1-*.spec.ts` including `m1-smoke.spec.ts`) needs Compose **hub** up, then `pnpm test:e2e:m1`. Doc export is Vitest (`snapshot.test.ts`), not Playwright. Compose `web` on `:8080` is [scenarios/compose](./scenarios/compose.md). Person-in-browser close-out is [Manual testing (M1)](#manual-testing-m1-close-out), [Manual testing (M2)](#manual-testing-m2-close-out), and [Manual testing (M3)](#manual-testing-m3-close-out).

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

First-paragraph `hello` / `from-a-…` / `from-b-…` mash on an old volume is leftover e2e typing (persist working). Title **Venus** and outline H1/H2 still count as seed. Wipe with `docker compose down -v` only if you want a clean note.

1. From the repo root: `pnpm compose:up` (or confirm `docker compose ps` shows `postgres`, `hub`, and `web` healthy). Open **http://127.0.0.1:8080** (Compose `web`, not Vite `:5173`).
2. **Seed.** Do not type yet. Title is **Venus**. Body has H1 **Why Venus** and H2 **Empty host**. The right-hand outline lists those headings only — no folders or other pages.
3. **Type.** Click the **empty paragraph at the top of the note**, not the title. Type `hello`. It appears in the note.
4. **Refresh.** Reload. `hello` is **still there**. Outline still matches (this fails M1 if it behaves like M0).
5. **Second tab.** Open the same URL in a second tab. It shows `hello` without typing. Type `tab-b` in B; A shows `tab-b` without reload.
6. **Image.** In A, insert an image (slash **Image** or paste). It renders. B shows the same image. Reload A; the image remains.
7. **WS.** DevTools → Network → **WS**: a sync socket is open to `/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` (same origin `:8080`, or `:3000` if you used host Vite). Not “no WS” like M0.
8. **Export.** From a terminal (no tab required):

```bash
curl -sSSf http://127.0.0.1:3000/api/block/77e4a2b1-8b40-5979-a73c-fd4477216d00/export -o /tmp/venus-page.yjs
```

Exit 0. File length **> 2** bytes.
9. **No AFFiNE shell.** `apps/web/package.json` still has no `@affine/core`. `mount-editor.js` still has no sync imports.
10. **Memory mode (optional sanity).** Unset sync env, `pnpm dev`: refresh **drops** text (M0 still works for people without Docker).

If any required step fails, M1 is not done. Fix provider, hydrate, blobs, or the server; do not fake two tabs with `localStorage`. Full contract: [M1 step 9](./design/M1/plan.md#9-step-verify).

## Manual testing (M2 close-out)

**Passed 2026-08-30** (live pane on Compose `:8080`; memory path below). Playwright does **not** replace this. Use Chrome or Firefox yourself. Fail on uncaught exceptions from the host or BlockSuite. Ignore extension noise (`contentscript.js`, MetaMask, ObjectMultiplex).

There is **no markdown mode switch**. Layout is three columns: markdown **source** (left), WYSIWYG (middle), outline (right).

1. From the repo root: `pnpm dev`. Open the Vite URL (usually `http://localhost:5173`). **Not** Compose `:8080` for this memory path.
2. **Source, not a preview.** Left pane shows `# Why Venus` and `## Empty host` as markdown (hash signs visible, token-colored). It is not a second rendered page. Outline on the right still lists those headings.
3. **Type.** Click the **empty paragraph at the top of the note**, not the title. Type a unique word (e.g. `m2-hello`). It appears in the note **and** in the left pane **without reload**. Tokens stay colored.
4. **Read-only.** Click the left pane and type. Nothing is inserted. The element is not `contenteditable`.
5. **Refresh (memory).** Reload. The unique word is **gone**. Seed title + H1/H2 are back. Pane matches the seed source again (same as M0 persist).
6. **No git.** There is no `wiki/` in the repo from this milestone. The pane is RAM only.
7. **Optional Compose.** `pnpm compose:up`, open **http://127.0.0.1:8080**. Type a unique word; pane updates; reload **keeps** the word **and** the pane still matches (M1 persist). Not required to close M2. Opt-in extra blocks: `?md-demo=1` (appends once; numbered lists, fences, linked-doc comment). Rebuild `web` after host changes (`docker compose up --build -d web`).
8. **Optional two tabs (M1).** Same URL in a second tab. Type a unique word in A; B’s **note** (not only the pane) should show it without reload. If B stays stale, check hub logs (`docker compose logs hub`) — y-octo apply must not panic. Do not `docker compose down -v` unless you want a clean seed.

If any required step (1–6) fails, M2 is not done. Fix the exporter or the pane loop; do not hide jitter by stripping whitespace. Full contract: [M2 step 8](./design/M2/plan.md#8-step-verify). Specs: [scenarios/markdown-projection](./scenarios/markdown-projection.md).

## Manual testing (M3 close-out)

Keep this checklist for close-out and regression. Playwright (`pnpm test:e2e:m3`) does **not** replace it. Use Chrome or Firefox yourself (not a screenshot, not Playwright headed mode). Fail on uncaught exceptions from the host or BlockSuite. Ignore extension noise (`contentscript.js`, MetaMask, ObjectMultiplex). There is **no** review-comment prompt on Flush.

Clone-elsewhere without the editor is `cargo test -p venus-sidecar --test verify` (CI). After a real Flush, `pnpm wiki:clone` copies `wiki/` to `/tmp/venus-wiki-clone`.

1. From the repo root: `mkdir -p wiki && chmod a+rwx wiki`, then `docker compose --profile snapshot up --build postgres hub sidecar` (or `pnpm compose:up` plus `--profile snapshot` sidecar). Host Vite: `VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` and `VITE_SIDECAR_URL=http://127.0.0.1:3002` (see `apps/web/.env.example`). Open the Vite URL on **:5174** if you copy the M3 Playwright ports, or Compose **http://127.0.0.1:8080** if `web` bakes `VITE_SIDECAR_URL`.
2. **Seed.** Title **Venus**. Body has H1 **Why Venus** and H2 **Empty host**. Flush and git-log chrome are visible (`data-testid="venus-flush"` / `venus-git-log`).
3. **Type.** Click the empty paragraph at the top of the note. Type a unique word (e.g. `m3-hello`). Wait ~2s for persist.
4. **Flush.** Click **Flush**. No review why. The editor stays editable (not `readonly`).
5. **Clone.** `pnpm wiki:clone` (or `git clone wiki /tmp/venus-wiki-clone`). Open `/tmp/venus-wiki-clone/spec/home.md` in an editor. It is ordinary markdown: seed headings **and** the unique word. No Postgres and no hub are required to read it. Fail if the file is Yjs binary.
6. **Git log.** `[data-testid="venus-git-log"]` shows `snapshot: …` (autocomment). Not Yjs undo labels.
7. **Second tab during Flush (optional delay).** Sidecar `SNAPSHOT_CONVERT_SLEEP_MS=3000` for a visible window (do not bake into Compose defaults). Open the same URL in tab B. Click Flush in A, type `during-flush` in A; B’s **note** shows it without reload. A can still type.
8. **No origin.** `git -C wiki remote` is empty. Product git still ignores `/wiki/`.

If any required step fails, M3 is not done. Fix the sidecar or host chrome; do not treat the live editor as the way to “see” git. Full contract: [M3 step 9](./design/M3/plan.md#9-step-verify).

## Build

```bash
pnpm build
```

Same as `pnpm --filter @venus/web build` → `tsc --noEmit && vite build`. Output: `apps/web/dist/`. Production minify stays **off** (lit-html / esbuild; see api-map pin notes).

Preview the production build:

```bash
pnpm --filter @venus/web preview
```
