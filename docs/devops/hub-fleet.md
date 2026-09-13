# Hub fleet — gateway, HPA, session drain

**Status:** later **devops track**. Not M3.0, not M3. **Gate:** [M3.0](../design/M3.0/README.md) **closed** (lease refuse + SIGTERM flush/drop proven on a second process). Owner/dirty/drain mutex stays in [M3.0 HA](../design/M3.0/high-availability.md). Snapshotters stay in [LiveSnapshot HA](../design/LiveSnapshot/high-availability.md) (`SKIP LOCKED`, not this gateway).

M3.0 Compose is one `hub` behind `web` nginx. This file is the scale contract so that thin loop does not paint **ClusterIP round-robin** or **`hash(workspace_id) % replicaCount`** into the product.

**Implement** only after M3.0. Laptop operators stay on Compose.

## What this track is

1. **Stateless balancing proxies** (gateway) in front of many hub pods. Route on `workspace_id`. Replicated, no session table.
2. **Hub autoscaling** (HPA up and down). Scale-out is **many wikis**, not a 51st tab on one wiki.
3. **Smooth owner change** when a wiki must leave a pod (downscale, rolling deploy, optional rebalance).

It is **not** a Venus membership/gossip plane. Hubs do not advertise replica count to proxies. Kubernetes Endpoints + `workspace_lease` (and optional **fixed** partition rows) are the maps. Do not build a watcher that both scales pods and pushes N into LBs.

```text
clients ──► ingress / web
                 │
                 ▼
            gateway × N   (stateless; parse workspace_id)
                 │  lease lookup or fixed partition → pod
                 ▼
            hub owner     (RAM apply + broadcast + persist)
                 │
                 ▼
            Postgres      workspace_lease is the mutex
```

## Three maps (do not collapse them)

| Map | Source of truth | Changes when |
|---|---|---|
| **Ready hubs** | k8s Endpoints from `/readyz` | Probe fail, SIGTERM, scale |
| **Live owner** | `workspace_lease` | Claim, TTL expiry, drain drop |
| **Hash / partition** | Optional **hint** (fixed P, not `replicaCount`) | Rebalance, never HPA N |

A proxy may hash as a hint. It must not apply on a wiki unless it holds the lease. Two applying owners is still split-brain ([M3.0 HA](../design/M3.0/high-availability.md)).

Vanilla **ClusterIP** is round-robin. Fine for **one** hub replica. Wrong as the only balancer once `replicaCount > 1`.

## Stateless gateway

| Rule | |
|---|---|
| **Replicas** | ≥2. Any proxy, same answer. |
| **State** | None. Short cache of lease → pod is allowed (TTL seconds, invalidate on refuse). |
| **Key** | Path `/collaboration/:workspace_id` (blob/export prefix too if those stay owner-sticky). Not cookie, not IP, not `doc_id`. |
| **Lookup** | Read `workspace_lease` (or partition owner). If none: send to any ready hub; it claims. If claim fails: retry lookup. |
| **Backstop** | Hub **refuses** (or redirects) when it does not hold the lease. Proxy / client reconnects to the owner. This is M3.0 lease, used by the fleet. |
| **Do not** | Sticky sessions. Hub→proxy gossip of N. Redis as a second owner map. Hash `% replicaCount`. |

Ingress TLS can sit in front. The wiki-sticky logic is this gateway (Envoy/nginx/Lua/a tiny Rust proxy — Actual later). `web` static nginx is not required to grow a hash ring in M3.0.

## Autoscaling

HPA (or manual replica count) on the **hub** Deployment. Metrics: CPU, WS count, `leases_held` per pod. **Ready** = Postgres reachable and not draining.

| Event | Placement |
|---|---|
| **Upscale** | New pod starts empty. It takes **new** wikis and **expired** leases. It does **not** steal live leases. No session move on this path. |
| **Downscale / rolling** | Pod must **drain** (below) before exit. PDB so two owners are not drained at once beyond what the cluster can absorb. |
| **Rebalance** (optional) | Move some wikis off a hot pod. Same drain protocol. Prefer **idle** wikis first. |

