import { expect, test, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Y from 'yjs';
import { PAGE_DOC_ID, WORKSPACE_ID, CATALOG_GUID } from './ids.js';
import { createM0Workspace, openPageStore, openWorkspaceDoc, OPEN_PAGE_ATTEMPTS, OPEN_PAGE_RETRY_MS, SYNC_TIMEOUT_MS, isOpenPageTransportError, shouldSeedEmptyPage, waitUntilSynced } from './workspace.js';
import { SEED_H1, SEED_TITLE } from './seed.js';
import pkg from '../../package.json' with { type: 'json' };
import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';
import { createDoc, seedOnce } from './catalog/ops.js';
import { FOLDER_SPEC_ID } from './catalog/schema.js';

function syncTimeoutError(kind = 'memory') {
  return new Error(
    `SyncProvider kind=${kind} did not sync within ${SYNC_TIMEOUT_MS}ms. Is Compose hub up (pnpm sync:up)?`,
  );
}

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
  expect(app).toMatch(/openPageStore/);
  expect(app).toMatch(/CatalogTree/);
});

function appOpenDocSrc() {
  const hostDir = dirname(fileURLToPath(import.meta.url));
  const app = readFileSync(join(hostDir, '../App.tsx'), 'utf8');
  const start = app.indexOf('const openDoc = useCallback');
  const end = app.indexOf('void createM0Workspace');
  if (start < 0 || end <= start) {
    throw new Error('openDoc slice not found in App.tsx');
  }
  return app.slice(start, end);
}

test('openDoc catch does not impersonate a missing doc', () => {
  const openDoc = appOpenDocSrc();
  const catchIdx = openDoc.lastIndexOf('} catch (err)');
  expect(catchIdx).toBeGreaterThan(-1);
  const catchBlock = openDoc.slice(catchIdx);
  expect(catchBlock).toContain('console.error');
  expect(catchBlock).not.toContain('setOpenDocId(');
  expect(catchBlock).not.toContain('setOpenStore(');
  expect(catchBlock).not.toContain('homeStore');
  expect(openDoc).toContain('extraDocIdRef.current === nextId');
  expect(openDoc).toContain('getDoc');
  expect(openDoc).toMatch(/getDoc[\s\S]*setOpenStore\(/);
});

test('openDoc aborts in-flight openPageStore when navigating away', () => {
  const openDoc = appOpenDocSrc();
  expect(openDoc).toContain('openAbortRef.current?.abort()');
  expect(openDoc).toContain('signal: ac.signal');
  expect(openDoc).toContain("err.name === 'AbortError'");
  expect(openDoc).not.toContain('await delay(');
});

test('openWorkspaceDoc reuses createDoc from catalog ops', async () => {
  const { workspace, provider } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(workspace.docs.size).toBe(2);
  const opened = await openWorkspaceDoc(workspace, provider, created.id);
  expect(opened.docId).toBe(created.id);
  expect(workspace.docs.size).toBe(2);
  expect(opened.store.root?.flavour).toBe('affine:page');
});

function emptyVenusProvider(disconnected: string[] = []): SyncProvider {
  return {
    kind: 'venus',
    synced: false,
    connect() {},
    disconnect(id: string) {
      disconnected.push(id);
    },
    whenReady() {
      return Promise.resolve();
    },
  };
}

test('shouldSeedEmptyPage is memory-only unless seedIfEmpty is set', () => {
  expect(shouldSeedEmptyPage({ kind: 'memory' })).toBe(true);
  expect(shouldSeedEmptyPage({ kind: 'venus' })).toBe(false);
  expect(shouldSeedEmptyPage({ kind: 'y-websocket' })).toBe(false);
  expect(shouldSeedEmptyPage({ kind: 'venus' }, { seedIfEmpty: true })).toBe(
    true,
  );
  expect(shouldSeedEmptyPage({ kind: 'memory' }, { seedIfEmpty: false })).toBe(
    false,
  );
});

test('createDoc seeds empty affine:page locally without connecting', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const store = workspace.getDoc?.(created.id)?.getStore();
  expect(store?.root?.flavour).toBe('affine:page');
  expect(store?.root?.props.title?.toString()).toBe('');
});

