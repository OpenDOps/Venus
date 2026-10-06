import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';
import { MemoryNoopProvider } from './sync-provider.js';
import { providerFromEnv } from './providers/from-env.js';
import { VenusHubProvider } from './providers/venus-hub-provider.js';
import { createM0Workspace } from './workspace.js';
import { COLLABORATION_PATH, PAGE_DOC_ID, PAGE_SQL_ID, CATALOG_GUID, CATALOG_SQL_ID, sqlIdForDoc, collaborationSocketUrl } from './ids.js';

const hostDir = dirname(fileURLToPath(import.meta.url));

const importOf = (pkg: string) =>
  new RegExp(`(?:from|import)\\s+['"]${pkg}(?:/[^'"]*)?['"]`);

test('default provider is memory no-op and connects before seed', async () => {
  const { docId, store, provider } = await createM0Workspace();
  expect(provider).toBeInstanceOf(MemoryNoopProvider);
  expect(provider.kind).toBe('memory');
  expect(provider.synced).toBe(true);
  expect(docId).toBe(PAGE_DOC_ID);
  expect(store.spaceDoc.guid).toBeTruthy();
});

test('a second SyncProvider can be passed without touching mount-editor', async () => {
  const calls: { op: string; docId: string; ydoc?: Doc }[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: true,
    connect(id, ydoc) {
      calls.push({ op: 'connect', docId: id, ydoc });
    },
    disconnect(id) {
      calls.push({ op: 'disconnect', docId: id });
    },
    whenReady() {
      return Promise.resolve();
    },
  };

  const { docId, store, provider: used } = await createM0Workspace(provider);
  expect(used).toBe(provider);
  expect(calls).toEqual([{ op: 'connect', docId, ydoc: store.spaceDoc }]);

  used.disconnect(docId);
  expect(calls).toEqual([
    { op: 'connect', docId, ydoc: store.spaceDoc },
    { op: 'disconnect', docId },
  ]);

  const mountEditor = readFileSync(join(hostDir, 'mount-editor.js'), 'utf8');
  expect(mountEditor).not.toMatch(
    /sync-provider|SyncProvider|OctoBase|octobase/i,
  );
});

