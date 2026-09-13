# Blobs

**Feature:** image bytes go through hub HTTP into Postgres. Second tab and reload show pixels, not a dead `blob:` URL.

**Boxes:** Blob HTTP, hub, Postgres ([architecture](../design/architecture.md#dataflow-m30), [CRDT persist](../design/CRDT/README.md#persist)).

## Run

```bash
pnpm test
pnpm sync:up
pnpm test:e2e:m1
```

Fixture: `apps/web/e2e/fixtures/dot.png`.

## Vitest

| Spec | Proves |
|---|---|
| `providers/blob-source.test.ts` — origin | `ws://` → `http://127.0.0.1:3000` |
| `blob-source.test.ts` — env unset | no `VITE_SYNC_URL` → no HTTP blob source |
| `blob-source.test.ts` — env set | `OctoBaseBlobSource` as `blobSources.main` |
| `blob-source.test.ts` — POST/GET | mocked `fetch`: set then get; `list` is `[]` |
| `workspace.test.ts` — blobSources.main | `blobSync.set` hits `blobSources.main` |

## Playwright (`m1-blob.spec.ts`, serial)

| Spec | Proves |
|---|---|
| upload posts the PNG and shows pixels | slash Image; `POST /api/blobs/77e4a2b1-8b40-5979-a73c-fd4477216d00` 2xx; `naturalWidth > 0` |
| second tab sees the image without picking a file | B has pixels without choosing a file |
| reload keeps the image pixels | after reload, image + seed title/H1 |

keck (M1) had no blob list route; `list` returning `[]` is still expected on the hub.
