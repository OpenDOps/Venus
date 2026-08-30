# Compose

M1 product runtime: **three services**, one file, [docker-compose.yml](../../docker-compose.yml) at the repo root. Postgres is the only database. keck is AGPL (`deploy/NOTICE`). The web image does **not** contain OctoBase source.

## Services

| Service | Image | Host port | Role |
|---|---|---|---|
| `postgres` | `postgres:16` | none | Docs + blob bytes. Volume `pg-data`. |
| `octobase` | `deploy/octobase/Dockerfile` (keck pin in api-map) | `3000` | Yjs WS `/collaboration/venus-m0`, blob HTTP, doc export. |
| `web` | `deploy/web/Dockerfile` (nginx + `apps/web/dist`) | `8080` | Host UI. Same-origin proxy: `/api` and `/collaboration` → `octobase:3000`. |

`docker compose config --services` must print exactly those three names. keck and Postgres must not share a container.

## Commands

From the repo root. Full stack (step 8 loop, exit demo):

```bash
docker compose up --build
# same as: pnpm compose:up
```

Open **http://127.0.0.1:8080** (the runbook web URL). That is the Compose loop, not Vite `:5173`.

Playwright and `pnpm dev` + `.env` only need keck:

```bash
docker compose up --build postgres octobase
# same as: pnpm sync:up
```

Stop (keeps `pg-data`):

```bash
docker compose down
# pnpm compose:down / pnpm sync:down
```

`docker compose down -v` **deletes** `pg-data`. Do not use `-v` if the page must survive.

## Same-origin

Compose `web` bakes `VITE_SYNC_URL=same-origin`. The browser opens `ws://127.0.0.1:8080/collaboration/venus-m0` (and `/api/blobs/…` on the same origin). nginx forwards the `AFFiNE` subprotocol to keck.

Host Vite still uses `VITE_SYNC_URL=ws://127.0.0.1:3000/collaboration/venus-m0` and proxies `/api` only ([vite.config.ts](../../apps/web/vite.config.ts)). Playwright `pnpm test:e2e:m1` stays on that path (Vite `:5174`).

Point Playwright at Compose web (step 8 **Loop from Compose**):

```bash
PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080 pnpm --filter @venus/web test:e2e:m1
```

That skips Vite `webServer`. Export curl is still keck `:3000` (api-map **Export command**).

## Layout

```text
docker-compose.yml
deploy/NOTICE                 # keck AGPL
deploy/octobase/Dockerfile
deploy/web/Dockerfile         # node build + nginx; no OctoBase COPY
deploy/web/nginx.conf
```

`.gitignore` does not ignore Compose files. Persist is the named volume `pg-data`, not an anonymous volume.

Kubernetes (later): [kubernetes.md](./kubernetes.md).
