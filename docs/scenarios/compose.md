# Compose stack

**Feature:** one Compose file runs **postgres**, **hub**, and **web**. A new machine brings up the loop without memorizing Vite flags. [devops/compose](../devops/compose.md). Hub internals: [hub](../design/components/backend/hub/).

**Boxes:** Postgres, hub, nginx `web` ([architecture](../design/architecture.md#elements)).

## Run

```bash
docker compose config --services
pnpm test                     # compose.test.ts (file parse; docker config if daemon up)
pnpm compose:dod              # postgres + hub; spike survives restart / down without -v
pnpm compose:ha               # hub-b :3001 is 503 while A holds the M0 lease; SIGTERM drain
pnpm compose:up               # wait until web healthy
# then either a person on http://127.0.0.1:8080 (two tabs, A→B)
# or:
PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080 pnpm --filter @venus/web exec playwright test e2e/m1-two-tabs.spec.ts
```

`pnpm test:e2e:m1` without `PLAYWRIGHT_BASE_URL` still uses host Vite `:5174`.

## Vitest

| Spec | Proves |
|---|---|
| `compose.test.ts` — Product path | `docker-compose.yml` has `postgres`, `hub`, `web`; `hub-b` is profile `ha`; volume `pg-venus-data`; hub DSN `venus_hub`; host ports `127.0.0.1`; required `HUB_DB_*` / persist / compact; `HUB_CORS_ORIGINS`; no `octobase` |
| `compose.test.ts` — Image / NOTICE | `deploy/hub/Dockerfile` is `debian:bookworm-slim` `USER venus` with no baked DSN; `deploy/NOTICE` is MIT OR Apache-2.0, not AGPL keck |
| `compose.test.ts` — `docker compose config` | when the daemon is up, `--services` is `postgres` `hub` `web` |
| `compose.test.ts` — Server up (skip if `:3000` down) | `POST /collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` → `{protocol:AFFiNE}`; `GET /` is `venus-hub` |
| `pnpm compose:dod` | Server up + spike persist across `restart hub` and `down` without `-v` |
| `pnpm compose:ha` | Second owner on `:3001` is 503; SIGTERM on A flushes and drops the lease so B hydrates |

## Playwright (optional Compose base URL)

| Spec | Proves |
|---|---|
| `e2e/m1-two-tabs.spec.ts` with `PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080` | A→B on the **web service URL** (same-origin WS through nginx) |
