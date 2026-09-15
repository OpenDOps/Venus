#!/usr/bin/env node
/**
 * M3.0 step-ha-owner DoD: second process is not a second owner; SIGTERM drain.
 *
 * From repo root: pnpm compose:ha
 */
import { execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { request as httpRequest } from 'node:http';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Y from 'yjs';

import { WORKSPACE_ID } from '../src/host/ids.js';
import { OctoBaseKeckProvider } from '../src/host/providers/octobase-keck-provider.js';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '../../..');
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

function hubUrl(port) {
  return `http://127.0.0.1:${port}`;
}

function healthUrl(port) {
  return `${hubUrl(port)}/collaboration/${WORKSPACE_ID}`;
}

async function waitHub(port, label) {
  const deadline = Date.now() + 90_000;
  let last = '';
  while (Date.now() < deadline) {
    try {
      const root = await fetch(`${hubUrl(port)}/`, {
        signal: AbortSignal.timeout(1500),
      });
      const rootText = (await root.text()).trim();
      const health = await fetch(healthUrl(port), {
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
  throw new Error(`hub not ready on :${port} (${label}): ${last}`);
}

async function waitHubStopped() {
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    const running = composeOut(['ps', '--status', 'running', '--format', '{{.Service}}'])
      .trim()
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean);
    if (!running.includes('hub')) {
      console.log(`hub stopped: ${running.join(' ') || '(none)'}`);
      return;
    }
    await sleep(250);
  }
  throw new Error('hub still running after SIGTERM');
}

function withReady(provider, ms) {
  return Promise.race([
    provider.whenReady(),
    new Promise((_, reject) => {
      setTimeout(() => reject(new Error('whenReady timeout')), ms);
    }),
  ]);
}

function wsUpgrade(port, workspace) {
  return new Promise((resolve, reject) => {
    const req = httpRequest({
      host: '127.0.0.1',
      port,
      path: `/collaboration/${workspace}`,
      headers: {
        Connection: 'Upgrade',
        Upgrade: 'websocket',
        'Sec-WebSocket-Key': 'dGhlIHNhbXBsZSBub25jZQ==',
        'Sec-WebSocket-Version': '13',
        'Sec-WebSocket-Protocol': 'AFFiNE',
      },
    });
    req.setTimeout(8_000, () => {
      req.destroy(new Error('upgrade timeout'));
    });
    req.on('upgrade', (_res, socket) => {
      socket.destroy();
      resolve({ status: 101, body: '' });
    });
    req.on('response', (res) => {
      const chunks = [];
      res.on('data', (c) => chunks.push(c));
      res.on('end', () => {
        resolve({
          status: res.statusCode ?? 0,
          body: Buffer.concat(chunks).toString('utf8'),
        });
      });
    });
    req.on('error', reject);
    req.end();
  });
}

function leaseQuery(sql) {
  return composeOut([
    'exec',
    '-T',
    'postgres',
    'psql',
    '-U',
    'venus',
    '-d',
    'venus',
    '-tAc',
    sql,
  ]).trim();
}

function leaseRows() {
  return leaseQuery(
    `SELECT COUNT(*)::text FROM workspace_lease WHERE workspace_id = '${WORKSPACE_ID}'`,
  );
}

function leaseLiveOwners() {
  return leaseQuery(
    `SELECT COALESCE(string_agg(owner, ','), '') FROM workspace_lease
     WHERE workspace_id = '${WORKSPACE_ID}' AND lease_until > now()`,
  );
}

async function wsHasDrain(port, drainId, label) {
  const doc = new Y.Doc();
  const provider = new OctoBaseKeckProvider(
    `ws://127.0.0.1:${port}/collaboration/${WORKSPACE_ID}`,
  );
  try {
    provider.connect('spike', doc);
    await withReady(provider, 10_000);
    const got = doc.getMap('spike').get('drain');
    if (got !== drainId) {
      throw new Error(
        `${label} missing spike.drain=${drainId} (got ${String(got)})`,
      );
    }
    console.log(`${label} WS spike.drain=${drainId}`);
  } finally {
    provider.disconnect('spike');
  }
}

try {
  compose(['--profile', 'ha', 'stop', 'hub-b']);
} catch {
  // hub-b may not exist yet
}

compose(['up', '--build', '-d', '--wait', 'postgres', 'hub']);
await waitHub(3000, 'hub A');

const doc = new Y.Doc();
const providerA = new OctoBaseKeckProvider(
  `ws://127.0.0.1:3000/collaboration/${WORKSPACE_ID}`,
);
providerA.connect('spike', doc);
await withReady(providerA, 15_000);
console.log(`A holds lease workspace=${WORKSPACE_ID}`);

compose(['--profile', 'ha', 'up', '--build', '-d', '--wait', 'hub-b']);
await waitHub(3001, 'hub B');

const refused = await wsUpgrade(3001, WORKSPACE_ID);
if (refused.status === 101) {
  throw new Error('hub-b must not apply a second live doc (WS upgraded on :3001)');
}
if (refused.status !== 503) {
  throw new Error(
    `hub-b WS must be 503, got ${refused.status}: ${refused.body}`,
  );
}
if (!refused.body.includes('owned by another hub')) {
  throw new Error(`503 body must name a lease conflict: ${refused.body}`);
}
const nLease = leaseRows();
if (nLease !== '1') {
  throw new Error(`Postgres must have one lease row, got ${nLease}`);
}
const liveOwners = leaseLiveOwners();
if (liveOwners.split(',').includes('hub-b')) {
  throw new Error(`hub-b must not own the lease while A holds it: ${liveOwners}`);
}
console.log(`Second owner refused: HTTP ${refused.status}; lease rows=${nLease} owners=${liveOwners}`);

const drainId = randomUUID();
doc.getMap('spike').set('drain', drainId);
await sleep(100);
compose(['stop', 'hub']);
await waitHubStopped();
providerA.disconnect('spike');

const liveAfter = leaseLiveOwners();
if (liveAfter !== '') {
  throw new Error(
    `lease must be gone after SIGTERM so B can own (live owners=${liveAfter})`,
  );
}
console.log('Drain: A lease gone after SIGTERM');

const deadline = Date.now() + 20_000;
let last = '';
while (Date.now() < deadline) {
  try {
    await wsHasDrain(3001, drainId, 'hub-b after A drain');
    last = '';
    break;
  } catch (err) {
    last = err instanceof Error ? err.message : String(err);
    await sleep(500);
  }
}
if (last) {
  throw new Error(`B did not hydrate flushed drain write: ${last}`);
}

const b = new Y.Doc();
const providerB = new OctoBaseKeckProvider(
  `ws://127.0.0.1:3001/collaboration/${WORKSPACE_ID}`,
);
try {
  providerB.connect('spike', b);
  await withReady(providerB, 20_000);
  if (b.getMap('spike').get('drain') !== drainId) {
    throw new Error('hub-b WS Step2 missing spike.drain after A drain');
  }
  console.log('hub-b WS spike.drain after A drain');
} finally {
  providerB.disconnect('spike');
}

compose(['--profile', 'ha', 'stop', 'hub-b']);
compose(['up', '-d', '--wait', 'hub']);
await waitHub(3000, 'product hub restored');
console.log('compose-ha: second owner 503; SIGTERM flushed drain and dropped lease');
