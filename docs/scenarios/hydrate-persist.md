# Hydrate and persist

**Feature:** wait until synced, seed **only if** empty. Memory refresh **drops** text (M0). Sync refresh **keeps** text (M1). No second page root.

**Boxes:** Store, hub, Postgres ([architecture](../design/architecture.md#dataflow-m30), [CRDT share / persist](../design/CRDT/README.md#share-between-clients)).

## Run

```bash
pnpm test
pnpm test:e2e                 # memory: text gone after reload
pnpm sync:up
pnpm test:e2e:m1
```

## Vitest

| Spec | Proves |
|---|---|
| `workspace.test.ts` — hydrate waits for synced | `whenReady()` before seed |
| `workspace.test.ts` — skips seed when page root exists | applying an existing Yjs update does not add a second H1 |
| `workspace.test.ts` — abort during wait | disconnect without a sync-timeout throw |

## Playwright

| Spec | Needs | Proves |
|---|---|---|
| `e2e/m0-provider.spec.ts` | memory Vite | type `hello`, reload → `hello` **absent**; seed back |
| `e2e/m0-smoke.spec.ts` — refresh discards | memory Vite | same M0 contract |
| `e2e/m1-hydrate.spec.ts` — typed hello after refresh | Compose **hub** | type `hello`, reload → `hello` **still there**; title + H1 remain |
| `e2e/m1-hydrate.spec.ts` — second session | Compose **hub** | new context: one `doc-title` Venus, one outline H1 |
| `e2e/m1-smoke.spec.ts` | Compose **hub** | kind `octobase`; outline H1; type `hello`; reload **keeps** it |

Postgres persist across `docker compose restart hub` is a hub persist test (wait ≥2s after a write), not a Playwright file. Operator notes: [runbook Sync](../runbook.md#sync-hub).