test('openWorkspaceDoc on venus does not seed a blank page after empty sync', async () => {
  const { workspace } = await createM0Workspace();
  const disconnected: string[] = [];
  const provider = emptyVenusProvider(disconnected);
  const uuid = crypto.randomUUID();
  await expect(openWorkspaceDoc(workspace, provider, uuid)).rejects.toMatchObject(
    {
      name: 'EmptyPageSyncError',
      message: `page ${uuid} has no affine:page root after sync`,
    },
  );
  expect(disconnected).toEqual([uuid]);
  const store = workspace.getDoc?.(uuid)?.getStore();
  expect(store?.root?.flavour).not.toBe('affine:page');
});

test('openWorkspaceDoc venus seedIfEmpty still seeds when asked', async () => {
  const { workspace } = await createM0Workspace();
  const provider = emptyVenusProvider();
  const uuid = crypto.randomUUID();
  const opened = await openWorkspaceDoc(workspace, provider, uuid, {
    seedIfEmpty: true,
  });
  expect(opened.store.root?.flavour).toBe('affine:page');
});

test('catalog createDoc then venus open uses local seed, not a post-sync seed', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const provider = emptyVenusProvider();
  const opened = await openWorkspaceDoc(workspace, provider, created.id);
  expect(opened.store.root?.flavour).toBe('affine:page');
  expect(opened.store.root?.props.title?.toString()).toBe('');
});

test('openPageStore does not retry empty venus sync (would seed on retry)', async () => {
  const { workspace } = await createM0Workspace();
  let connects = 0;
  const provider: SyncProvider = {
    kind: 'venus',
    synced: false,
    connect() {
      connects += 1;
    },
    disconnect() {},
    whenReady() {
      return Promise.resolve();
    },
  };
  await expect(
    openPageStore(workspace, provider, crypto.randomUUID()),
  ).rejects.toMatchObject({ name: 'EmptyPageSyncError' });
  expect(connects).toBe(1);
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

test('openPageStore does not connect when signal is already aborted', async () => {
  const { workspace } = await createM0Workspace();
  const ac = new AbortController();
  ac.abort();
  const connected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(id: string) {
      connected.push(id);
    },
    disconnect() {},
    whenReady() {
      return new Promise(() => {});
    },
  };
  const uuid = crypto.randomUUID();
  await expect(
    openPageStore(workspace, provider, uuid, { signal: ac.signal }),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(connected).toEqual([]);
});

test('openPageStore abort during wait does not retry connect', async () => {
  const { workspace } = await createM0Workspace();
  const ac = new AbortController();
  const connected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(id: string) {
      connected.push(id);
    },
    disconnect() {},
    whenReady() {
      ac.abort();
      return new Promise(() => {});
    },
  };
  const uuid = crypto.randomUUID();
  await expect(
    openPageStore(workspace, provider, uuid, { signal: ac.signal }),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(connected).toEqual([uuid]);
});

test('openPageStore abort during backoff does not reconnect', async () => {
  const { workspace } = await createM0Workspace();
  const ac = new AbortController();
  const connected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(id: string) {
      connected.push(id);
    },
    disconnect() {},
    whenReady() {
      return Promise.reject(syncTimeoutError());
    },
  };
  const uuid = crypto.randomUUID();
  vi.useFakeTimers();
  try {
    const p = openPageStore(workspace, provider, uuid, { signal: ac.signal });
    await Promise.resolve();
    await Promise.resolve();
    expect(connected).toEqual([uuid]);
    ac.abort();
    await expect(p).rejects.toMatchObject({ name: 'AbortError' });
    await vi.advanceTimersByTimeAsync(10_000);
    expect(connected).toEqual([uuid]);
  } finally {
    vi.useRealTimers();
  }
});

test('openPageStore retries transport timeout until success', async () => {
  const { workspace } = await createM0Workspace();
  let connects = 0;
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect() {
      connects += 1;
    },
    disconnect() {},
    whenReady() {
      if (connects < OPEN_PAGE_ATTEMPTS) {
        return Promise.reject(syncTimeoutError());
      }
      return Promise.resolve();
    },
  };
  const uuid = crypto.randomUUID();
  vi.useFakeTimers();
  try {
    const p = openPageStore(workspace, provider, uuid);
    await vi.runAllTimersAsync();
    await p;
    expect(connects).toBe(OPEN_PAGE_ATTEMPTS);
  } finally {
    vi.useRealTimers();
  }
});

