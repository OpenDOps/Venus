#!/usr/bin/env node
/**
 * Path B convert CLI for the snapshotter. Rust sidecar (step 2+) spawns this.
 * BlockSuite publishes `.ts`, so plain `node pin-from-doc.js` cannot load the
 * adapter. This file boots Vite `ssrLoadModule` with the web config (same
 * graph as Vitest), then `fromPinnedBytes`. It does not git-commit and must
 * not import the spectator pane or RAM splice exporter.
 *
 * Usage (cwd anywhere):
 *   VENUS_BLOB_ORIGIN=http://127.0.0.1:3000 node from-pinned-cli.js /tmp/venus-page.yjs
 *   node from-pinned-cli.js -   # stdin
 * stdout: JSON `{ markdown, sidecar }`. Vite noise stays off stdout.
 */
import { readFileSync, writeSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = join(here, '../../..');

function writeStdout(s) {
  const buf = Buffer.from(s);
  let offset = 0;
  while (offset < buf.length) {
    try {
      offset += writeSync(1, buf.subarray(offset));
    } catch (err) {
      if (err && (err.code === 'EAGAIN' || err.code === 'EWOULDBLOCK')) {
        continue;
      }
      throw err;
    }
  }
}

function usage() {
  process.stderr.write(
    'usage: node from-pinned-cli.js <update-v1.bin|->\n',
  );
}

const input = process.argv[2];
if (!input) {
  usage();
  process.exit(2);
}

const bytes = new Uint8Array(
  input === '-' ? readFileSync(0) : readFileSync(input),
);

const blobOrigin = (process.env.VENUS_BLOB_ORIGIN ?? '').trim();

const server = await createServer({
  configFile: join(webRoot, 'vite.config.ts'),
  root: webRoot,
  cacheDir: join(tmpdir(), 'venus-from-pinned-vite'),
  server: { middlewareMode: true },
  appType: 'custom',
  logLevel: 'error',
});

let code = 1;
try {
  const pin = await server.ssrLoadModule(
    '/src/host/mdgate/pin-from-doc.js',
  );
  /** @type {{ main: unknown } | undefined} */
  let blobSources;
  if (blobOrigin) {
    const blob = await server.ssrLoadModule(
      '/src/host/providers/blob-source.js',
    );
    blobSources = {
      main: new blob.OctoBaseBlobSource({ origin: blobOrigin }),
    };
  }
  const out = await pin.fromPinnedBytes(bytes, { blobSources });
  // Sync loop: stdout.write + process.exit truncates ~64KiB; writeFileSync(1)
  // throws EAGAIN when the sidecar pipes stdout.
  writeStdout(JSON.stringify(out));
  code = 0;
} catch (err) {
  process.stderr.write(err instanceof Error ? `${err.stack}\n` : `${err}\n`);
  code = 1;
} finally {
  await Promise.race([
    server.close(),
    new Promise((resolve) => {
      setTimeout(resolve, 2000);
    }),
  ]).catch(() => {});
  process.exit(code);
}