Do not treat HPA replica count as the shard function. Adding a pod must not remap every `workspace_id`.

Observer/worker replica counts stay [LiveSnapshot HA](../design/LiveSnapshot/high-availability.md#snapshotter-fleet-competing-consumers-not-hash-shards): not this gateway, not wiki sticky.

## Memory budget as replicas grow

`work_mem` ([P5](../design/M3.0/logicals-and-performance.md#p5--cap-sized-flush-spills-at-default-work_mem)) is **per backend per node**, not per workspace. Thousands of wikis do not multiply it — flushes funnel through a bounded pool. What multiplies it is **replicas**:

```text
Postgres worst case = replicas × HUB_DB_MAX_CONNECTIONS × HUB_DB_WORK_MEM
Compose today       = 2 × 32 × 16MB = 1 GB   (postgres mem_limit is 1g)
```

It is a ceiling taken lazily per sort/hash node and released at statement end, not a reservation, so steady state is a fraction of that — but the worst case already equals the whole container, and **each added hub replica books another ~512 MB of it**. Size `HUB_DB_WORK_MEM` against `replicas × pool`, not per pod, and keep the fleet's total connections under the server `max_connections` (image default 100).

The **larger** budget at thousands of wikis is the hub's own RAM, not Postgres: one live `Y.Doc` per room plus a persist buffer capped at `PERSIST_BYTES` (8 MiB) **per room**. A thousand backed-up rooms is 8 GB against `mem_limit: 1g`. Room eviction and that cap are the levers there; `work_mem` is a rounding error beside it. Do not tune `work_mem` to fix a hub OOM. Process-level caps vs working set: [hub architecture — Memory](../design/components/hub/architecture.md#memory-caps-not-working-set).

Two ways to make the allowance shared rather than multiplied, if the Postgres side ever binds. Neither is built — measure first:

1. **Split the pool.** The 32 connections are sized for cold exports and blob GETs; only flushes need the memory. A 4–8 connection writer pool at `16MB` with everything else at the server default takes a hub from 512 MB to ~176 MB of worst case.
2. **Statement-scoped.** `BEGIN; SET LOCAL work_mem …; INSERT …; COMMIT` on batches above a threshold returns the allowance at commit instead of parking it on every idle session. Costs two round trips, so keep typing-sized flushes on the single-statement path. It cannot be folded into one statement: `SET` takes no parameters, and a CTE calling `set_config` has no guaranteed evaluation order against the insert.

**Pooler caveat:** with PgBouncer in transaction pooling, a session-level `SET work_mem` is wrong — sessions no longer map to backends, so it is lost or leaks to unrelated clients. In that topology option 2 is the only correct mechanism, and `connect_with`'s `after_connect` must go.

## Session movement (the actual problem)

Hub RAM is not shared. You **cannot** live-migrate a `Y.Doc`. “Move a session” means: old owner **sheds**, clients **reconnect**, new owner **hydrates from SQL**. Same loss bound as crash failover (~1s unflushed) if drain skips flush; with a proper drain, persist is flushed first.

**Forbidden:** hash ring (or partition map) **splits first** while old WebSockets still apply on the previous pod, and new sockets apply on the next. That is two owners.

**Locked:** **shed-then-claim**, with a **wait** so the new placement is not live until the old lease is gone.

```text
1. Compute moved set M (wikis that should leave this pod).
   Gateway still routes M to the CURRENT lease owner.
   New hash / new partition row is not live for M yet.

2. Optional idle grace (upscale rebalance only):
   skip wikis with recent WS activity; try again later.
   Pod deletion / SIGTERM skips grace — drain now.

3. SHED (old owner), per wiki in M:
   refuse new sockets (or accept and do not apply — prefer refuse)
   flush persist
   close remaining WS
   drop RAM doc
   DROP workspace_lease

4. WAIT until lease row is gone (or TTL). New owner must not claim early.

5. Flip placement (partition owner / gateway cache) for M.

6. CLAIM (new owner): take lease, hydrate from SQL, accept WS.

7. Clients reconnect (y-protocols). Gateway sends them to the new owner.
```

| Approach | Use? |
|---|---|
| Split the hash, then migrate while both apply | **No.** Split-brain. |
| Wait / drain, then flip the map, then claim | **Yes.** |
| Replicate hub RAM / hand off the TCP socket | **No.** |
| Upscale steals half the wikis because N changed | **No.** Upscale does not remap. |

Refuse-on-wrong-lease (already M3.0) is the safety net if a proxy is briefly stale. It is not a license to run two apply loops.

**Downscale PreStop** is this shed for **every** wiki on that pod, then exit. Same as [M3.0 HA drain](../design/M3.0/high-availability.md) (flush then drop lease), plus close sockets so clients do not sit on a dying process.

## Plan (when this track is scheduled)

Do not start these steps while M3.0 is open. No board yaml until the track is staffed.

| # | id | Adds |
|---|---|---|
| 1 | `step-gateway` | Stateless proxy: parse `workspace_id`, lease lookup, ≥2 replicas, hub refuse still wins. Prove two tabs on one wiki hit one owner through two proxies. |
| 2 | `step-readyz` | Hub `/readyz` + unready while draining. PreStop = flush, close WS, drop leases. Endpoints drop the pod. |
| 3 | `step-upscale` | 1→2 hub pods: existing wiki **stays**; a second wiki may land on the new pod. No dual apply. Not `hash % 2`. |
| 4 | `step-shed-claim` | Forced move (drain one wiki or downscale one pod): shed-then-claim; clients reconnect; hydrate from SQL; never two apply owners. Idle grace optional and **off** for PreStop. |
| 5 | `step-hpa` | HPA from CPU / WS / `leases_held`. Scale-down uses PreStop drain. Replica count is not the ring. |

**Exit:** many hub pods, stateless proxies, HPA up and down, wiki owner changes only through shed-then-claim. Compose laptop path unchanged (one hub, no gateway mesh).

## Acceptance

| # | Locked |
|---|---|
| G1 | Owner of record is `workspace_lease`. Proxies are stateless. Hubs do not advertise N. |
| G2 | ClusterIP RR and `hash % replicaCount` are not the fleet balancer. |
| G3 | Upscale does not steal live leases. Downscale / rebalance **sheds then claims**. |
| G4 | No window where two hubs apply the same `workspace_id`. Wait until the old lease is dropped. |
| G5 | Session move is reconnect + SQL hydrate, not RAM migrate. Drain flushes first. |
| G6 | Snapshotter fleets are not behind this gateway. M3.0 one-replica Compose is not blocked on this track. |

## Do not

- Live-migrate Y.Doc or replicate apply to the destination pod.
- Flip the hash ring for a wiki before the old owner has dropped the lease.
- Use HPA `replicaCount` as P in `hash % P`.
- Hub gossip / custom EDS that hubs push to proxies.
- Cookie/IP/`doc_id` sticky.
- Set `work_mem` on the `venus_hub` role or in `postgresql.conf` — that hands the same allowance to snapshotters, observers, exports and admin sessions, so every fleet multiplies the budget.
- Start this track (or k8s YAML for a hub ring) while M3.0 is open.

## Files

| File | Role |
|---|---|
| [M3.0 HA](../design/M3.0/high-availability.md) | Mutex: one owner, persist, dirty, SIGTERM drain |
| [hub-fleet.md](./hub-fleet.md) | This track: gateway, HPA, shed-then-claim |
| [kubernetes.md](./kubernetes.md) | Cluster process map (when manifests exist) |
| [LiveSnapshot HA](../design/LiveSnapshot/high-availability.md) | Observer / worker scale (not wiki sticky) |
