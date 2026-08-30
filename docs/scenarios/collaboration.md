# Collaboration

**Feature:** two tabs share one Y.Doc over the keck socket. Typing appears without reload. Not BroadcastChannel.

**Boxes:** Tab A, Tab B, SyncProvider, keck ([architecture](../design/architecture.md#dataflow-m0m1-with-m2-dashed), [CRDT share](../design/CRDT/README.md#share-between-clients)).

## Run

```bash
pnpm sync:up
pnpm test:e2e:m1
# or against Compose web:
# PLAYWRIGHT_M1=1 PLAYWRIGHT_BASE_URL=http://127.0.0.1:8080 pnpm --filter @venus/web exec playwright test e2e/m1-two-tabs.spec.ts
```

Open tab B **after** A has the seed H1 (DoD: no double page).

## Playwright

| Spec | Proves |
|---|---|
| `e2e/m1-two-tabs.spec.ts` — A typing appears in B | A types `from-a-…`; B sees it within 10s without reload |
| `e2e/m1-two-tabs.spec.ts` — both tabs same seed once | both titles `Venus`, one H1 `Why Venus` each |
| `e2e/m1-two-tabs.spec.ts` — B typing appears in A | B types `from-b-…`; A sees it without reload |

Person-in-browser two windows is [M1 step 9](../design/M1/plan.md#9-step-verify), not a substitute for this spec.