test('isOpenPageTransportError is timeout / ECONNREFUSED only', () => {
  expect(isOpenPageTransportError(syncTimeoutError('venus'))).toBe(true);
  expect(isOpenPageTransportError(new Error('ECONNREFUSED'))).toBe(true);
  expect(
    isOpenPageTransportError(new Error('hub websocket closed before sync')),
  ).toBe(false);
  expect(
    isOpenPageTransportError(new Error('hub websocket error')),
  ).toBe(false);
  expect(
    isOpenPageTransportError(new Error('hub websocket disconnected before sync')),
  ).toBe(false);
  expect(isOpenPageTransportError(new Error('ensure_doc hydrate'))).toBe(false);
  expect(OPEN_PAGE_RETRY_MS).toBeGreaterThanOrEqual(1_000);
});

test('openPageStore does not retry websocket close (hydrate-shaped)', async () => {
  const { workspace } = await createM0Workspace();
  const connected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(id: string) {
      connected.push(id);
    },
    disconnect() {},
    whenReady() {
      return Promise.reject(new Error('hub websocket closed before sync'));
    },
  };
  const uuid = crypto.randomUUID();
  await expect(openPageStore(workspace, provider, uuid)).rejects.toMatchObject({
    message: 'hub websocket closed before sync',
  });
  expect(connected).toEqual([uuid]);
});

test('openPageStore does not retry hub websocket error', async () => {
  const { workspace } = await createM0Workspace();
  let connects = 0;
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect() {
      connects += 1;
    },
    disconnect() {},
    whenReady() {
      return Promise.reject(new Error('hub websocket error'));
    },
  };
  await expect(
    openPageStore(workspace, provider, crypto.randomUUID()),
  ).rejects.toMatchObject({ message: 'hub websocket error' });
  expect(connects).toBe(1);
});

test('openPageStore retries ECONNREFUSED then succeeds', async () => {
  const { workspace } = await createM0Workspace();
  let connects = 0;
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect() {
      connects += 1;
      if (connects < 2) throw new Error('connect ECONNREFUSED');
    },
    disconnect() {},
    whenReady() {
      return Promise.resolve();
    },
  };
  vi.useFakeTimers();
  try {
    const p = openPageStore(workspace, provider, crypto.randomUUID());
    await vi.runAllTimersAsync();
    await p;
    expect(connects).toBe(2);
  } finally {
    vi.useRealTimers();
  }
});

test('waitUntilSynced(docId) does not wait on other sessions', async () => {
  let resolvePage: (() => void) | undefined;
  const provider: SyncProvider = {
    kind: 'venus',
    synced: false,
    connect() {},
    disconnect() {},
    whenReady(id?: string) {
      if (id === 'page-uuid') {
        return new Promise<void>((resolve) => {
          resolvePage = resolve;
        });
      }
      return new Promise(() => {});
    },
  };
  const p = waitUntilSynced(provider, undefined, 'page-uuid');
  let done = false;
  void p.then(() => {
    done = true;
  });
  await Promise.resolve();
  expect(done).toBe(false);
  expect(resolvePage).toBeTypeOf('function');
  resolvePage?.();
  await p;
  expect(done).toBe(true);
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
