import * as Y from 'yjs';
import { expect, test } from 'vitest';
import { PAGE_DOC_ID } from '../ids.js';
import type { SyncProvider } from '../sync-provider.js';
import { createM0Workspace } from '../workspace.js';
import { createPublishedDoc } from './create-published.js';
import { seedOnce } from './ops.js';
import { FOLDER_SPEC_ID, getNode, listNodes } from './schema.js';

async function seeded() {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace };
}

function docIds(catalog: Y.Doc) {
  return listNodes(catalog)
    .filter((n) => n.kind === 'doc')
    .map((n) => n.id);
}

test('memory create seeds locally and does not open a socket', async () => {
  const { catalog, workspace } = await seeded();
  const connected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: true,
    connect(id) {
      connected.push(id);
    },
    disconnect() {},
    whenReady() {
      return Promise.resolve();
    },
  };
  const node = await createPublishedDoc(catalog, workspace, provider, {
    createAt: FOLDER_SPEC_ID,
  });
  expect(connected).toEqual([]);
  expect(node.gitPath).toBe(`spec/${node.id}.md`);
  expect(workspace.getDoc?.(node.id)?.getStore()?.root?.flavour).toBe(
    'affine:page',
  );
});

test('live create inserts the catalog node only after the hub confirms the seed', async () => {
  const { catalog, workspace } = await seeded();
  const order: string[] = [];
  let nodeAtConfirm = true;
  let seedBytes = 0;
  let pageDoc: Y.Doc | null = null;
  const provider: SyncProvider = {
    kind: 'venus',
    synced: false,
    connect(id, ydoc) {
      pageDoc = ydoc;
      order.push(`connect:${id}`);
    },
    disconnect(id) {
      order.push(`disconnect:${id}`);
    },
    whenReady() {
      order.push('ready');
      return Promise.resolve();
    },
    nextStep2() {
      order.push('next');
      return Promise.resolve();
    },
    async confirmApplied(id) {
      order.push('confirm');
      nodeAtConfirm = Boolean(getNode(catalog, id));
      seedBytes = pageDoc ? Y.encodeStateAsUpdate(pageDoc).byteLength : 0;
    },
  };
  const node = await createPublishedDoc(catalog, workspace, provider, {
    createAt: FOLDER_SPEC_ID,
  });
  expect(nodeAtConfirm).toBe(false);
  expect(seedBytes).toBeGreaterThan(64);
  expect(getNode(catalog, node.id)?.gitPath).toBe(`spec/${node.id}.md`);
  expect(order).toEqual([
    `connect:${node.id}`,
    'ready',
    'next',
    'confirm',
    `disconnect:${node.id}`,
  ]);
});

test('a rejected seed confirm leaves no catalog node and disconnects', async () => {
  const { catalog, workspace } = await seeded();
  const disconnected: string[] = [];
  const provider: SyncProvider = {
    kind: 'venus',
    synced: false,
    connect() {},
    disconnect(id) {
      disconnected.push(id);
    },
    whenReady() {
      return Promise.resolve();
    },
    nextStep2() {
      return Promise.resolve();
    },
    confirmApplied() {
      return Promise.reject(new Error('hub did not apply the page seed'));
    },
  };
  await expect(
    createPublishedDoc(catalog, workspace, provider, {
      createAt: FOLDER_SPEC_ID,
    }),
  ).rejects.toThrow(/did not apply/);
  expect(docIds(catalog)).toEqual([PAGE_DOC_ID]);
  expect(disconnected).toHaveLength(1);
});

test('a failed sync leaves no catalog node and disconnects', async () => {
  const { catalog, workspace } = await seeded();
  const disconnected: string[] = [];
  const provider: SyncProvider = {
    kind: 'venus',
    synced: false,
    connect() {},
    disconnect(id) {
      disconnected.push(id);
    },
    whenReady() {
      return Promise.reject(new Error('hub websocket error'));
    },
  };
  await expect(
    createPublishedDoc(catalog, workspace, provider, {
      createAt: FOLDER_SPEC_ID,
    }),
  ).rejects.toThrow(/websocket error/);
  expect(docIds(catalog)).toEqual([PAGE_DOC_ID]);
  expect(disconnected).toHaveLength(1);
});
