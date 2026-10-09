# Compose

**Today (M3.0):** **three services** on the product path, one file, [docker-compose.yml](../../docker-compose.yml) at the repo root. Postgres is the only database. The collab front is the Venus **hub** (Rust + y-octo). The web image does **not** contain OctoBase source. How the hub works: [hub](../design/components/backend/hub/). Hub HA: [M3.0/high-availability.md](../design/M3.0/high-availability.md).

keck stays under `deploy/octobase/` as M1 history; product Compose does not build it. `hub-b` is an optional `--profile ha` process to prove `workspace_lease` (not in default `up`). `sidecar` is an optional `--profile snapshot` process (`crates/venus-sidecar`, host `127.0.0.1:28720`); default `up` stays three services. Host binary: `CONVERT_CWD=apps/web WIKI_DIR=wiki cargo run -p venus-sidecar` from the repo root (health `GET /` → `venus-sidecar`). Observer/workers start when `DATABASE_URL` or `POSTGRES_HOST` is set. First Flush **autoinits** `WIKI_DIR` (default `wiki/`, gitignored). Compose bind-mounts `./wiki:/wiki`; `mkdir -p wiki && chmod a+rwx wiki` so `USER venus` can write. Host Flush chrome: `VITE_SIDECAR_URL=http://127.0.0.1:28720` (Compose web bakes that). `POST /flush` pulls `jobs.not_before` to now; a worker then pins, converts, and git-commits. Host `data-testid="venus-git-log"` lists `GET /git/log?path=spec/home.md` subjects. After Flush, `pnpm wiki:clone` copies `wiki/` to `/tmp/venus-wiki-clone` (no hub needed to read `spec/home.md`). Optional `SNAPSHOT_CONVERT_SLEEP_MS` (default **0**) sleeps after cut, before `fromDoc` — test hook only ([step-live-during-flush](../design/M3/plan.md#7-step-live-during-flush)); do not set it in ordinary Compose.

## Services

| Service | Image | Host port | Role |
|---|---|---|---|
| `postgres` | `postgres:16` | none | `crdt_*` + blob bytes. Volume `pg-venus-data`, database `venus`. Superuser `venus`. |
| `hub` | `deploy/hub/Dockerfile` (`debian:bookworm-slim`, `USER venus`) | `127.0.0.1:28710` (WS/HTTP), `127.0.0.1:28711` (gRPC `ExportDoc` / `ListDocs`) | Yjs WS `/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00`, blob HTTP, advertisement GET `/export`. Connects as `venus_hub` (NOSUPERUSER). `HUB_WORKSPACES` (default that uuid) is the only workspace the process opens; any other id is 404 / gRPC NotFound before a room or lease. |
| `web` | `deploy/web/Dockerfile` (nginx + `apps/web/dist`) | `127.0.0.1:28700` | Host UI. Same-origin proxy: `/api` and `/collaboration` → `hub:28710`. |
| `sidecar` | `deploy/sidecar/Dockerfile` (`node:22`, `USER venus`) | `127.0.0.1:28720` | Optional `--profile snapshot`. Observer + N workers on Venus `jobs` (`SKIP LOCKED`) for `WIKI_WORKSPACE_ID` only (default the M0 uuid). Health `GET /`. `POST /flush` pulls that workspace's `jobs.not_before` to now (does not pin); any other uuid is 404. `GET /git/log?path=spec/home.md` → `{ subject, sha }[]`. Workers autoinit `WIKI_DIR`, write `.venus/workspace`, and git-commit. DSN required for the queue (`DATABASE_URL` / `POSTGRES_*`, same role as hub). Bind-mount `./wiki:/wiki`. |
| `surreal-graph` | `surrealdb/surrealdb:v2.7.0` | `127.0.0.1:28730` | Optional `--profile graph`. RocksDB `rocksdb:/data/graph.db`, volume `surreal-graph-data`. Cap 1g, block cache `64MB`. |
| `surreal-search-0` | same image | `127.0.0.1:28731` | Optional `--profile graph`. `rocksdb:/data/search.db`, volume `surreal-search-0-data`. Same cap and cache. |
| `surreal-search-1` | same image | `127.0.0.1:28732` | Optional `--profile graph`. Own volume `surreal-search-1-data`. Not the graph volume and not search-0's volume. |

Default `docker compose config --services` prints `postgres`, `hub`, `web` (plus `sidecar` / `hub-b` when those profiles are enabled). Hub and Postgres must not share a container. Profile `graph` (`surreal-graph`, `surreal-search-0`, `surreal-search-1`) is not on that path.

Hub Postgres env (required; no SQLite): `POSTGRES_HOST`, `POSTGRES_USER`, `POSTGRES_PASSWORD` (Compose hub: `postgres` / `venus_hub` / `venus`) and `DATABASE_URL=postgres://venus_hub:venus@postgres:5432/venus?sslmode=disable` (`DATABASE_URL` wins). Service `postgres` still uses superuser `venus` to create that role (`deploy/postgres/ensure-app-role.sh`, also the healthcheck so existing volumes get it). `POSTGRES_SSLMODE` defaults to `disable` (private network); set `require` against managed Postgres.

Host ports bind **`127.0.0.1` only** (`28710`, gRPC `28711`, `28700`, `hub-b` `28712` / gRPC `28713`, sidecar `28720`, SurrealDB `28730`–`28735`). The LAN cannot reach the unauthenticated hub. Vite / Playwright on the same machine are unchanged.

Every Venus host port is in the **`287xx`** block so it does not collide with other local stacks on `3000`, `8000`, `8080`, or `8443`. The block is below the Linux (`32768`+) and macOS (`49152`+) ephemeral ranges. Tens digit is the service: `0x` web, `1x` hub (HTTP even, gRPC odd; hub-b `+2`), `2x` sidecar, `3x` SurrealDB (`28730` + `node_id`). A new service takes the next free tens. Inside a container nginx stays on `80` and SurrealDB on `8000`; only the host publish is in the block.

Hub and postgres have memory / pids limits. Hub drops all capabilities and sets `no-new-privileges`. The hub image does not bake `POSTGRES_PASSWORD` / `DATABASE_URL`.

Pool and timers are **required** on the hub (no sqlx defaults): `HUB_DB_MAX_CONNECTIONS=32`, `HUB_DB_MIN_CONNECTIONS=4`, `HUB_DB_ACQUIRE_TIMEOUT_SECS=10`, `HUB_DB_WORK_MEM=16MB`, `HUB_PERSIST_INTERVAL_MS=1000`, `HUB_COMPACT_AFTER=32`. Missing or blank is a start error that names the var. `32` is sized for 10 distinct cold exports + 10 blob GETs + 10 room flushes overlapping. Two hubs (`hub` + `hub-b`) hold up to 64 of Postgres’s default 100. `HUB_DB_WORK_MEM` is per session per sort/hash, so `16MB` × 64 is the worst case to budget; it exists because a cap-sized flush spills at the 4 MB default ([P5](../design/M3.0/logicals-and-performance.md#p5--cap-sized-flush-spills-at-default-work_mem)).

`HUB_CORS_ORIGINS` is the six localhost Vite/Compose origins so Playwright can preflight `hub:28710`. Set it empty to disable CORS when the browser is same-origin through nginx. The hub does not echo the request `Origin`. Methods are `GET,HEAD,POST,DELETE` (not `any()`).

## Commands

From the repo root. Full stack:

```bash
docker compose up --build
# same as: pnpm compose:up
```

Open **http://127.0.0.1:28700** (the runbook web URL). That is the Compose loop, not Vite `:5173`.

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

Step 4 persist proof (spike `k=v`, wait ≥2s, `restart hub`, then `down` without `-v` and `up`):

```bash
pnpm compose:dod
# apps/web/scripts/m30-compose-dod.mjs
```

That command also asserts `GET /` is `venus-hub` and no `octobase` container.

Second owner + SIGTERM drain (Compose profile `ha`, `hub-b` on `127.0.0.1:28712`):

```bash
pnpm compose:ha
# apps/web/scripts/m30-ha-dod.mjs
```

M1 used volume `pg-data` and database `jwst`. This cutover uses a **new** volume so keck rows are not mistaken for hub tables. Re-seed the M0 wiki. TEXT `workspace_id` / `doc_id` from earlier hub boots are altered to UUID on migrate (`venus-m0` / `doc:home` map to the v5 constants; other slugs fail — `docker compose down -v`).

## Same-origin

Compose `web` bakes `VITE_SYNC_URL=same-origin`. The browser opens `ws://127.0.0.1:28700/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` (and `/api/blobs/…` on the same origin). nginx forwards the `AFFiNE` subprotocol to the hub.

Host Vite still uses `VITE_SYNC_URL=ws://127.0.0.1:28710/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` and proxies `/api` only ([vite.config.ts](../../apps/web/vite.config.ts)). Playwright `pnpm test:e2e:m1` stays on that path (Vite `:5174`).

M3 Flush e2e (`pnpm test:e2e:m3`) needs `docker compose --profile snapshot up` so sidecar `:28720` and `./wiki` are up. Typing-during-convert (`pnpm test:e2e:m3:live`) also needs sidecar `SNAPSHOT_CONVERT_SLEEP_MS=3000` (Compose interpolates `${SNAPSHOT_CONVERT_SLEEP_MS:-0}`; default **0** so ordinary Flush is not delayed):

```bash
mkdir -p wiki && chmod a+rwx wiki
SNAPSHOT_CONVERT_SLEEP_MS=3000 docker compose --profile snapshot up --build sidecar
pnpm test:e2e:m3:live
```

Host binary: `SNAPSHOT_CONVERT_SLEEP_MS=3000 CONVERT_CWD=apps/web WIKI_DIR=wiki cargo run -p venus-sidecar`. Do not bake `3000` into Compose defaults.

Point Playwright at Compose web:

```bash
PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:28700 pnpm --filter @venus/web test:e2e:m1
```

That skips Vite `webServer`. Export curl is still hub `:28710` (api-map **Export command**).

## Layout

```text
docker-compose.yml
deploy/NOTICE                 # hub MIT/Apache; keck history
deploy/hub/Dockerfile         # build rust:1.98.1-bookworm; runtime slim USER venus; no DSN in image
deploy/postgres/ensure-app-role.sh  # venus_hub NOSUPERUSER (initdb + healthcheck)
deploy/web/Dockerfile         # node build + nginx; no OctoBase COPY
deploy/web/nginx.conf
crates/venus-hub/
```

Hub healthcheck is `bash -c 'echo >/dev/tcp/127.0.0.1/28710'`. Slim has bash; do not switch the runtime to distroless without changing that probe.

`.gitignore` does not ignore Compose files. Persist is the named volume `pg-venus-data`, not an anonymous volume.

Kubernetes (later): [kubernetes.md](./kubernetes.md). Hub HPA / gateway (after M3.0): [hub-fleet.md](./hub-fleet.md).
