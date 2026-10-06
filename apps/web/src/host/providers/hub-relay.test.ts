import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as syncProtocol from 'y-protocols/sync';
import { afterEach, expect, test } from 'vitest';
import * as Y from 'yjs';
import { HubRelay, readHubFrame, readTabFrame } from './hub-relay.js';
import { SharedWorkerSockets } from './shared-worker-socket.js';
import { VenusHubProvider } from './venus-hub-provider.js';

const URL_HOME = 'ws://hub.test/collaboration/w';
const URL_PAGE = 'ws://hub.test/collaboration/w?doc=p';

/** Hub stand-in: hello on open, step 2 reply to the asker, no self echo. */
class FakeHub {
  docs = new Map<string, Y.Doc>();
  sockets: FakeHubSocket[] = [];

  doc(url: string) {
    let doc = this.docs.get(url);
    if (!doc) {
      doc = new Y.Doc();
      const d = doc;
      d.on('update', (update: Uint8Array, origin: unknown) => {
        for (const s of this.sockets) {
          if (s.url !== url || s === origin || s.readyState !== 1) continue;
          const enc = encoding.createEncoder();
          encoding.writeVarUint(enc, 0);
          syncProtocol.writeUpdate(enc, update);
          s.deliver(encoding.toUint8Array(enc));
        }
      });
      this.docs.set(url, d);
    }
    return doc;
  }

  open(url: string) {
    return this.sockets.filter((s) => s.url === url && s.readyState <= 1);
  }
}

class FakeHubSocket {
  readyState = 0;
  binaryType = '';
  listeners = new Map<string, Array<(ev: unknown) => void>>();
  constructor(
    readonly hub: FakeHub,
    readonly url: string,
  ) {
    hub.sockets.push(this);
    setTimeout(() => this.accept(), 0);
  }
  addEventListener(type: string, fn: (ev: unknown) => void) {
    const list = this.listeners.get(type) ?? [];
    list.push(fn);
    this.listeners.set(type, list);
  }
  emit(type: string, ev: unknown = {}) {
    for (const fn of this.listeners.get(type) ?? []) fn(ev);
  }
  accept() {
    if (this.readyState !== 0) return;
    this.readyState = 1;
    this.emit('open');
    const doc = this.hub.doc(this.url);
    const aware = encoding.createEncoder();
    encoding.writeVarUint(aware, 1);
    encoding.writeVarUint8Array(aware, new Uint8Array([0]));
    this.deliver(encoding.toUint8Array(aware));
    const s1 = encoding.createEncoder();
    encoding.writeVarUint(s1, 0);
    syncProtocol.writeSyncStep1(s1, doc);
    this.deliver(encoding.toUint8Array(s1));
    const s2 = encoding.createEncoder();
    encoding.writeVarUint(s2, 0);
    syncProtocol.writeSyncStep2(s2, doc);
    this.deliver(encoding.toUint8Array(s2));
  }
  deliver(bytes: Uint8Array) {
    const data = bytes.slice().buffer;
    setTimeout(() => {
      if (this.readyState === 1) this.emit('message', { data });
    }, 0);
  }
  send(bytes: Uint8Array) {
    const doc = this.hub.doc(this.url);
    const decoder = decoding.createDecoder(bytes);
    while (decoding.hasContent(decoder)) {
      if (decoding.readVarUint(decoder) !== 0) break;
      const reply = encoding.createEncoder();
      encoding.writeVarUint(reply, 0);
      syncProtocol.readSyncMessage(decoder, reply, doc, this);
      if (encoding.length(reply) > 1) this.deliver(encoding.toUint8Array(reply));
    }
  }
  close() {
    if (this.readyState >= 2) return;
    this.readyState = 3;
    setTimeout(() => this.emit('close'), 0);
  }
  drop() {
    this.readyState = 3;
    this.emit('close');
  }
}

const cleanups: Array<() => void> = [];
afterEach(() => {
  for (const fn of cleanups.splice(0)) fn();
});

function setup() {
  const hub = new FakeHub();
  const relay = new HubRelay(
    (url) => new FakeHubSocket(hub, url) as unknown as WebSocket,
  );
  const tab = () => {
    const { port1, port2 } = new MessageChannel();
    port2.addEventListener('message', (ev) => relay.handle(port2, ev.data));
    port2.start();
    relay.attach(port2);
    const sockets = new SharedWorkerSockets(port1);
    const provider = new VenusHubProvider(URL_HOME, { sockets });
    cleanups.push(() => {
      port1.close();
      port2.close();
    });
    return { provider, sockets, port: port2 };
  };
  return { hub, relay, tab };
}

async function until(fn: () => boolean, ms = 2000) {
  const start = Date.now();
  while (!fn()) {
    if (Date.now() - start > ms) throw new Error('timed out');
    await new Promise((r) => setTimeout(r, 5));
  }
}

const text = (doc: Y.Doc) => doc.getText('t').toString();

test('two tabs share one hub socket; an update in A reaches B without a hub echo', async () => {
  const { hub, relay, tab } = setup();
  const a = tab();
  const b = tab();
  expect(a.provider.transport).toBe('shared-worker');
  const docA = new Y.Doc();
  const docB = new Y.Doc();
  a.provider.connect('doc:home', docA);
  b.provider.connect('doc:home', docB);
  await Promise.all([a.provider.whenReady(), b.provider.whenReady()]);

  expect(hub.open(URL_HOME)).toHaveLength(1);
  expect(relay.stats().sockets).toEqual([
    { url: URL_HOME, readyState: 1, channels: 2, ports: 2 },
  ]);

  docA.getText('t').insert(0, 'from A');
  await until(() => text(docB) === 'from A');
  expect(text(hub.doc(URL_HOME))).toBe('from A');

  docB.getText('t').insert(0, 'B+');
  await until(() => text(docA) === 'B+from A');
  expect(text(hub.doc(URL_HOME))).toBe('B+from A');
});

