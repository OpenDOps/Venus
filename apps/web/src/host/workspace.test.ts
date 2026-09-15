import { expect, test } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Y from 'yjs';
import { PAGE_DOC_ID, WORKSPACE_ID, CATALOG_GUID } from './ids.js';
import { createM0Workspace, openWorkspaceDoc } from './workspace.js';
import { SEED_H1, SEED_TITLE } from './seed.js';
import pkg from '../../package.json' with { type: 'json' };
import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';

function h1Count(
  store: Awaited<ReturnType<typeof createM0Workspace>>['store'],
) {
  const note = store.root?.children.find((c) => c.flavour === 'affine:note');
  return note?.children.filter((c) => c.props.type === 'h1').length ?? 0;
}

test('single page default tree', async () => {
  const { workspace, store, docId } = await createM0Workspace();
  expect(docId).toBe(PAGE_DOC_ID);
  expect(workspace.id).toBe(WORKSPACE_ID);
  expect(workspace.docs.size).toBe(1);

  const root = store.root;
  expect(root?.flavour).toBe('affine:page');
  expect(root?.children.map((c) => c.flavour).sort()).toEqual([
    'affine:note',
    'affine:surface',
  ]);
  const note = root?.children.find((c) => c.flavour === 'affine:note');
  expect(note?.children[0]?.flavour).toBe('affine:paragraph');
  expect(note?.children.some((c) => c.flavour === 'affine:paragraph')).toBe(
    true,
  );
  expect(store.canUndo).toBe(false);
});

test('memory createDoc second page is a second Y.Doc', async () => {
  const { workspace, store, provider } = await createM0Workspace();
  const uuid = crypto.randomUUID();
  const second = await openWorkspaceDoc(workspace, provider, uuid);
  expect(second.docId).toBe(uuid);
  expect(workspace.docs.size).toBe(2);
  expect(second.store.root?.flavour).toBe('affine:page');
  expect(second.store.root?.props.title?.toString()).toBe('');
  expect(store.root?.props.title?.toString()).toBe(SEED_TITLE);
  const note = second.store.root?.children.find(
    (c) => c.flavour === 'affine:note',
  );
  expect(
    note?.children.some(
      (c) => c.flavour === 'affine:paragraph' && c.props.type === 'h1',
    ),
  ).toBe(false);
});

test('App boot does not mint a second page', () => {
  const hostDir = dirname(fileURLToPath(import.meta.url));
  const app = readFileSync(join(hostDir, '../App.tsx'), 'utf8');
  expect(app).toMatch(/createM0Workspace/);
  expect(app).toMatch(/openCatalog/);
  expect(app).toMatch(/connectDocs/);
  expect(app).toMatch(/disposeCatalog/);
  expect(app).not.toMatch(/openWorkspaceDoc/);
  expect(app).not.toMatch(/CatalogTree/);
});

test('no sync socket on create', async () => {
  const Ws = globalThis.WebSocket;
  const constructed: unknown[] = [];
  // @ts-expect-error stub
  globalThis.WebSocket = class {
    constructor(...args: unknown[]) {
      constructed.push(args);
      throw new Error('WebSocket must not open in M0');
    }
  };

  try {
    await createM0Workspace();
    expect(constructed).toEqual([]);
  } finally {
    globalThis.WebSocket = Ws;
  }

  const deps: Record<string, string | undefined> = {
    ...pkg.dependencies,
    ...pkg.devDependencies,
  };
  for (const name of [
    'y-websocket',
    'y-indexeddb',
    'hocuspocus',
    '@affine/core',
    '@affine/graphql',
    '@blocksuite/integration-test',
  ]) {
    expect(deps[name]).toBeUndefined();
  }
});

test('hydrate waits for synced before seeding', async () => {
  const order: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(_id, _ydoc: Doc) {
      order.push('connect');
    },
    disconnect() {},
    whenReady() {
      order.push('wait');
      return Promise.resolve().then(() => {
        order.push('synced');
      });
    },
  };

  const { store } = await createM0Workspace(provider);
  order.push('done');
  expect(order).toEqual(['connect', 'wait', 'synced', 'done']);
  expect(store.root?.flavour).toBe('affine:page');
});

test('hydrate skips seed when a page root already exists', async () => {
  const { store: first } = await createM0Workspace();
  const snapshot = Y.encodeStateAsUpdate(first.spaceDoc);

  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(_id, ydoc: Doc) {
      Y.applyUpdate(ydoc, snapshot);
    },
    disconnect() {},
    whenReady() {
      return Promise.resolve();
    },
  };

  const { store } = await createM0Workspace(provider);
  expect(store.root?.flavour).toBe('affine:page');
  expect(store.root?.props.title?.toString()).toBe(SEED_TITLE);
  expect(h1Count(store)).toBe(1);
  expect(
    store.root?.children
      .find((c) => c.flavour === 'affine:note')
      ?.children.find((c) => c.props.type === 'h1')
      ?.props.text?.toString(),
  ).toBe(SEED_H1);
});

test('abort during wait disconnects without throwing a sync timeout', async () => {
  const ac = new AbortController();
  let disconnected = 0;
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect() {},
    disconnect() {
      disconnected += 1;
    },
    whenReady() {
      ac.abort();
      return new Promise(() => {});
    },
  };

  await expect(
    createM0Workspace(provider, { signal: ac.signal }),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(disconnected).toBe(1);
});

test('abort during wait disconnects connectDocs extras', async () => {
  const ac = new AbortController();
  const disconnected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect() {},
    disconnect(id: string) {
      disconnected.push(id);
    },
    whenReady() {
      ac.abort();
      return new Promise(() => {});
    },
  };

  await expect(
    createM0Workspace(provider, {
      signal: ac.signal,
      connectDocs: [{ docId: CATALOG_GUID, ydoc: new Y.Doc() }],
    }),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(disconnected.sort()).toEqual([CATALOG_GUID, PAGE_DOC_ID].sort());
});

test('blobSources.main is wired as blobSync main', async () => {
  const keys: string[] = [];
  const main = {
    name: 'venus',
    readonly: false,
    get: async () => null,
    set: async (key: string) => {
      keys.push(key);
      return key;
    },
    delete: async () => {},
    list: async () => [],
  };
  const { store } = await createM0Workspace(undefined, {
    blobSources: { main },
  });
  expect(store.blobSync.main.name).toBe('venus');
  const id = await store.blobSync.set(new Blob([new Uint8Array([1, 2, 3])]));
  expect(keys).toEqual([id]);
});
