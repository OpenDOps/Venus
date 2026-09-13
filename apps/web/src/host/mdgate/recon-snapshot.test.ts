import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, test } from 'vitest';

import { PAGE_DOC_ID, WORKSPACE_ID } from '../ids.js';
import { SEED_H1 } from '../seed.js';
import { MemoryNoopProvider } from '../sync-provider.js';
import { createM0Workspace } from '../workspace.js';
import { fromPinnedBytes, pinYjsBytes } from './pin-from-doc.js';

const here = dirname(fileURLToPath(import.meta.url));
const webRoot = join(here, '../../..');
const repoRoot = join(webRoot, '../..');

const EXPORT_COMMAND = `curl -sSSf http://127.0.0.1:3000/api/block/${WORKSPACE_ID}/export -o /tmp/venus-page.yjs`;
const EXPORT_URL = `http://127.0.0.1:3000/api/block/${WORKSPACE_ID}/export`;
const CONVERT_CLI = join(here, 'from-pinned-cli.js');
const BLOB_ORIGIN = 'http://127.0.0.1:3000';

function snapshotterSection(map: string): string {
  const start = map.indexOf('## Names — git snapshotter');
  expect(start).toBeGreaterThan(-1);
  const rest = map.slice(start);
  const end = rest.indexOf('\n## ', 10);
  return end === -1 ? rest : rest.slice(0, end);
}

function actualCells(section: string): string[] {
  const rows = section
    .split('\n')
    .filter((line) => line.startsWith('|') && !line.startsWith('|---') && !line.startsWith('| Design name'));
  return rows.map((line) => {
    const cols = line.split('|').map((c) => c.trim());
    // | Design | Likely | Actual | Notes |
    return cols[3] ?? '';
  });
}

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
    'recon-snapshot.test.ts: skipping hub export spike — nothing on 127.0.0.1:3000. Start with pnpm sync:up.',
  );
}

