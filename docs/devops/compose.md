# Compose

**Today (M3.0):** **three services** on the product path, one file, [docker-compose.yml](../../docker-compose.yml) at the repo root. Postgres is the only database. The collab front is the Venus **hub** (Rust + y-octo). The web image does **not** contain OctoBase source. How the hub works: [hub](../design/components/hub/). Hub HA: [M3.0/high-availability.md](../design/M3.0/high-availability.md).

keck stays under `deploy/octobase/` as M1 history; product Compose does not build it. `hub-b` is an optional `--profile ha` process to prove `workspace_lease` (not in default `up`).

## Services

| Service | Image | Host port | Role |
|---|---|---|---|
| `postgres` | `postgres:16` | none | `crdt_*` + blob bytes. Volume `pg-venus-data`, database `venus`. |
| `hub` | `deploy/hub/Dockerfile` (`debian:bookworm-slim`, `USER venus`) | `3000` | Yjs WS `/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00`, blob HTTP, doc export. |
| `web` | `deploy/web/Dockerfile` (nginx + `apps/web/dist`) | `8080` | Host UI. Same-origin proxy: `/api` and `/collaboration` → `hub:3000`. |

Default `docker compose config --services` prints `postgres`, `hub`, `web`. Hub and Postgres must not share a container.

Hub Postgres env (required; no SQLite): `POSTGRES_HOST`, `POSTGRES_USER`, `POSTGRES_PASSWORD` (Compose: `postgres` / `venus` / `venus`). `DATABASE_URL` overrides if set. `POSTGRES_SSLMODE` defaults to `disable` (private network); set `require` against managed Postgres.

Pool and timers are **required** on the hub (no sqlx defaults): `HUB_DB_MAX_CONNECTIONS=32`, `HUB_DB_MIN_CONNECTIONS=4`, `HUB_DB_ACQUIRE_TIMEOUT_SECS=10`, `HUB_PERSIST_INTERVAL_MS=1000`, `HUB_COMPACT_AFTER=32`. Missing or blank is a start error that names the var. `32` is sized for 10 distinct cold exports + 10 blob GETs + 10 room flushes overlapping. Two hubs (`hub` + `hub-b`) hold up to 64 of Postgres’s default 100.

`HUB_CORS_ORIGINS` is the six localhost Vite/Compose origins so Playwright can preflight `hub:3000`. Set it empty to disable CORS when the browser is same-origin through nginx. The hub does not echo the request `Origin`. Methods are `GET,HEAD,POST,DELETE` (not `any()`).

## Commands

From the repo root. Full stack:

```bash
docker compose up --build
# same as: pnpm compose:up
```

Open **http://127.0.0.1:8080** (the runbook web URL). That is the Compose loop, not Vite `:5173`.

Playwright and `pnpm dev` + `.env` only need Postgres + hub:

```bash
docker compose up --build postgres hub
# same as: pnpm sync:up
```

Stop (keeps `pg-venus-data`):

```bash
docker compose down
# pnpm compose:down / pnpm sync:down
```

`docker compose down -v` **deletes** `pg-venus-data`. Do not use `-v` if the page must survive.

M1 used volume `pg-data` and database `jwst`. This cutover uses a **new** volume so keck rows are not mistaken for hub tables. Re-seed the M0 wiki. TEXT `workspace_id` / `doc_id` from earlier hub boots are altered to UUID on migrate (`venus-m0` / `doc:home` map to the v5 constants; other slugs fail — `docker compose down -v`).

## Same-origin

Compose `web` bakes `VITE_SYNC_URL=same-origin`. The browser opens `ws://127.0.0.1:8080/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` (and `/api/blobs/…` on the same origin). nginx forwards the `AFFiNE` subprotocol to the hub.

Host Vite still uses `VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` and proxies `/api` only ([vite.config.ts](../../apps/web/vite.config.ts)). Playwright `pnpm test:e2e:m1` stays on that path (Vite `:5174`).

Point Playwright at Compose web:

```bash
PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080 pnpm --filter @venus/web test:e2e:m1
```

That skips Vite `webServer`. Export curl is still hub `:3000` (api-map **Export command**).

## Layout

```text
docker-compose.yml
deploy/NOTICE                 # hub MIT/Apache; keck history
deploy/hub/Dockerfile         # build rust:1.90-bookworm; runtime slim USER venus
deploy/web/Dockerfile         # node build + nginx; no OctoBase COPY
deploy/web/nginx.conf
crates/venus-hub/
```

Hub healthcheck is `bash -c 'echo >/dev/tcp/127.0.0.1/3000'`. Slim has bash; do not switch the runtime to distroless without changing that probe.

`.gitignore` does not ignore Compose files. Persist is the named volume `pg-venus-data`, not an anonymous volume.

Kubernetes (later): [kubernetes.md](./kubernetes.md). Hub HPA / gateway (after M3.0): [hub-fleet.md](./hub-fleet.md).
