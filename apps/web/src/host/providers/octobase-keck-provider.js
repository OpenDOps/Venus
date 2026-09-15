/**
 * Thin Yjs ↔ Venus hub bridge. Speaks y-protocols/sync over WebSocket with
 * subprotocol AFFiNE. `kind: 'octobase'` is a wire alias. Do not import this
 * from mount-editor.js.
 */
import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as syncProtocol from 'y-protocols/sync';
import { collaborationSocketUrl } from '../ids.js';

const MSG_SYNC = 0;
const MSG_AWARENESS = 1;
const MSG_AUTH = 2;
const MSG_QUERY_AWARENESS = 3;
const REMOTE = 'keck';

function asBytes(data) {
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  return null;
}

export class OctoBaseKeckProvider {
  kind = 'octobase';
  synced = false;

  /**
   * @param {string} url full WS URL, e.g. ws://127.0.0.1:3000/collaboration/<workspace uuid>
   */
  constructor(url) {
    this.url = url;
    /** @type {Map<string, { ws: WebSocket, ydoc: import('yjs').Doc, onUpdate: (u: Uint8Array, origin: unknown) => void }>} */
    this._sessions = new Map();
    /** @type {Set<() => void>} */
    this._syncListeners = new Set();
    this._ready = Promise.resolve();
    this._resolveReady = () => {};
    this._rejectReady = () => {};
  }

  /**
   * @param {'sync'} event
   * @param {() => void} fn
   */
  on(event, fn) {
    if (event !== 'sync') return () => {};
    this._syncListeners.add(fn);
    if (this.synced) queueMicrotask(fn);
    return () => this._syncListeners.delete(fn);
  }

  whenReady() {
    return this._ready;
  }

  connect(docId, ydoc) {
    this.disconnect(docId);
    this.synced = false;
    this._ready = new Promise((resolve, reject) => {
      this._resolveReady = resolve;
      this._rejectReady = reject;
    });

    const url = collaborationSocketUrl(this.url, docId);
    const ws = new WebSocket(url, ['AFFiNE']);
    ws.binaryType = 'arraybuffer';

    const onUpdate = (update, origin) => {
      if (origin === REMOTE || ws.readyState !== WebSocket.OPEN) return;
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, MSG_SYNC);
      syncProtocol.writeUpdate(encoder, update);
      ws.send(encoding.toUint8Array(encoder));
    };
    ydoc.on('update', onUpdate);
    const session = { ws, ydoc, onUpdate };
    this._sessions.set(docId, session);

    ws.addEventListener('open', () => {
      if (this._sessions.get(docId) !== session) return;
      const encoder = encoding.createEncoder();
      encoding.writeVarUint(encoder, MSG_SYNC);
      syncProtocol.writeSyncStep1(encoder, ydoc);
      ws.send(encoding.toUint8Array(encoder));
    });

    ws.addEventListener('message', (ev) => {
      if (this._sessions.get(docId) !== session) return;
      if (typeof ev.data === 'string') return;
      const bytes = asBytes(ev.data);
      if (!bytes) return;
      this._handleBinary(ws, ydoc, bytes);
    });

    ws.addEventListener('error', () => {
      if (this._sessions.get(docId) !== session || this.synced) return;
      this._rejectReady(new Error('keck websocket error'));
    });

    ws.addEventListener('close', () => {
      if (this._sessions.get(docId) !== session || this.synced) return;
      this._rejectReady(new Error('keck websocket closed before sync'));
    });
  }

  disconnect(docId) {
    const session = this._sessions.get(docId);
    if (!session) return;
    session.ydoc.off('update', session.onUpdate);
    const { ws } = session;
    this._sessions.delete(docId);
    if (
      ws.readyState === WebSocket.CONNECTING ||
      ws.readyState === WebSocket.OPEN
    ) {
      ws.close();
    }
    this.synced = false;
  }

  _markSynced() {
    if (this.synced) return;
    this.synced = true;
    this._resolveReady();
    for (const fn of this._syncListeners) fn();
  }

  /**
   * @param {WebSocket} ws
   * @param {import('yjs').Doc} ydoc
   * @param {Uint8Array} buf
   */
  _handleBinary(ws, ydoc, buf) {
    const decoder = decoding.createDecoder(buf);
    while (decoding.hasContent(decoder)) {
      const messageType = decoding.readVarUint(decoder);
      if (messageType === MSG_SYNC) {
        const encoder = encoding.createEncoder();
        encoding.writeVarUint(encoder, MSG_SYNC);
        const syncType = syncProtocol.readSyncMessage(
          decoder,
          encoder,
          ydoc,
          REMOTE,
        );
        const reply = encoding.toUint8Array(encoder);
        if (reply.byteLength > 1 && ws.readyState === WebSocket.OPEN) {
          ws.send(reply);
        }
        if (syncType === syncProtocol.messageYjsSyncStep2) {
          this._markSynced();
        }
      } else if (messageType === MSG_AWARENESS) {
        decoding.readVarUint8Array(decoder);
      } else if (messageType === MSG_AUTH) {
        decoding.readVarUint(decoder);
      } else if (messageType === MSG_QUERY_AWARENESS) {
        // keck answers awareness itself
      } else {
        break;
      }
    }
  }
}