test('Gate held: M3.0 steps 1–9 done; LiveSnapshot HA accepted with a date', () => {
  const board = readFileSync(
    join(repoRoot, 'docs/design/M3.0/M3.0.state.yaml'),
    'utf8',
  );
  for (const id of [
    'step-recon-hub',
    'step-store',
    'step-ws',
    'step-compose-hub',
    'step-provider',
    'step-parity',
    'step-ha-owner',
    'step-dirty',
    'step-verify',
  ]) {
    const m = board.match(new RegExp(`${id}:[\\s\\S]*?state: (\\w+)`));
    expect(m?.[1], id).toBe('done');
  }
  expect(board).not.toMatch(/step-verify:[\s\S]*?state: pending/);

  const ha = readFileSync(
    join(repoRoot, 'docs/design/LiveSnapshot/high-availability.md'),
    'utf8',
  );
  expect(ha, 'HA must not still say un-accepted').not.toMatch(
    /\*\*re-accept before M3 code\.\*\*/,
  );
  expect(ha).toMatch(/Accepted 2026-09-13/);
  expect(ha).toMatch(/#acceptance-gate-for-m3|# Acceptance \(gate for M3\)/);
});

test('Map complete: Names — git snapshotter Actuals are concrete', () => {
  const map = readFileSync(join(repoRoot, 'docs/design/api-map.md'), 'utf8');
  const section = snapshotterSection(map);
  expect(section).not.toMatch(/\| recon:/);
  const actuals = actualCells(section);
  expect(actuals.length).toBeGreaterThanOrEqual(8);
  for (const cell of actuals) {
    expect(cell, 'Actual cell must not be empty').toMatch(/\S/);
    expect(cell).not.toMatch(/^recon:/);
  }

  expect(section).toContain('crates/venus-sidecar');
  expect(section).toContain('Compose `sidecar`');
  expect(section).toContain(EXPORT_COMMAND);
  expect(section).toContain('from-pinned-cli.js');
  expect(section).toContain('spec/home.md');
  expect(section).toContain('wiki/.venus/ids/doc:home.json');
  expect(section).toContain('SNAPSHOT_IDLE_MS');
  expect(section).toContain('60000');
  expect(section).toContain('POST http://127.0.0.1:3002/flush');
  expect(section).toContain('GET http://127.0.0.1:3002/git/log');
  expect(section).toMatch(/one inflight/i);
  expect(section).toMatch(/pin at \*\*run\*\*|pin at run/i);
  expect(section).toContain('no `jobs`');
  expect(section).toMatch(/crash recovery/i);
  expect(section).toContain('doc:home');
  expect(section).toContain('WIKI_DIR');
  expect(section).toMatch(/autoinit/i);
  expect(section).toContain('/wiki/');
  expect(section).not.toMatch(/spec\/(?!home\.md)\w+\.md/);
});

test('Product ignores wiki/', () => {
  const gi = readFileSync(join(repoRoot, '.gitignore'), 'utf8');
  expect(gi).toMatch(/^\/wiki\/?\s*$/m);
  const out = execFileSync('git', ['check-ignore', '-v', 'wiki'], {
    cwd: repoRoot,
    encoding: 'utf8',
  });
  expect(out).toMatch(/wiki/);
});

test('Spike convert: Path B does not import splice / pane / git', () => {
  const pin = readFileSync(join(here, 'pin-from-doc.js'), 'utf8');
  const cli = readFileSync(CONVERT_CLI, 'utf8');
  for (const [name, src] of [
    ['pin-from-doc.js', pin],
    ['from-pinned-cli.js', cli],
  ] as const) {
    expect(src, name).not.toMatch(/from ['"]\.\/splice\.js['"]/);
    expect(src, name).not.toMatch(/from ['"].*mount-md-pane/);
    expect(src, name).not.toMatch(/simple-git|git2|git commit/);
  }
  expect(pin).toContain("from './from-doc.js'");
  expect(cli).toContain('fromPinnedBytes');
  expect(cli).toContain('ssrLoadModule');
});

test('Spike convert: fromPinnedBytes of a seed pin contains Why Venus (offline, not the live tab)', async () => {
  const session = await createM0Workspace(new MemoryNoopProvider());
  const pin = pinYjsBytes(session.store.spaceDoc);
  const converted = await fromPinnedBytes(pin.bytes, {
    clock: pin.clock,
    docId: PAGE_DOC_ID,
  });
  expect(converted.markdown).toContain(SEED_H1);
  expect(converted.sidecar.docId).toBe(PAGE_DOC_ID);
  expect(converted.sidecar.clock).toBe(pin.clock);
});

describe.skipIf(!hubUp)('Spike convert: hub GET export → fromPinnedBytes (no git)', () => {
  test('export command then Path B markdown contains Why Venus', async () => {
    await new Promise((r) => setTimeout(r, 2000));
    execFileSync('curl', ['-sSSf', EXPORT_URL, '-o', '/tmp/venus-page.yjs'], {
      stdio: 'pipe',
    });
    const bytes = new Uint8Array(readFileSync('/tmp/venus-page.yjs'));
    expect(bytes.byteLength).toBeGreaterThan(2);

    const { OctoBaseBlobSource } = await import('../providers/blob-source.js');
    const converted = await fromPinnedBytes(bytes, {
      blobSources: { main: new OctoBaseBlobSource({ origin: BLOB_ORIGIN }) },
    });
    expect(converted.markdown).toContain(SEED_H1);
    expect(converted.sidecar.docId).toBe(PAGE_DOC_ID);

    const cliOut = execFileSync(process.execPath, [CONVERT_CLI, '/tmp/venus-page.yjs'], {
      cwd: webRoot,
      encoding: 'utf8',
      env: { ...process.env, VENUS_BLOB_ORIGIN: BLOB_ORIGIN },
      timeout: 60_000,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const parsed = JSON.parse(cliOut);
    expect(parsed.markdown).toContain(SEED_H1);
    expect(parsed.sidecar.docId).toBe(PAGE_DOC_ID);
  }, 90_000);
});
