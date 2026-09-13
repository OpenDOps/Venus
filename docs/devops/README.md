# DevOps

How Venus **runs**: containers, ports, volumes, later cluster deploy. Product rules stay in [design](../design/README.md). Copy-paste commands stay in the [runbook](../runbook.md). Tests of this stack: [scenarios/compose](../scenarios/compose.md).

| File | Status |
|---|---|
| [compose.md](./compose.md) | **M3.0:** `postgres` + `hub` + `web`. |
| [kubernetes.md](./kubernetes.md) | **Not in M1.** Same three processes behind Ingress. ClusterIP RR is one hub replica only. |
| [hub-fleet.md](./hub-fleet.md) | **Later track** (after M3.0): stateless gateway, hub HPA, shed-then-claim session move. |

Do not put keck REST (blobs, export) here — that is [api-map](../design/api-map.md). Do not split [architecture](../design/architecture.md) into one page per container.
