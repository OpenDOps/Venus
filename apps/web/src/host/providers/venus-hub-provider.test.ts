import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as syncProtocol from 'y-protocols/sync';
import * as Y from 'yjs';
import { expect, test, vi } from 'vitest';
import { CATALOG_GUID, CATALOG_SQL_ID } from '../ids.js';
import {
  RECONNECT_CAP_MS,
  VenusHubProvider,
  reconnectDelayMs,
} from './venus-hub-provider.js';

type ConfirmSession = {
  ws: { send: (buf: Uint8Array) => void };
  ydoc: Y.Doc;
};

type Probe = VenusHubProvider & {
  _sessions: Map<string, ConfirmSession>;
  _handleBinary(
    ws: ConfirmSession['ws'],
    ydoc: Y.Doc,
    buf: Uint8Array,
    session: ConfirmSession,
  ): void;
};

function probe(provider: VenusHubProvider) {
  return provider as Probe;
}

function step2Frame(doc: Y.Doc) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, 0);
  syncProtocol.writeSyncStep2(encoder, doc);
  return encoding.toUint8Array(encoder);
}

function installSession(provider: VenusHubProvider, docId: string) {
  const ydoc = new Y.Doc();
  const sent: Uint8Array[] = [];
  const session = {
    gen: 1,
    ws: {
      readyState: WebSocket.OPEN,
      send(buf: Uint8Array) {
        sent.push(buf);
      },
      close() {},
    },
    ydoc,
    onUpdate: () => {},
    ready: Promise.resolve(),
    resolveReady: () => {},
    rejectReady: () => {},
    synced: true,
    confirm: null,
  };
  probe(provider)._sessions.set(docId, session);
  return { ydoc, sent, session };
}

test('confirmApplied asks with an empty state vector and rejects a tiny step 2', async () => {
  const provider = new VenusHubProvider(
    'ws://127.0.0.1:9/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00',
  );
  const { ydoc, sent, session } = installSession(provider, 'page');
  const pending = provider.confirmApplied('page');
  expect(sent).toHaveLength(1);
  const decoder = decoding.createDecoder(sent[0]);
  expect(decoding.readVarUint(decoder)).toBe(0);
  expect(decoding.readVarUint(decoder)).toBe(syncProtocol.messageYjsSyncStep1);
  const sv = decoding.readVarUint8Array(decoder);
  expect(sv).toEqual(Y.encodeStateVector(new Y.Doc()));

  probe(provider)._handleBinary(session.ws, ydoc, step2Frame(new Y.Doc()), session);
  await expect(pending).rejects.toThrow(/did not apply/);
});

test('confirmApplied resolves when the step 2 is a seeded doc', async () => {
  const provider = new VenusHubProvider(
    'ws://127.0.0.1:9/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00',
  );
  const { ydoc, session } = installSession(provider, 'page');
  const seeded = new Y.Doc();
  seeded.getText('t').insert(0, 'x'.repeat(80));
  const pending = provider.confirmApplied('page');
  probe(provider)._handleBinary(session.ws, ydoc, step2Frame(seeded), session);
  await pending;
  seeded.destroy();
});

test('nextStep2 accepts the empty handshake reply before confirmApplied', async () => {
  const provider = new VenusHubProvider(
    'ws://127.0.0.1:9/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00',
  );
  const { ydoc, session } = installSession(provider, 'page');
  const drained = provider.nextStep2('page');
  probe(provider)._handleBinary(session.ws, ydoc, step2Frame(new Y.Doc()), session);
  await drained;

  const pending = provider.confirmApplied('page');
  probe(provider)._handleBinary(session.ws, ydoc, step2Frame(new Y.Doc()), session);
  await expect(pending).rejects.toThrow(/did not apply/);
});

test('disconnect rejects an in-flight page-seed confirm', async () => {
  const provider = new VenusHubProvider(
    'ws://127.0.0.1:9/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00',
  );
  installSession(provider, 'page');
  const pending = provider.confirmApplied('page');
  provider.disconnect('page');
  await expect(pending).rejects.toThrow(/disconnected/);
});

test('reconnect delay is jittered and capped', () => {
  expect(reconnectDelayMs(0, () => 0)).toBe(125);
  expect(reconnectDelayMs(0, () => 1)).toBe(250);
  expect(reconnectDelayMs(2, () => 1)).toBe(1000);
  expect(reconnectDelayMs(20, () => 0)).toBe(RECONNECT_CAP_MS / 2);
  expect(reconnectDelayMs(20, () => 1)).toBe(RECONNECT_CAP_MS);
});

class StubSocket {
  static CONNECTING = 0;
  static OPEN = 1;
  static CLOSING = 2;
  static CLOSED = 3;
  readyState = 0;
  binaryType = '';
  url = '';
  sent: Uint8Array[] = [];
  listeners: Record<string, Array<(ev?: { data?: unknown }) => void>> = {};

  constructor(url: string) {
    this.url = url;
    stubSockets.push(this);
  }

  addEventListener(type: string, fn: (ev?: { data?: unknown }) => void) {
    (this.listeners[type] ??= []).push(fn);
  }

  send(buf: Uint8Array) {
    this.sent.push(buf);
  }

  close() {
    this.readyState = StubSocket.CLOSED;
    for (const fn of this.listeners.close ?? []) fn();
  }

  open() {
    this.readyState = StubSocket.OPEN;
    for (const fn of this.listeners.open ?? []) fn();
  }

  serverMessage(data: Uint8Array) {
    for (const fn of this.listeners.message ?? []) fn({ data });
  }

  fail() {
    for (const fn of this.listeners.error ?? []) fn();
  }
}

const stubSockets: StubSocket[] = [];
const HUB = 'ws://127.0.0.1:9/collaboration/77e4a2b1-8b40-5979-a73c-fd4477216d00';
const PAGE = 'aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee';

