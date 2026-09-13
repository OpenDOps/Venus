# Kubernetes

**Not in M1.** No manifests in this repo yet. When they land, they should be the **same three processes** as [compose.md](./compose.md), not a fourth database or a merged collab-front + Postgres pod. After [M3.0](../design/M3.0/README.md) the collab Deployment is **hub** (**Rust + y-octo**), not keck. Route WS on **`workspace_id`** (wiki sticky / lease — not cookie, not `doc_id`): [M3.0 HA](../design/M3.0/high-availability.md). Many hub replicas, stateless gateways, HPA, and drain-then-claim: [hub-fleet.md](./hub-fleet.md) (**after** M3.0). Do not treat ClusterIP round-robin as wiki sticky.

| Compose | Cluster (intent) |
|---|---|
| `postgres` + volume `pg-venus-data` | StatefulSet or a managed Postgres. Same DSN shape (`POSTGRES_*` or `DATABASE_URL`). Against managed Postgres set `POSTGRES_SSLMODE=require` (Compose default is `disable`). Hub pool knobs (`HUB_DB_MAX_CONNECTIONS` / `MIN` / `ACQUIRE_TIMEOUT_SECS` / `WORK_MEM`, persist/compact) are required; copy Compose values or size against the cluster `max_connections`. `HUB_DB_WORK_MEM` is per session, so budget it against replicas × pool size, not per pod. Same-origin Ingress: `HUB_CORS_ORIGINS` empty. |
| **`hub`** | Deployment. **One replica:** ClusterIP is enough. **Fleet:** gateway in [hub-fleet.md](./hub-fleet.md), not ClusterIP RR. Hub is Venus-owned ([hub](../design/components/hub/)). Do not put keck in the web image. |
| `web` (nginx + static) | Deployment. Ingress / TLS to this Service. Keep same-origin `/api` and `/collaboration` to the collab Service. |

`VITE_SYNC_URL=same-origin` is already baked for that Ingress host. Rebuild the web image only when the host app changes; do not bake a cluster hostname.

Do not start this folder’s YAML until M1 is closed and someone owns a cluster. Operators on a laptop stay on Compose.
