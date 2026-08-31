# Kubernetes

**Not in M1.** No manifests in this repo yet. When they land, they should be the **same three processes** as [compose.md](./compose.md), not a fourth database or a merged collab-front + Postgres pod. After [M3.0](../design/M3.0/README.md) the collab Deployment is **hub**, not keck. Route WS on **`workspace_id`** (not cookie sticky): [M3.0 HA](../design/M3.0/high-availability.md).

| Compose | Cluster (intent) |
|---|---|
| `postgres` + volume `pg-data` | StatefulSet or a managed Postgres. Same DSN shape (`DATABASE_URL`). |
| `octobase` (M1 keck) / **`hub` (M3.0+)** | Deployment. ClusterIP Service. Hub is Venus-owned. Do not put keck in the web image. |
| `web` (nginx + static) | Deployment. Ingress / TLS to this Service. Keep same-origin `/api` and `/collaboration` to the collab Service. |

`VITE_SYNC_URL=same-origin` is already baked for that Ingress host. Rebuild the web image only when the host app changes; do not bake a cluster hostname.

Do not start this folder’s YAML until M1 is closed and someone owns a cluster. Operators on a laptop stay on Compose.
