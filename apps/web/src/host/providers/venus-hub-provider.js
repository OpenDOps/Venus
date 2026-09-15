/**
 * Thin Yjs ↔ Venus hub bridge. Speaks y-protocols/sync over WebSocket with
 * subprotocol AFFiNE. `kind: 'venus'`. Do not import this from mount-editor.js.
 *
 * `_gen` is per session: catalog + page sockets stay live together. Closing
 * one doc must not reject another session's `whenReady`.
 */
import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as syncProtocol from 'y-protocols/sync';
import { collaborationSocketUrl } from '../ids.js';

const MSG_SYNC = 0;
const MSG_AWARENESS = 1;
const MSG_AUTH = 2;
const MSG_QUERY_AWARENESS = 3;
const REMOTE = 'hub';

function asBytes(data) {
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  return null;
}

export class VenusHubProvider {
  kind = 'venus';
  synced = false;

  /**
   * @param {string} url full WS URL, e.g. ws://127.0.0.1:3000/collaboration/<workspace uuid>
   */
  constructor(url) {
    this.url = url;
    /** @type {Map<string, Session>} */
    this._sessions = new Map();
    /** @type {Set<() => void>} */
    this._syncListeners = new Set();
    /** Incremented for each new session (not a single global ready generation). */
    this._gen = 0;
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
    const sessions = [...this._sessions.values()];
    if (sessions.length === 0) return Promise.resolve();
    return Promise.all(sessions.map((s) => s.ready)).then(() => {});
  }

  connect(docId, ydoc) {
    this.disconnect(docId);
    const gen = ++this._gen;
    let resolveReady = () => {};
    let rejectReady = () => {};
    const ready = new Promise((resolve, reject) => {
      resolveReady = resolve;
      rejectReady = reject;
    });
    // Intentional disconnect / late close must not become an unhandled rejection
    // when waitUntilSynced already lost a Promise.race (abort / timeout).
    ready.catch(() => {});
    this.synced = false;

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
    /** @type {Session} */
    const session = {
      gen,
      ws,
      ydoc,
      onUpdate,
      ready,
      resolveReady,
      rejectReady,
      synced: false,
    };
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
      this._handleBinary(ws, ydoc, bytes, session);
    });

    ws.addEventListener('error', () => {
      if (this._sessions.get(docId) !== session || session.synced) return;
      session.rejectReady(new Error('hub websocket error'));
    });

    ws.addEventListener('close', () => {
      if (this._sessions.get(docId) !== session || session.synced) return;
      session.rejectReady(new Error('hub websocket closed before sync'));
    });
  }

  disconnect(docId) {
    const session = this._sessions.get(docId);
    if (!session) return;
    session.ydoc.off('update', session.onUpdate);
    this._sessions.delete(docId);
    if (!session.synced) {
      session.rejectReady(new Error('hub websocket disconnected before sync'));
    }
    const { ws } = session;
    if (
      ws.readyState === WebSocket.CONNECTING ||
      ws.readyState === WebSocket.OPEN
    ) {
      ws.close();
    }
    this._recomputeSynced();
  }

  /**
   * @param {Session} session
   */
  _markSessionSynced(session) {
    if (session.synced) return;
    session.synced = true;
    session.resolveReady();
    this._recomputeSynced();
  }

  _recomputeSynced() {
    const sessions = [...this._sessions.values()];
    const next = sessions.length > 0 && sessions.every((s) => s.synced);
    if (next && !this.synced) {
      this.synced = true;
      for (const fn of this._syncListeners) fn();
    } else if (!next) {
      this.synced = false;
    }
  }

  /**
   * @param {WebSocket} ws
   * @param {import('yjs').Doc} ydoc
   * @param {Uint8Array} buf
   * @param {Session} session
   */
  _handleBinary(ws, ydoc, buf, session) {
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
          this._markSessionSynced(session);
        }
      } else if (messageType === MSG_AWARENESS) {
        decoding.readVarUint8Array(decoder);
      } else if (messageType === MSG_AUTH) {
        decoding.readVarUint(decoder);
      } else if (messageType === MSG_QUERY_AWARENESS) {
        // hub answers awareness itself
      } else {
        break;
      }
    }
  }
}

/**
 * @typedef {{
 *   gen: number,
 *   ws: WebSocket,
 *   ydoc: import('yjs').Doc,
 *   onUpdate: (u: Uint8Array, origin: unknown) => void,
 *   ready: Promise<void>,
 *   resolveReady: () => void,
 *   rejectReady: (err: Error) => void,
 *   synced: boolean,
 * }} Session
 */
