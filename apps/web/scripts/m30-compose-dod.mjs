#!/usr/bin/env node
/**
 * M3.0 step-compose-hub DoD: Server up + persist across `restart hub`
 * and `down` without `-v`. Spike / export against Compose, not keck.
 *
 * From repo root: pnpm compose:dod
 */
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Y from 'yjs';

import { WORKSPACE_ID } from '../src/host/ids.js';
import { OctoBaseKeckProvider } from '../src/host/providers/octobase-keck-provider.js';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const HUB = 'http://127.0.0.1:3000';
const HEALTH = `${HUB}/collaboration/${WORKSPACE_ID}`;
const COMPOSE_TIMEOUT_MS = 20 * 60 * 1000;

function compose(args, timeoutMs = COMPOSE_TIMEOUT_MS) {
  execFileSync('docker', ['compose', ...args], {
    cwd: repoRoot,
    stdio: 'inherit',
    timeout: timeoutMs,
  });
}

function composeOut(args) {
  return execFileSync('docker', ['compose', ...args], {
    cwd: repoRoot,
    encoding: 'utf8',
    timeout: 60_000,
  });
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function waitHub(label) {
  const deadline = Date.now() + 90_000;
  let last = '';
  while (Date.now() < deadline) {
    try {
      const root = await fetch(`${HUB}/`, { signal: AbortSignal.timeout(1500) });
      const rootText = (await root.text()).trim();
      const health = await fetch(HEALTH, {
        method: 'POST',
        signal: AbortSignal.timeout(1500),
      });
      const healthText = await health.text();
      last = `GET / ${root.status} ${rootText}; POST health ${health.status} ${healthText}`;
      if (
        root.ok &&
        rootText === 'venus-hub' &&
        health.ok &&
        healthText.includes('"protocol":"AFFiNE"')
      ) {
        console.log(`${label}: ${last}`);
        return;
      }
    } catch (err) {
      last = err instanceof Error ? err.message : String(err);
    }
    await sleep(500);
  }
  throw new Error(`hub not ready (${label}): ${last}`);
}

function assertProductCompose() {
  const running = composeOut(['ps', '--format', '{{.Service}}'])
    .trim()
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
    .sort();
  if (running.includes('octobase')) {
    throw new Error(`octobase must not be required: ${running.join(' ')}`);
  }
  if (!running.includes('postgres') || !running.includes('hub')) {
    throw new Error(`expected postgres + hub running, got: ${running.join(' ')}`);
  }
  const cfg = composeOut(['config']);
  if (!cfg.includes('pg-venus-data')) {
    throw new Error('compose must use volume pg-venus-data (not leftover keck pg-data)');
  }
  if (/^\s*octobase:/m.test(cfg)) {
    throw new Error('resolved compose still has an octobase service');
  }
  console.log(`compose ps: ${running.join(' ')}`);
}

function withReady(provider, ms) {
  return Promise.race([
    provider.whenReady(),
    new Promise((_, reject) => {
      setTimeout(() => reject(new Error('whenReady timeout')), ms);
    }),
  ]);
}

async function writeSpike(workspace) {
  const doc = new Y.Doc();
  const provider = new OctoBaseKeckProvider(
    `ws://127.0.0.1:3000/collaboration/${workspace}`,
  );
  provider.connect('spike', doc);
  await withReady(provider, 15_000);
  doc.getMap('spike').set('k', 'v');
  await sleep(2500);
  provider.disconnect('spike');
  console.log(`SPIKE OK workspace=${workspace}`);
}

function mapHasV(doc) {
  return doc.getMap('spike').get('k') === 'v';
}

async function checkExport(workspace, label) {
  const url = `${HUB}/api/block/${workspace}/export`;
  const res = await fetch(url, { signal: AbortSignal.timeout(10_000) });
  if (!res.ok) {
    throw new Error(`${label} export HTTP ${res.status}`);
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  const doc = new Y.Doc();
  Y.applyUpdate(doc, bytes);
  if (!mapHasV(doc)) {
    throw new Error(`${label} export ${bytes.byteLength} bytes missing spike.k=v`);
  }
  console.log(`${label} export ${bytes.byteLength} bytes spike.k=v`);
}

async function checkWs(workspace, label) {
  const doc = new Y.Doc();
  const provider = new OctoBaseKeckProvider(
    `ws://127.0.0.1:3000/collaboration/${workspace}`,
  );
  try {
    provider.connect('spike', doc);
    await withReady(provider, 20_000);
    if (!mapHasV(doc)) {
      throw new Error(`${label} WS Step2 missing spike.k=v`);
    }
    console.log(`${label} WS spike.k=v`);
  } finally {
    provider.disconnect('spike');
  }
}

async function checkSpike(workspace, label) {
  await checkExport(workspace, label);
  const deadline = Date.now() + 30_000;
  let last = '';
  while (Date.now() < deadline) {
    try {
      await checkWs(workspace, label);
      return;
    } catch (err) {
      last = err instanceof Error ? err.message : String(err);
      await sleep(1000);
    }
  }
  throw new Error(`${label} WS still missing spike after export passed: ${last}`);
}

const workspace = randomUUID();

compose(['up', '--build', '-d', '--wait', 'postgres', 'hub']);
await waitHub('Server up');
assertProductCompose();
await writeSpike(workspace);
compose(['restart', 'hub']);
await waitHub('after restart hub');
assertProductCompose();
await checkSpike(workspace, 'after restart hub');
compose(['down']);
compose(['up', '-d', '--wait', 'postgres', 'hub']);
await waitHub('after down (no -v) + up');
assertProductCompose();
await checkSpike(workspace, 'after down + up');
console.log('compose-dod: persist kept spike.k=v (pg-venus-data, not keck jwst)');
