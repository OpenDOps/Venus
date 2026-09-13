# Doc export

**Feature:** a non-browser client reads the **current** Y.Doc from the hub. Not markdown, not `T0`. [glossary](../design/glossary.md). [CRDT doc export](../design/CRDT/README.md#doc-export-m1-step-7).

**Boxes:** curl → hub export ([architecture](../design/architecture.md#elements), [CRDT doc export](../design/CRDT/README.md#doc-export-m1-step-7)).

## Run

```bash
pnpm test
pnpm sync:up          # then hydrate once (app or pnpm test:e2e:m1)
pnpm test             # Reachable / Decodes run when hub is on :3000
```

Exact command: api-map **Export command**. [runbook](../runbook.md#sync-m1).

## Vitest

`snapshot.test.ts` shells the api-map command (`execFileSync` of that `curl`). Decode is Node `Y.applyUpdate`. There is no `crates/venus-sidecar` and no editor export button.

| Spec | Proves |
|---|---|
| `snapshot.test.ts` — Export command string | `docs/design/api-map.md` still contains the exact `curl …/export` (fails if the cell is an empty template) |
| `snapshot.test.ts` — not in the editor UI | `mount-editor.js` / `editor-container.js` / `boot.js` / `App.tsx` do not call `/api/block/…/export` |
| `snapshot.test.ts` — Reachable | `curl -sSSf …/export -o /tmp/venus-page.yjs` exit 0; file length **> 2** bytes |
| `snapshot.test.ts` — Decodes | `Y.applyUpdate(new Y.Doc(), bytes)` does not throw; `Y.encodeStateAsUpdate` length **> 2** |

Reachable / Decodes **skip** when nothing listens on `127.0.0.1:3000` so `pnpm test` stays Docker-free. If the hub is up and export is empty or HTTP-fails, they **fail**. Local DoD: Compose up, `doc:home` hydrated at least once, those tests pass (not skipped).
