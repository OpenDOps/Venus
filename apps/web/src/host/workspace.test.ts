import { expect, test } from 'vitest';
import * as Y from 'yjs';
import { PAGE_DOC_ID, WORKSPACE_ID } from './ids.js';
import { createM0Workspace } from './workspace.js';
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

test('blobSources.main is wired as blobSync main', async () => {
  const keys: string[] = [];
  const main = {
    name: 'octobase',
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
  expect(store.blobSync.main.name).toBe('octobase');
  const id = await store.blobSync.set(new Blob([new Uint8Array([1, 2, 3])]));
  expect(keys).toEqual([id]);
});
