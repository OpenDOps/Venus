# Sync seam

**Feature:** the editor talks to a `SyncProvider`. Default is memory. Live keck is env-selected. `mount-editor` does not import the server.

**Boxes:** SyncProvider, keck WS ([architecture](../design/architecture.md#elements), [CRDT seam](../design/CRDT/README.md#seam)).

## Run

```bash
pnpm test
pnpm sync:up          # for the Playwright spec
pnpm test:e2e:m1
```

The Vitest env-switch tests **do not** open a socket.

## Vitest

| Spec | Proves |
|---|---|
| `sync-provider.test.ts` — default is memory | `createM0Workspace()` → `kind === 'memory'`; connects before seed |
| `sync-provider.test.ts` — second provider | another `SyncProvider` can be passed; `mount-editor` unused |
| `sync-provider.test.ts` — Seam holds | `mount-editor.js` / `editor-container.js` / `boot.js` do not import live clients |
| `sync-provider.test.ts` — unset env | no `VITE_SYNC_URL` → memory, no `WebSocket` constructed |
| `sync-provider.test.ts` — set env | `VITE_SYNC_URL` selects `octobase` without connecting until `connect` |

## Playwright

| Spec | Needs | Proves |
|---|---|---|
| `e2e/m1-provider.spec.ts` | Compose keck, `pnpm test:e2e:m1` | `kind === 'octobase'`; one WS to `ws://127.0.0.1:3000/collaboration/venus-m0` with subprotocol `AFFiNE` |
