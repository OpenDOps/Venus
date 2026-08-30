#!/usr/bin/env node
/**
 * M1 throwaway spike. Delete before M1 exit (step-verify).
 *
 * Two Y.Docs on OctoBase keck. Not BlockSuite. Not Playwright.
 *
 *   KECK_WS=ws://127.0.0.1:3000/collaboration/venus-m0 node scripts/m1-recon-spike.mjs
 *
 * Persist check (no write): after keck restart / compose down+up
 *
 *   SPIKE_CHECK=1 node scripts/m1-recon-spike.mjs
 *
 * Product store is Compose Postgres (`DATABASE_URL`). Do not set
 * USE_MEMORY_SQLITE. Do not omit DATABASE_URL on keck.
 */
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
// y-protocols / lib0 are not direct @venus/web deps yet (step-provider).
const require = createRequire(
  join(
    root,
    'node_modules/.pnpm/@blocksuite+store@0.22.4/node_modules/@blocksuite/store/package.json'
  )
);

const Y = require('yjs');
const encoding = require('lib0/encoding');
const decoding = require('lib0/decoding');
const syncProtocol = require('y-protocols/sync');

const WS_URL =
  process.env.KECK_WS ?? 'ws://127.0.0.1:3000/collaboration/venus-m0';
const ORIGIN = 'keck';
const MSG_SYNC = 0;
const MSG_AWARENESS = 1;
const MSG_AUTH = 2;
const MSG_QUERY_AWARENESS = 3;
const DEADLINE_MS = 5000;

function sendSyncStep1(ws, doc) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, MSG_SYNC);
  syncProtocol.writeSyncStep1(encoder, doc);
  ws.send(encoding.toUint8Array(encoder));
}

function handleBinary(ws, doc, buf) {
  const decoder = decoding.createDecoder(new Uint8Array(buf));
  while (decoding.hasContent(decoder)) {
    const messageType = decoding.readVarUint(decoder);
    if (messageType === MSG_SYNC) {
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, MSG_SYNC);
      const syncType = syncProtocol.readSyncMessage(
        decoder,
        encoder,
        doc,
        ORIGIN
      );
      const reply = encoding.toUint8Array(encoder);
      if (reply.byteLength > 1) {
        ws.send(reply);
      }
      if (
        syncType === syncProtocol.messageYjsSyncStep2 &&
        !doc._venusSynced
      ) {
        doc._venusSynced = true;
        doc._venusOnSynced?.();
      }
    } else if (messageType === MSG_AWARENESS) {
      decoding.readVarUint8Array(decoder);
    } else if (messageType === MSG_AUTH) {
      decoding.readVarUint(decoder);
    } else if (messageType === MSG_QUERY_AWARENESS) {
      // keck answers awareness itself; ignore
    } else {
      break;
    }
  }
}

function connectDoc(label) {
  const doc = new Y.Doc();
  doc._venusSynced = false;
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(WS_URL, ['AFFiNE']);
    ws.binaryType = 'arraybuffer';
    const timer = setTimeout(() => {
      reject(new Error(`${label}: connect/sync timed out`));
    }, DEADLINE_MS);

    doc.on('update', (update, origin) => {
      if (origin === ORIGIN || ws.readyState !== WebSocket.OPEN) return;
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, MSG_SYNC);
      syncProtocol.writeUpdate(encoder, update);
      ws.send(encoding.toUint8Array(encoder));
    });

    ws.addEventListener('open', () => {
      console.log(`${label}: ws open protocol=${ws.protocol}`);
      sendSyncStep1(ws, doc);
    });
    ws.addEventListener('error', (ev) => {
      reject(new Error(`${label}: ws error ${ev.message ?? ''}`));
    });
    ws.addEventListener('message', (ev) => {
      const data = ev.data;
      if (typeof data === 'string') return;
      handleBinary(ws, doc, data);
    });

    doc._venusOnSynced = () => {
      clearTimeout(timer);
      const bytes = Y.encodeStateAsUpdate(doc);
      console.log(`${label}: synced, encodeStateAsUpdate ${bytes.byteLength} bytes`);
      resolve({ label, doc, ws });
    };
  });
}

function waitForMap(doc, timeoutMs) {
  return new Promise((resolve, reject) => {
    const map = doc.getMap('spike');
    if (map.get('k') === 'v') {
      resolve();
      return;
    }
    const timer = setTimeout(() => {
      map.unobserve(onChange);
      reject(
        new Error(
          `B did not see spike.k=v within ${timeoutMs}ms (got ${JSON.stringify(map.get('k'))})`
        )
      );
    }, timeoutMs);
    const onChange = () => {
      if (map.get('k') === 'v') {
        clearTimeout(timer);
        map.unobserve(onChange);
        resolve();
      }
    };
    map.observe(onChange);
  });
}

if (process.env.SPIKE_CHECK === '1') {
  const c = await connectDoc('check');
  const got = c.doc.getMap('spike').get('k');
  console.log(`check: spike.k=${JSON.stringify(got)}`);
  if (got !== 'v') {
    process.exitCode = 1;
    console.error('SPIKE CHECK FAIL');
  } else {
    console.log('SPIKE CHECK OK');
  }
  c.ws.close();
  c.doc.destroy();
} else {
  const a = await connectDoc('A');
  const b = await connectDoc('B');

  const t0 = Date.now();
  a.doc.getMap('spike').set('k', 'v');
  console.log('A: set spike.k=v');

  await waitForMap(b.doc, DEADLINE_MS);
  const elapsed = Date.now() - t0;
  const got = b.doc.getMap('spike').get('k');
  const updateB = Y.encodeStateAsUpdate(b.doc);
  console.log(
    `B: spike.k=${JSON.stringify(got)} after ${elapsed}ms; encodeStateAsUpdate ${updateB.byteLength} bytes`
  );

  if (got !== 'v') {
    process.exitCode = 1;
  } else {
    console.log('SPIKE OK');
  }

  a.ws.close();
  b.ws.close();
  a.doc.destroy();
  b.doc.destroy();
}
