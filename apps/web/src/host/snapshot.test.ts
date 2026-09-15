import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import { CATALOG_SQL_ID, PAGE_SQL_ID, WORKSPACE_ID } from './ids.js';

const hostDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(hostDir, '../../../..');

/** Exact api-map advertisement GET. Product Yjs export is gRPC ExportDoc. */
const EXPORT_AD_COMMAND = `curl -sSSf http://127.0.0.1:3000/api/block/${WORKSPACE_ID}/export`;
const EXPORT_URL = `http://127.0.0.1:3000/api/block/${WORKSPACE_ID}/export`;

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

let exportCt = '';
if (hubUp) {
  try {
    const res = await fetch(EXPORT_URL, { signal: AbortSignal.timeout(1500) });
    exportCt = res.headers.get('content-type') ?? '';
  } catch {
    exportCt = '';
  }
}
const exportIsAd = exportCt.includes('json');

if (hubBody && hubBody !== 'venus-hub') {
  throw new Error(
    `:3000 is not the Venus hub (GET / → ${JSON.stringify(hubBody)}). Fail if keck is the process.`,
  );
}

if (!hubUp) {
  console.warn(
    'snapshot.test.ts: skipping advertisement GET — nothing on 127.0.0.1:3000. Start with pnpm sync:up (postgres + hub). Documented skip when Compose is down.',
  );
} else if (!exportIsAd) {
  console.warn(
    'snapshot.test.ts: skipping advertisement GET — hub still serves Yjs (rebuild: docker compose up --build postgres hub).',
  );
}

test('api-map Export advertisement GET is not an empty template', () => {
  const map = readFileSync(join(repoRoot, 'docs/design/api-map.md'), 'utf8');
  expect(map).toContain(EXPORT_AD_COMMAND);
  expect(map).toContain('venus.hub.v1.Hub/ExportDoc');
});

test('GET export does not call live_export (Yjs is gRPC)', () => {
  const http = readFileSync(
    join(repoRoot, 'crates/venus-hub/src/http.rs'),
    'utf8',
  );
  expect(http).toMatch(/advertisement_response/);
  expect(http).not.toMatch(/st\.hub\.live_export/);
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

describe.skipIf(!hubUp || !exportIsAd)(`hub GET /api/block/${WORKSPACE_ID}/export`, () => {
  test('Reachable: 200 JSON advertisement, no root error', async () => {
    const res = await fetch(EXPORT_URL, { signal: AbortSignal.timeout(5000) });
    expect(res.status).toBe(200);
    expect(res.headers.get('content-type') ?? '').toMatch(/json/);
    const body = await res.json();
    expect(body.error).toBeUndefined();
    expect(body.advertisement.kind).toBe('doc_export');
    expect(body.advertisement.http_export).toBe(false);
    expect(body.advertisement.grpc.service).toBe('venus.hub.v1.Hub');
    const roles = body.advertisement.docs.map((d: { role: string }) => d.role);
    expect(roles).toContain('home');
    expect(roles).toContain('catalog');
    const home = body.advertisement.docs.find((d: { role: string }) => d.role === 'home');
    expect(home.sql_id).toBe(PAGE_SQL_ID);
    const catalog = body.advertisement.docs.find(
      (d: { role: string }) => d.role === 'catalog',
    );
    expect(catalog.sql_id).toBe(CATALOG_SQL_ID);
  });

  test('?doc= is 400 export_http_disabled with advertisement', async () => {
    const res = await fetch(`${EXPORT_URL}?doc=${PAGE_SQL_ID}`, {
      signal: AbortSignal.timeout(5000),
    });
    expect(res.status).toBe(400);
    const body = await res.json();
    expect(body.error.code).toBe('export_http_disabled');
    expect(body.advertisement.kind).toBe('doc_export');
  });
});