function withSockets(run: () => Promise<void>) {
  const prev = globalThis.WebSocket;
  stubSockets.length = 0;
  vi.useFakeTimers();
  globalThis.WebSocket = StubSocket as unknown as typeof WebSocket;
  return run().finally(() => {
    globalThis.WebSocket = prev;
    vi.useRealTimers();
  });
}

function handshake(socket: StubSocket) {
  socket.open();
  socket.serverMessage(step2Frame(new Y.Doc()));
}

test('a dropped socket reconnects and sends edits made while it was down', async () => {
  await withSockets(async () => {
    const provider = new VenusHubProvider(HUB);
    const ydoc = new Y.Doc();
    const states: string[] = [];
    provider.on('connection', () => {
      states.push(provider.connection);
    });
    try {
      provider.connect(PAGE, ydoc);
      expect(stubSockets[0]?.url).toContain(`?doc=${PAGE}`);
      handshake(stubSockets[0]);
      await provider.whenReady(PAGE);
      expect(provider.synced).toBe(true);
      expect(provider.connection).toBe('synced');

      stubSockets[0].sent.length = 0;
      ydoc.getText('t').insert(0, 'live');
      expect(stubSockets[0].sent).toHaveLength(1);

      stubSockets[0].close();
      expect(provider.synced).toBe(false);
      expect(provider.connection).toBe('reconnecting');
      expect(states).toContain('reconnecting');

      const base = Y.encodeStateAsUpdate(ydoc);
      ydoc.getText('t').insert(4, '-offline');
      expect(stubSockets).toHaveLength(1);

      await vi.advanceTimersByTimeAsync(RECONNECT_CAP_MS);
      expect(stubSockets).toHaveLength(2);
      expect(stubSockets[1]?.url).toBe(stubSockets[0]?.url);
      stubSockets[1].open();

      const step1 = decoding.createDecoder(stubSockets[1].sent[0]);
      expect(decoding.readVarUint(step1)).toBe(0);
      expect(decoding.readVarUint(step1)).toBe(syncProtocol.messageYjsSyncStep1);
      expect(decoding.readVarUint8Array(step1)).toEqual(Y.encodeStateVector(ydoc));

      const updateFrame = decoding.createDecoder(stubSockets[1].sent[1]);
      decoding.readVarUint(updateFrame);
      decoding.readVarUint(updateFrame);
      const copy = new Y.Doc();
      Y.applyUpdate(copy, base);
      Y.applyUpdate(copy, decoding.readVarUint8Array(updateFrame));
      expect(copy.getText('t').toString()).toBe('live-offline');
      copy.destroy();

      stubSockets[1].serverMessage(step2Frame(ydoc));
      expect(provider.synced).toBe(true);
      expect(provider.connection).toBe('synced');
    } finally {
      provider.disconnect(PAGE);
      ydoc.destroy();
    }
  });
});

test('close before the first sync does not reconnect', async () => {
  await withSockets(async () => {
    const provider = new VenusHubProvider(HUB);
    const ydoc = new Y.Doc();
    provider.connect(PAGE, ydoc);
    const ready = provider.whenReady(PAGE);
    stubSockets[0].close();
    await expect(ready).rejects.toThrow(/closed before sync/);
    await vi.advanceTimersByTimeAsync(RECONNECT_CAP_MS);
    expect(stubSockets).toHaveLength(1);
    expect(provider.connection).toBe('synced');
    provider.disconnect(PAGE);
    ydoc.destroy();
  });
});

test('disconnect cancels a scheduled reconnect', async () => {
  await withSockets(async () => {
    const provider = new VenusHubProvider(HUB);
    const ydoc = new Y.Doc();
    provider.connect(PAGE, ydoc);
    handshake(stubSockets[0]);
    await provider.whenReady(PAGE);
    stubSockets[0].close();
    provider.disconnect(PAGE);
    await vi.advanceTimersByTimeAsync(RECONNECT_CAP_MS);
    expect(stubSockets).toHaveLength(1);
    ydoc.destroy();
  });
});

test('error and close share one reconnect', async () => {
  await withSockets(async () => {
    const provider = new VenusHubProvider(HUB);
    const ydoc = new Y.Doc();
    provider.connect(CATALOG_GUID, ydoc);
    expect(stubSockets[0]?.url).toContain(`?doc=${CATALOG_SQL_ID}`);
    handshake(stubSockets[0]);
    await provider.whenReady(CATALOG_GUID);
    stubSockets[0].fail();
    stubSockets[0].close();
    await vi.advanceTimersByTimeAsync(RECONNECT_CAP_MS);
    expect(stubSockets).toHaveLength(2);
    expect(stubSockets[1]?.url).toBe(stubSockets[0]?.url);
    provider.disconnect(CATALOG_GUID);
    ydoc.destroy();
  });
});

test('one page dropping does not reconnect the sockets that are still open', async () => {
  await withSockets(async () => {
    const provider = new VenusHubProvider(HUB);
    const home = new Y.Doc();
    const page = new Y.Doc();
    provider.connect('doc:home', home);
    provider.connect(PAGE, page);
    handshake(stubSockets[0]);
    handshake(stubSockets[1]);
    await provider.whenReady();
    stubSockets[1].close();
    await vi.advanceTimersByTimeAsync(RECONNECT_CAP_MS);
    expect(stubSockets).toHaveLength(3);
    expect(stubSockets[0]?.readyState).toBe(StubSocket.OPEN);
    expect(stubSockets[2]?.url).toBe(stubSockets[1]?.url);
    provider.disconnect('doc:home');
    provider.disconnect(PAGE);
    home.destroy();
    page.destroy();
  });
});
