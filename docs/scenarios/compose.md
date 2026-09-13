# Compose stack

**Feature:** one Compose file runs **postgres**, **hub**, and **web**. A new machine brings up the loop without memorizing Vite flags. [devops/compose](../devops/compose.md). Hub internals: [hub](../design/components/hub/).

**Boxes:** Postgres, hub, nginx `web` ([architecture](../design/architecture.md#elements)).

## Run

```bash
docker compose config --services
pnpm test                     # compose.test.ts (file parse; docker config if daemon up)
pnpm compose:up               # wait until web healthy
# then either a person on http://127.0.0.1:8080 (two tabs, A→B)
# or:
PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080 pnpm --filter @venus/web exec playwright test e2e/m1-two-tabs.spec.ts
```

`pnpm test:e2e:m1` without `PLAYWRIGHT_BASE_URL` still uses host Vite `:5174`.

## Vitest

| Spec | Proves |
|---|---|
| `compose.test.ts` — Product path | `docker-compose.yml` has `postgres`, `hub`, `web`; `hub-b` is profile `ha`; volume `pg-venus-data`; `POSTGRES_HOST` / `USER` / `PASSWORD`; required `HUB_DB_*` / persist / compact; `HUB_CORS_ORIGINS`; no `octobase` |
| `compose.test.ts` — `docker compose config` | when the daemon is up, `--services` is `postgres` `hub` `web` |

## Playwright (optional Compose base URL)

| Spec | Proves |
|---|---|
| `e2e/m1-two-tabs.spec.ts` with `PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080` | A→B on the **web service URL** (same-origin WS through nginx) |
