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

const hostDir = dirname(fileURLToPath(import.meta.url));

const importOf = (pkg: string) =>
  new RegExp(`(?:from|import)\\s+['"]${pkg}(?:/[^'"]*)?['"]`);

test('default provider is memory no-op and connects after load', () => {
  const { docId, store, provider } = createM0Workspace();
  expect(provider).toBeInstanceOf(MemoryNoopProvider);
  expect(provider.kind).toBe('memory');
  expect(provider.synced).toBe(true);
  expect(docId).toBe('doc:home');
  expect(store.spaceDoc.guid).toBeTruthy();
});

test('a second SyncProvider can be passed without touching mount-editor', () => {
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

  const { docId, store, provider: used } = createM0Workspace(provider);
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
  }
});

test('Env switch: unset VITE_SYNC_URL is memory and constructs no WebSocket', () => {
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

    const { provider } = createM0Workspace();
    expect(provider.kind).toBe('memory');
    expect(constructed).toEqual([]);
  } finally {
    globalThis.WebSocket = Ws;
  }
});

test('Env switch: VITE_SYNC_URL selects octobase without opening a socket until connect', () => {
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
    const url = 'ws://127.0.0.1:3000/collaboration/venus-m0';
    const fromEnv = providerFromEnv({ VITE_SYNC_URL: url });
    expect(fromEnv).toBeInstanceOf(OctoBaseKeckProvider);
    expect(fromEnv.kind).toBe('octobase');
    expect(constructed).toEqual([]);

    fromEnv.connect('doc:home', createM0Workspace().store.spaceDoc);
    expect(constructed).toEqual([[url, ['AFFiNE']]]);
    fromEnv.disconnect('doc:home');
  } finally {
    globalThis.WebSocket = Ws;
  }
});
