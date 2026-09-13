import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, test } from 'vitest';
import * as Y from 'yjs';

import { WORKSPACE_ID } from './ids.js';

const hostDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(hostDir, '../../../..');

/** Exact api-map Export command. Do not invent a second store or URL. */
const EXPORT_COMMAND =
  `curl -sSSf http://127.0.0.1:3000/api/block/${WORKSPACE_ID}/export -o /tmp/venus-page.yjs`;
const EXPORT_URL = `http://127.0.0.1:3000/api/block/${WORKSPACE_ID}/export`;
const EXPORT_FILE = '/tmp/venus-page.yjs';
const CURL_ARGV = ['-sSSf', EXPORT_URL, '-o', EXPORT_FILE];

async function hubRootBody(): Promise<string | null> {
  try {
    const res = await fetch('http://127.0.0.1:3000/', {
      signal: AbortSignal.timeout(1500),
    });
    return (await res.text()).trim();
  } catch {
    return null;
  }
}

const hubBody = await hubRootBody();
const hubUp = hubBody === 'venus-hub';

if (hubBody && hubBody !== 'venus-hub') {
  throw new Error(
    `:3000 is not the Venus hub (GET / → ${JSON.stringify(hubBody)}). Fail if keck is the process.`,
  );
}

if (!hubUp) {
  console.warn(
    'snapshot.test.ts: skipping Reachable/Decodes — nothing on 127.0.0.1:3000. Start with pnpm sync:up (postgres + hub). Documented skip when Compose is down.',
  );
}

test('api-map Export command is the hub GET (not an empty template)', () => {
  const map = readFileSync(join(repoRoot, 'docs/design/api-map.md'), 'utf8');
  expect(map).toContain(EXPORT_COMMAND);
  expect(`curl ${CURL_ARGV.join(' ')}`).toBe(EXPORT_COMMAND);
});

test('export is hub live_export, not keck Block REST', () => {
  const http = readFileSync(
    join(repoRoot, 'crates/venus-hub/src/http.rs'),
    'utf8',
  );
  expect(http).toMatch(/st\.hub\.live_export/);
  expect(http).not.toMatch(/jwst/i);
  const cargo = readFileSync(join(repoRoot, 'crates/venus-hub/Cargo.toml'), 'utf8');
  expect(cargo).not.toMatch(/jwst|octobase|keck/);
});

test('export is not wired into the editor UI', () => {
  const files = ['mount-editor.js', 'editor-container.js', 'boot.js'];
  for (const name of files) {
    const src = readFileSync(join(hostDir, name), 'utf8');
    expect(src, name).not.toMatch(/\/api\/block\/.+\/export/);
  }
  const app = readFileSync(join(hostDir, '../App.tsx'), 'utf8');
  expect(app).not.toMatch(/\/api\/block\/.+\/export/);
});

describe.skipIf(!hubUp)(`hub GET /api/block/${WORKSPACE_ID}/export`, () => {
  beforeAll(() => {
    execFileSync('curl', CURL_ARGV, { stdio: 'pipe' });
  });

  test('Reachable: Export command exit 0 and file length > 2', () => {
    const bytes = readFileSync(EXPORT_FILE);
    expect(bytes.byteLength).toBeGreaterThan(2);
  });

  test('Decodes: Y.applyUpdate does not throw and re-encode is > 2 bytes', () => {
    const bytes = new Uint8Array(readFileSync(EXPORT_FILE));
    const d = new Y.Doc();
    expect(() => Y.applyUpdate(d, bytes)).not.toThrow();
    expect(Y.encodeStateAsUpdate(d).byteLength).toBeGreaterThan(2);
  });
});
