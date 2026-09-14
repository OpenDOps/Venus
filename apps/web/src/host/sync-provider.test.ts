import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import type { Doc } from 'yjs';
import type { SyncProvider } from './sync-provider.js';
import { MemoryNoopProvider } from './sync-provider.js';
import { providerFromEnv } from './providers/from-env.js';
import { OctoBaseKeckProvider } from './providers/octobase-keck-provider.js';
import { createM0Workspace } from './workspace.js';
import { COLLABORATION_PATH, PAGE_DOC_ID } from './ids.js';

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
    expect(src, name).not.toMatch(/octobase-keck-provider/);
    expect(src, name).not.toMatch(/from-env/);
    expect(src, name).not.toMatch(/providers\//);
    expect(src, name).not.toMatch(/venus-hub/);
    expect(src, name).not.toMatch(/blob-source/);
    expect(src, name).not.toMatch(/venus-sidecar/);
    expect(src, name).not.toMatch(/\/git\/log|venus-git-log/);
    expect(src, name).not.toMatch(/from-doc\.js|pin-from-doc/);
    expect(src, name).not.toMatch(/git2|simple-git|isomorphic-git/);
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

test('Env switch: VITE_SYNC_URL selects octobase without opening a socket until connect', async () => {
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
    expect(fromEnv).toBeInstanceOf(OctoBaseKeckProvider);
    expect(fromEnv.kind).toBe('octobase');
    expect(constructed).toEqual([]);

    fromEnv.connect(PAGE_DOC_ID, (await createM0Workspace()).store.spaceDoc);
    expect(constructed).toEqual([[url, ['AFFiNE']]]);
    fromEnv.disconnect(PAGE_DOC_ID);
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
    expect(fromEnv).toBeInstanceOf(OctoBaseKeckProvider);
    expect(fromEnv).toMatchObject({
      kind: 'octobase',
      url: `ws://127.0.0.1:8080${COLLABORATION_PATH}`,
    });
  } finally {
    if (desc) Object.defineProperty(globalThis, 'location', desc);
    else Reflect.deleteProperty(globalThis, 'location');
  }
});