test('Seam holds: editor host files do not import live sync clients', () => {
  const files = ['mount-editor.js', 'editor-container.js', 'boot.js'];
  for (const name of files) {
    const src = readFileSync(join(hostDir, name), 'utf8');
    expect(src, name).not.toMatch(importOf('octobase'));
    expect(src, name).not.toMatch(importOf('y-websocket'));
    expect(src, name).not.toMatch(importOf('y-indexeddb'));
    expect(src, name).not.toMatch(importOf('hocuspocus'));
    expect(src, name).not.toMatch(importOf('y-protocols'));
    expect(src, name).not.toMatch(importOf('lib0'));
    expect(src, name).not.toMatch(/venus-hub-provider/);
    expect(src, name).not.toMatch(/from-env/);
    expect(src, name).not.toMatch(/providers\//);
    expect(src, name).not.toMatch(/venus-hub/);
    expect(src, name).not.toMatch(/blob-source/);
    expect(src, name).not.toMatch(/venus-sidecar/);
    expect(src, name).not.toMatch(/\/git\/log|venus-git-log/);
    expect(src, name).not.toMatch(/from-doc\.js|pin-from-doc/);
    expect(src, name).not.toMatch(/git2|simple-git|isomorphic-git/);
    expect(src, name).not.toMatch(/catalog\//);
    expect(src, name).not.toMatch(/CatalogTree/);
    expect(src, name).not.toMatch(/SharedWorker/);
    expect(src, name).not.toMatch(/hub-shared-worker|shared-worker-socket|hub-relay/);
    expect(src, name).not.toMatch(/serviceWorker/);
  }
});

test('Seam holds: the hub worker relays bytes and never holds a Y.Doc', () => {
  const providers = join(hostDir, 'providers');
  for (const name of ['hub-shared-worker.js', 'hub-relay.js']) {
    const src = readFileSync(join(providers, name), 'utf8');
    expect(src, name).not.toMatch(importOf('yjs'));
    expect(src, name).not.toMatch(importOf('y-protocols'));
    expect(src, name).not.toMatch(importOf('@blocksuite'));
    expect(src, name).not.toMatch(/serviceWorker/);
  }
  const fromEnv = readFileSync(join(providers, 'from-env.js'), 'utf8');
  expect(fromEnv).not.toMatch(/serviceWorker/);
});

test('Memory has no worker: unset VITE_SYNC_URL constructs no SharedWorker', async () => {
  const constructed: unknown[] = [];
  const desc = Object.getOwnPropertyDescriptor(globalThis, 'SharedWorker');
  Object.defineProperty(globalThis, 'SharedWorker', {
    configurable: true,
    value: class {
      constructor(...args: unknown[]) {
        constructed.push(args);
        throw new Error('SharedWorker must not start without VITE_SYNC_URL');
      }
    },
  });
  try {
    expect(providerFromEnv({})).toBeInstanceOf(MemoryNoopProvider);
    const { provider } = await createM0Workspace();
    expect(provider.kind).toBe('memory');
    expect(constructed).toEqual([]);
  } finally {
    if (desc) Object.defineProperty(globalThis, 'SharedWorker', desc);
    else Reflect.deleteProperty(globalThis, 'SharedWorker');
  }
});

test('Fallback: no SharedWorker, or one that throws, is the per-tab socket path', () => {
  const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
  expect(typeof (globalThis as { SharedWorker?: unknown }).SharedWorker).toBe(
    'undefined',
  );
  const plain = providerFromEnv({ VITE_SYNC_URL: url });
  expect(plain).toBeInstanceOf(VenusHubProvider);
  expect((plain as VenusHubProvider).transport).toBe('tab');

  Object.defineProperty(globalThis, 'SharedWorker', {
    configurable: true,
    value: class {
      constructor() {
        throw new Error('blocked by policy');
      }
    },
  });
  try {
    const threw = providerFromEnv({ VITE_SYNC_URL: url });
    expect((threw as VenusHubProvider).transport).toBe('tab');
  } finally {
    Reflect.deleteProperty(globalThis, 'SharedWorker');
  }
});

test('SharedWorker present: sessions open worker channels, not tab WebSockets', () => {
  const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
  const workers: Array<{ name: string; posted: unknown[] }> = [];
  Object.defineProperty(globalThis, 'SharedWorker', {
    configurable: true,
    value: class {
      port: MessagePort;
      constructor(_script: URL, options: { name: string; type: string }) {
        const posted: unknown[] = [];
        workers.push({ name: options.name, posted });
        this.port = {
          addEventListener() {},
          start() {},
          postMessage(msg: unknown) {
            posted.push(msg);
          },
        } as unknown as MessagePort;
      }
      addEventListener() {}
    },
  });
  const Ws = globalThis.WebSocket;
  const tabSockets: unknown[] = [];
  // @ts-expect-error stub
  globalThis.WebSocket = class {
    constructor(...args: unknown[]) {
      tabSockets.push(args);
    }
  };
  try {
    const provider = providerFromEnv({ VITE_SYNC_URL: url }) as VenusHubProvider;
    expect(provider.transport).toBe('shared-worker');
    expect(workers.map((w) => w.name)).toEqual([`venus-hub:${url}`]);
    provider.connect(CATALOG_GUID, { on() {}, off() {} } as unknown as Doc);
    expect(tabSockets).toEqual([]);
    expect(workers[0]?.posted).toEqual([
      { t: 'open', ch: 1, url: `${url}?doc=${CATALOG_SQL_ID}` },
    ]);
    provider.disconnect(CATALOG_GUID);
  } finally {
    globalThis.WebSocket = Ws;
    Reflect.deleteProperty(globalThis, 'SharedWorker');
  }
});

test('Env switch: unset VITE_SYNC_URL is memory and constructs no WebSocket', async () => {
  const Ws = globalThis.WebSocket;
  const constructed: unknown[] = [];
  // @ts-expect-error stub
  globalThis.WebSocket = class {
    constructor(...args: unknown[]) {
      constructed.push(args);
      throw new Error('WebSocket must not open without VITE_SYNC_URL');
    }
  };

  try {
    const fromEnv = providerFromEnv({});
    expect(fromEnv).toBeInstanceOf(MemoryNoopProvider);
    expect(fromEnv.kind).toBe('memory');

    const { provider } = await createM0Workspace();
    expect(provider.kind).toBe('memory');
    expect(constructed).toEqual([]);
  } finally {
    globalThis.WebSocket = Ws;
  }
});

test('Env switch: VITE_SYNC_URL selects venus without opening a socket until connect', async () => {
  const Ws = globalThis.WebSocket;
  const constructed: unknown[] = [];
  class StubSocket {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    readyState = 0;
    binaryType = '';
    constructor(...args: unknown[]) {
      constructed.push(args);
    }
    addEventListener() {}
    close() {
      this.readyState = 3;
    }
  }
  // @ts-expect-error stub
  globalThis.WebSocket = StubSocket;

  try {
    const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
    const fromEnv = providerFromEnv({ VITE_SYNC_URL: url });
    expect(fromEnv).toBeInstanceOf(VenusHubProvider);
    if (!(fromEnv instanceof VenusHubProvider)) {
      throw new Error('expected VenusHubProvider');
    }
    expect(fromEnv.kind).toBe('venus');
    expect(constructed).toEqual([]);

    fromEnv.connect(PAGE_DOC_ID, (await createM0Workspace()).store.spaceDoc);
    expect(constructed).toEqual([[url, ['AFFiNE']]]);
    fromEnv.connect(CATALOG_GUID, new (await import('yjs')).Doc());
    expect(constructed).toEqual([
      [url, ['AFFiNE']],
      [`${url}?doc=${CATALOG_SQL_ID}`, ['AFFiNE']],
    ]);
    expect(fromEnv._gen).toBe(2);
    fromEnv.disconnect(PAGE_DOC_ID);
    fromEnv.disconnect(CATALOG_GUID);
  } finally {
    globalThis.WebSocket = Ws;
  }
});

test('Env switch: same-origin needs location.host and does not open a socket yet', () => {
  expect(() => providerFromEnv({ VITE_SYNC_URL: 'same-origin' })).toThrow(
    /window\.location/,
  );

  const desc = Object.getOwnPropertyDescriptor(globalThis, 'location');
  Object.defineProperty(globalThis, 'location', {
    configurable: true,
    value: { protocol: 'http:', host: '127.0.0.1:8080' },
  });
  try {
    const fromEnv = providerFromEnv({ VITE_SYNC_URL: 'same-origin' });
    expect(fromEnv).toBeInstanceOf(VenusHubProvider);
    expect(fromEnv).toMatchObject({
      kind: 'venus',
      url: `ws://127.0.0.1:8080${COLLABORATION_PATH}`,
    });
  } finally {
    if (desc) Object.defineProperty(globalThis, 'location', desc);
    else Reflect.deleteProperty(globalThis, 'location');
  }
});

test('wire A: ?doc= is SQL uuid; home omits the query', () => {
  const base = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
  expect(sqlIdForDoc(PAGE_DOC_ID)).toBe(PAGE_SQL_ID);
  expect(sqlIdForDoc(PAGE_SQL_ID.toUpperCase())).toBe(PAGE_SQL_ID);
  expect(sqlIdForDoc(CATALOG_GUID)).toBe(CATALOG_SQL_ID);
  expect(collaborationSocketUrl(base, PAGE_DOC_ID)).toBe(base);
  expect(collaborationSocketUrl(`${base}?doc=stale`, PAGE_DOC_ID)).toBe(base);
  expect(collaborationSocketUrl(base, CATALOG_GUID)).toBe(
    `${base}?doc=${CATALOG_SQL_ID}`,
  );
  expect(() => sqlIdForDoc('doc:protocol')).toThrow(/SQL uuid/);
});

test('venus hub _gen is per session; disconnect one keeps the other socket', () => {
  const sockets: Array<{ closed: boolean }> = [];
  class StubWs {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    readyState = 1;
    binaryType = '';
    closed = false;
    listeners: Record<string, Array<() => void>> = {};
    constructor(..._args: unknown[]) {
      sockets.push(this);
    }
    addEventListener(type: string, fn: () => void) {
      (this.listeners[type] ??= []).push(fn);
    }
    close() {
      this.readyState = 3;
      this.closed = true;
      for (const fn of this.listeners.close ?? []) fn();
    }
  }
  const Ws = globalThis.WebSocket;
  // @ts-expect-error stub
  globalThis.WebSocket = StubWs;
  try {
    const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
    const provider = new VenusHubProvider(url);
    const homeDoc = { on() {}, off() {} } as unknown as Doc;
    const pageDoc = { on() {}, off() {} } as unknown as Doc;
    const page = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
    provider.connect(PAGE_DOC_ID, homeDoc);
    provider.connect(page, pageDoc);
    expect(provider._gen).toBe(2);
    expect(sockets).toHaveLength(2);
    provider.disconnect(PAGE_DOC_ID);
    expect(sockets[0]?.closed).toBe(true);
    expect(sockets[1]?.closed).toBe(false);
    provider.disconnect(page);
  } finally {
    globalThis.WebSocket = Ws;
  }
});

test('whenReady(docId) does not fail when another session closes', async () => {
  const sockets: Array<{
    closed: boolean;
    listeners: Record<string, Array<() => void>>;
    close: () => void;
  }> = [];
  class StubWs {
    static CONNECTING = 0;
    static OPEN = 1;
    static CLOSING = 2;
    static CLOSED = 3;
    readyState = 1;
    binaryType = '';
    closed = false;
    listeners: Record<string, Array<() => void>> = {};
    constructor(..._args: unknown[]) {
      sockets.push(this);
    }
    addEventListener(type: string, fn: () => void) {
      (this.listeners[type] ??= []).push(fn);
    }
    close() {
      this.readyState = 3;
      this.closed = true;
      for (const fn of this.listeners.close ?? []) fn();
    }
  }
  const Ws = globalThis.WebSocket;
  // @ts-expect-error stub
  globalThis.WebSocket = StubWs;
  try {
    const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
    const provider = new VenusHubProvider(url);
    const homeDoc = { on() {}, off() {} } as unknown as Doc;
    const pageDoc = { on() {}, off() {} } as unknown as Doc;
    const page = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';
    provider.connect(PAGE_DOC_ID, homeDoc);
    provider.connect(page, pageDoc);
    const pageReady = provider.whenReady(page);
    const allReady = provider.whenReady();
    sockets[0]?.close();
    await expect(allReady).rejects.toMatchObject({
      message: 'hub websocket closed before sync',
    });
    let pageSettled = false;
    void pageReady.then(
      () => {
        pageSettled = true;
      },
      () => {
        pageSettled = true;
      },
    );
    await Promise.resolve();
    expect(pageSettled).toBe(false);
    sockets[1]?.close();
    await expect(pageReady).rejects.toMatchObject({
      message: 'hub websocket closed before sync',
    });
  } finally {
    globalThis.WebSocket = Ws;
  }
});