test('a tab that joins after the hello still gets the full doc and whenReady', async () => {
  const { hub, tab } = setup();
  hub.doc(URL_HOME).getText('t').insert(0, 'seeded');
  const a = tab();
  const docA = new Y.Doc();
  a.provider.connect('doc:home', docA);
  await a.provider.whenReady('doc:home');
  expect(text(docA)).toBe('seeded');

  const b = tab();
  const docB = new Y.Doc();
  b.provider.connect('doc:home', docB);
  await b.provider.whenReady('doc:home');
  expect(text(docB)).toBe('seeded');
  expect(hub.open(URL_HOME)).toHaveLength(1);

  await b.provider.nextStep2('doc:home');
  docB.getText('t').insert(0, '>');
  await until(() => text(docA) === '>seeded');
});

test('a step 2 reply goes only to the channel that sent the step 1', async () => {
  const { relay, hub } = setup();
  const got: Record<string, Array<{ t: string; ch: number }>> = { p1: [], p2: [] };
  const port = (name: string) => ({
    postMessage(msg: { t: string; ch: number }) {
      got[name].push(msg);
    },
  });
  const p1 = port('p1');
  const p2 = port('p2');
  relay.attach(p1);
  relay.attach(p2);
  relay.handle(p1, { t: 'open', ch: 1, url: URL_PAGE });
  relay.handle(p2, { t: 'open', ch: 1, url: URL_PAGE });
  await until(() => got.p1.some((m) => m.t === 'open'));
  await until(() =>
    got.p2.filter((m) => m.t === 'message').length >= 3,
  );
  const before = { p1: got.p1.length, p2: got.p2.length };

  const enc = encoding.createEncoder();
  encoding.writeVarUint(enc, 0);
  syncProtocol.writeSyncStep1(enc, new Y.Doc());
  relay.handle(p1, { t: 'send', ch: 1, data: encoding.toUint8Array(enc) });
  await until(() => got.p1.length > before.p1);
  await new Promise((r) => setTimeout(r, 20));
  expect(got.p2.length).toBe(before.p2);
  expect(hub.open(URL_PAGE)).toHaveLength(1);
});

test('last channel close closes the hub socket; one tab leaving keeps it', async () => {
  const { hub, relay, tab } = setup();
  const a = tab();
  const b = tab();
  a.provider.connect('doc:home', new Y.Doc());
  b.provider.connect('doc:home', new Y.Doc());
  await Promise.all([a.provider.whenReady(), b.provider.whenReady()]);

  a.provider.disconnect('doc:home');
  await until(() => relay.stats().sockets[0]?.channels === 1);
  expect(hub.open(URL_HOME)).toHaveLength(1);

  relay.detachPort(b.port);
  await until(() => hub.open(URL_HOME).length === 0);
  expect(relay.stats().sockets).toEqual([]);
});

test('a hub drop closes every channel and providers reconnect through the worker', async () => {
  const { hub, tab } = setup();
  const a = tab();
  const b = tab();
  const docA = new Y.Doc();
  const docB = new Y.Doc();
  a.provider.connect('doc:home', docA);
  b.provider.connect('doc:home', docB);
  await Promise.all([a.provider.whenReady(), b.provider.whenReady()]);

  hub.open(URL_HOME)[0]?.drop();
  await until(() => a.provider.connection === 'reconnecting');
  docA.getText('t').insert(0, 'offline');
  await until(
    () =>
      a.provider.connection === 'synced' && b.provider.connection === 'synced',
    5000,
  );
  await until(() => text(docB) === 'offline');
  expect(hub.open(URL_HOME)).toHaveLength(1);
  expect(text(hub.doc(URL_HOME))).toBe('offline');
});

test('a worker that fails to load falls back to per-tab WebSockets', () => {
  const { port1 } = new MessageChannel();
  cleanups.push(() => port1.close());
  let onError = () => {};
  const sockets = new SharedWorkerSockets(port1, {
    addEventListener(type: string, fn: () => void) {
      if (type === 'error') onError = fn;
    },
  });
  const provider = new VenusHubProvider(URL_HOME, { sockets });
  expect(provider.transport).toBe('shared-worker');
  onError();
  expect(provider.transport).toBe('tab');
});

test('frame readers: tab step 1 count and updates; hub step 2 payload', () => {
  const doc = new Y.Doc();
  doc.getText('t').insert(0, 'x');
  const s1 = encoding.createEncoder();
  encoding.writeVarUint(s1, 0);
  syncProtocol.writeSyncStep1(s1, doc);
  expect(readTabFrame(encoding.toUint8Array(s1))).toEqual({
    step1: 1,
    updates: [],
  });
  const up = encoding.createEncoder();
  encoding.writeVarUint(up, 0);
  syncProtocol.writeUpdate(up, Y.encodeStateAsUpdate(doc));
  expect(readTabFrame(encoding.toUint8Array(up)).updates).toHaveLength(1);
  const s2 = encoding.createEncoder();
  encoding.writeVarUint(s2, 0);
  syncProtocol.writeSyncStep2(s2, doc);
  const frame = readHubFrame(encoding.toUint8Array(s2));
  expect(frame.kind).toBe('step2');
  expect(readHubFrame(new Uint8Array([1, 1, 0])).kind).toBe('other');
});
