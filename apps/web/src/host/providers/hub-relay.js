/**
 * Fan-in of wire A sockets for the hub SharedWorker. One WebSocket per URL
 * (`/collaboration/:workspace` or `?doc=`); each tab session is a channel on
 * that socket. Frames stay vanilla y-protocols; this file never decodes Yjs
 * and never holds a `Y.Doc`.
 *
 * The hub does not echo a client's own updates, so an update a channel
 * sends is also posted to its sibling channels. The hub answers each
 * sync step 1 with a step 2 to that socket only, in order: replies go to the
 * channel that asked. The first step 2 after open is the hello (full doc)
 * and goes to every channel attached by then. A channel that joins after
 * the hello gets its own full step 2 by asking with an empty state vector.
 */
import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';

const MSG_SYNC = 0;
const SYNC_STEP1 = 0;
const SYNC_STEP2 = 1;
const SYNC_UPDATE = 2;
const WS_OPEN = 1;

/** Sync step 1 with an empty state vector: the hub replies with the full doc. */
function emptyStep1Frame() {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, MSG_SYNC);
  encoding.writeVarUint(encoder, SYNC_STEP1);
  encoding.writeVarUint8Array(encoder, new Uint8Array([0]));
  return encoding.toUint8Array(encoder);
}

/** @param {Uint8Array} update */
function updateFrame(update) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, MSG_SYNC);
  encoding.writeVarUint(encoder, SYNC_UPDATE);
  encoding.writeVarUint8Array(encoder, update);
  return encoding.toUint8Array(encoder);
}

/**
 * Sync step 1 count and step 2 / update payloads in a tab frame.
 *
 * @param {Uint8Array} bytes
 */
export function readTabFrame(bytes) {
  const decoder = decoding.createDecoder(bytes);
  let step1 = 0;
  /** @type {Uint8Array[]} */
  const updates = [];
  try {
    while (decoding.hasContent(decoder)) {
      if (decoding.readVarUint(decoder) !== MSG_SYNC) break;
      const syncType = decoding.readVarUint(decoder);
      const payload = decoding.readVarUint8Array(decoder);
      if (syncType === SYNC_STEP1) step1 += 1;
      else if (syncType === SYNC_STEP2 || syncType === SYNC_UPDATE) {
        updates.push(payload);
      }
    }
  } catch {
    // Truncated frame: forward as-is; the hub logs it.
  }
  return { step1, updates };
}

/**
 * `step2` with its payload, or the first message kind of a hub frame.
 *
 * @param {Uint8Array} bytes
 * @returns {{ kind: 'step1' | 'update' | 'other' } | { kind: 'step2', update: Uint8Array }}
 */
export function readHubFrame(bytes) {
  try {
    const decoder = decoding.createDecoder(bytes);
    if (decoding.readVarUint(decoder) !== MSG_SYNC) return { kind: 'other' };
    const syncType = decoding.readVarUint(decoder);
    if (syncType === SYNC_STEP1) return { kind: 'step1' };
    if (syncType === SYNC_UPDATE) return { kind: 'update' };
    if (syncType === SYNC_STEP2) {
      return { kind: 'step2', update: decoding.readVarUint8Array(decoder) };
    }
  } catch {
    // fall through
  }
  return { kind: 'other' };
}

/** @param {unknown} data */
function asBytes(data) {
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  return null;
}

/**
 * @typedef {{ postMessage: (msg: unknown, transfer?: Transferable[]) => void }} RelayPort
 *
 * @typedef {{
 *   port: RelayPort,
 *   ch: number,
 *   shared: Shared,
 * }} Channel
 *
 * @typedef {{
 *   url: string,
 *   ws: WebSocket,
 *   channels: Set<Channel>,
 *   helloPending: boolean,
 *   helloTo: Set<Channel>,
 *   replyTo: Array<Channel | null>,
 * }} Shared
 */

export class HubRelay {
  /**
   * @param {(url: string) => WebSocket} createSocket
   */
  constructor(createSocket) {
    this._createSocket = createSocket;
    /** @type {Map<string, Shared>} */
    this._sockets = new Map();
    /** @type {Map<RelayPort, Map<number, Channel>>} */
    this._ports = new Map();
  }

  /** @param {RelayPort} port */
  attach(port) {
    if (!this._ports.has(port)) this._ports.set(port, new Map());
    port.postMessage({ t: 'ready' });
  }

  /**
   * @param {RelayPort} port
   * @param {any} msg
   */
  handle(port, msg) {
    if (!msg || typeof msg !== 'object') return;
    switch (msg.t) {
      case 'open':
        this._open(port, msg.ch, msg.url);
        return;
      case 'send':
        this._send(port, msg.ch, msg.data);
        return;
      case 'close':
        this._close(port, msg.ch);
        return;
      case 'bye':
        this.detachPort(port);
        return;
      case 'stats':
        port.postMessage({ t: 'stats', id: msg.id, ...this.stats() });
        return;
      default:
    }
  }

