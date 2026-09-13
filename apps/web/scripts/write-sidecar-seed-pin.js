#!/usr/bin/env node
/**
 * Write crates/venus-sidecar/tests/fixtures/seed-home.yjs — MemoryNoopProvider
 * seed pin (Y.encodeStateAsUpdate of seeded doc:home). Hydrate tests load
 * that file without Node. Re-run after seed.js changes:
 *
 *   pnpm --filter @venus/web exec node ./scripts/write-sidecar-seed-pin.js
 */
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = join(here, '..');
const repoRoot = join(webRoot, '../..');
const out = join(repoRoot, 'crates/venus-sidecar/tests/fixtures/seed-home.yjs');

const server = await createServer({
  configFile: join(webRoot, 'vite.config.ts'),
  root: webRoot,
  cacheDir: join(webRoot, 'node_modules/.vite-write-seed-pin'),
  server: { middlewareMode: true },
  appType: 'custom',
  logLevel: 'error',
});

let code = 1;
try {
  const workspace = await server.ssrLoadModule('/src/host/workspace.js');
  const sync = await server.ssrLoadModule('/src/host/sync-provider.js');
  const pinMod = await server.ssrLoadModule('/src/host/mdgate/pin-from-doc.js');
  const session = await workspace.createM0Workspace(
    new sync.MemoryNoopProvider(),
  );
  const { bytes } = pinMod.pinYjsBytes(session.store.spaceDoc);
  if (bytes.byteLength <= 2) {
    throw new Error(`seed pin too small: ${bytes.byteLength} bytes`);
  }
  writeFileSync(out, bytes);
  process.stderr.write(`wrote ${bytes.byteLength} bytes to ${out}\n`);
  code = 0;
} catch (err) {
  process.stderr.write(err instanceof Error ? `${err.stack}\n` : `${err}\n`);
} finally {
  await Promise.race([
    server.close(),
    new Promise((resolve) => {
      setTimeout(resolve, 2000);
    }),
  ]).catch(() => {});
  process.exit(code);
}
