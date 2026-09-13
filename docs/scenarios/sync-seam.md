# Sync seam

**Feature:** the editor talks to a `SyncProvider`. Default is memory. Live hub is env-selected (`kind: 'octobase'` alias). `mount-editor` does not import the server.

**Boxes:** SyncProvider, hub WS ([architecture](../design/architecture.md#elements), [CRDT seam](../design/CRDT/README.md#seam)).

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
| `e2e/m1-provider.spec.ts` | Compose hub, `pnpm test:e2e:m1` | `kind === 'octobase'` (alias); one WS to `ws://127.0.0.1:3000/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00` with subprotocol `AFFiNE` |