  /**
   * Close every channel of a tab that went away.
   *
   * @param {RelayPort} port
   */
  detachPort(port) {
    const channels = this._ports.get(port);
    if (!channels) return;
    for (const ch of [...channels.keys()]) this._close(port, ch);
    this._ports.delete(port);
  }

  stats() {
    return {
      ports: this._ports.size,
      sockets: [...this._sockets.values()].map((s) => ({
        url: s.url,
        readyState: s.ws.readyState,
        channels: s.channels.size,
        ports: new Set([...s.channels].map((c) => c.port)).size,
      })),
    };
  }

  /**
   * @param {RelayPort} port
   * @param {number} ch
   * @param {string} url
   */
  _open(port, ch, url) {
    let channels = this._ports.get(port);
    if (!channels) {
      channels = new Map();
      this._ports.set(port, channels);
    }
    if (channels.has(ch)) this._close(port, ch);
    let shared = this._sockets.get(url);
    if (!shared) shared = this._openShared(url);
    /** @type {Channel} */
    const channel = { port, ch, shared };
    channels.set(ch, channel);
    shared.channels.add(channel);
    if (shared.ws.readyState !== WS_OPEN) return;
    if (shared.helloPending) {
      shared.helloTo.add(channel);
    } else {
      shared.replyTo.push(channel);
      shared.ws.send(emptyStep1Frame());
    }
    port.postMessage({ t: 'open', ch });
  }

  /** @param {string} url */
  _openShared(url) {
    const ws = this._createSocket(url);
    ws.binaryType = 'arraybuffer';
    /** @type {Shared} */
    const shared = {
      url,
      ws,
      channels: new Set(),
      helloPending: false,
      helloTo: new Set(),
      replyTo: [],
    };
    this._sockets.set(url, shared);
    ws.addEventListener('open', () => {
      if (this._sockets.get(url) !== shared) return;
      shared.helloPending = true;
      for (const channel of shared.channels) {
        shared.helloTo.add(channel);
        channel.port.postMessage({ t: 'open', ch: channel.ch });
      }
    });
    ws.addEventListener('message', (ev) => {
      if (this._sockets.get(url) !== shared) return;
      const bytes = asBytes(ev.data);
      if (bytes) this._fromHub(shared, bytes);
    });
    ws.addEventListener('close', () => {
      if (this._sockets.get(url) !== shared) return;
      this._sockets.delete(url);
      for (const channel of shared.channels) {
        this._ports.get(channel.port)?.delete(channel.ch);
        channel.port.postMessage({ t: 'close', ch: channel.ch });
      }
      shared.channels.clear();
    });
    return shared;
  }

  /**
   * @param {Shared} shared
   * @param {Uint8Array} bytes
   */
  _fromHub(shared, bytes) {
    const frame = readHubFrame(bytes);
    if (frame.kind === 'step2') {
      if (shared.helloPending) {
        shared.helloPending = false;
        const to = [...shared.helloTo];
        shared.helloTo.clear();
        this._deliver(to, bytes);
        return;
      }
      if (shared.replyTo.length > 0) {
        const asker = shared.replyTo.shift();
        if (asker && shared.channels.has(asker)) this._deliver([asker], bytes);
        return;
      }
      this._deliver([...shared.channels], updateFrame(frame.update));
      return;
    }
    if (frame.kind === 'update' || !shared.helloPending) {
      this._deliver([...shared.channels], bytes);
      return;
    }
    this._deliver([...shared.helloTo], bytes);
  }

  /**
   * @param {RelayPort} port
   * @param {number} ch
   * @param {unknown} data
   */
  _send(port, ch, data) {
    const channel = this._ports.get(port)?.get(ch);
    const bytes = asBytes(data);
    if (!channel || !bytes) return;
    const { shared } = channel;
    if (shared.ws.readyState !== WS_OPEN) return;
    const { step1, updates } = readTabFrame(bytes);
    for (let i = 0; i < step1; i++) shared.replyTo.push(channel);
    shared.ws.send(bytes);
    if (updates.length === 0) return;
    const siblings = [...shared.channels].filter((c) => c !== channel);
    if (siblings.length === 0) return;
    for (const update of updates) this._deliver(siblings, updateFrame(update));
  }

  /**
   * @param {RelayPort} port
   * @param {number} ch
   */
  _close(port, ch) {
    const channels = this._ports.get(port);
    const channel = channels?.get(ch);
    if (!channel) return;
    channels.delete(ch);
    const { shared } = channel;
    shared.channels.delete(channel);
    shared.helloTo.delete(channel);
    for (let i = 0; i < shared.replyTo.length; i++) {
      if (shared.replyTo[i] === channel) shared.replyTo[i] = null;
    }
    if (shared.channels.size > 0) return;
    if (this._sockets.get(shared.url) === shared) this._sockets.delete(shared.url);
    if (shared.ws.readyState <= WS_OPEN) shared.ws.close();
  }

  /**
   * @param {Channel[]} channels
   * @param {Uint8Array} bytes
   */
  _deliver(channels, bytes) {
    for (const channel of channels) {
      const copy = bytes.slice().buffer;
      channel.port.postMessage({ t: 'message', ch: channel.ch, data: copy }, [
        copy,
      ]);
    }
  }
}
