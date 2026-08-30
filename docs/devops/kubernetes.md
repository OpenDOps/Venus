# Kubernetes

**Not in M1.** No manifests in this repo yet. When they land, they should be the **same three processes** as [compose.md](./compose.md), not a fourth database or a merged keck+Postgres pod.

| Compose | Cluster (intent) |
|---|---|
| `postgres` + volume `pg-data` | StatefulSet or a managed Postgres. Same DSN shape (`DATABASE_URL`). |
| `octobase` (keck image) | Deployment. ClusterIP Service. Still AGPL; not in the web image. |
| `web` (nginx + static) | Deployment. Ingress / TLS to this Service. Keep same-origin `/api` and `/collaboration` to the keck Service (do not teach the browser a second origin unless CORS on keck is expanded). |

`VITE_SYNC_URL=same-origin` is already baked for that Ingress host. Rebuild the web image only when the host app changes; do not bake a cluster hostname.

Do not start this folder’s YAML until M1 is closed and someone owns a cluster. Operators on a laptop stay on Compose.
