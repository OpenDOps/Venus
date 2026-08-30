# Compose stack

**Feature:** one Compose file runs **postgres**, **octobase**, and **web**. A new machine brings up the M1 loop without memorizing Vite flags. [devops/compose](../devops/compose.md).

**Boxes:** Postgres, keck, nginx `web` ([architecture](../design/architecture.md#elements)).

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
| `compose.test.ts` — Three services | `docker-compose.yml` has separate `postgres`, `octobase`, `web`; named volume `pg-data`; no `USE_MEMORY_SQLITE` |
| `compose.test.ts` — `docker compose config` | when the daemon is up, `--services` is those three names |

## Playwright (optional Compose base URL)

| Spec | Proves |
|---|---|
| `e2e/m1-two-tabs.spec.ts` with `PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080` | A→B on the **web service URL** (same-origin WS through nginx) |

## Manual

Step 8 DoD allows a person: `docker compose up --build`, open http://127.0.0.1:8080, two tabs, type `from-a` in A, B sees it without reload (10s). Full close-out is [M1 step 9](../design/M1/plan.md#9-step-verify).
