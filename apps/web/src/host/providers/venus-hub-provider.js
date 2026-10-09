/**
 * Thin Yjs ↔ Venus hub bridge. Speaks y-protocols/sync over WebSocket with
 * subprotocol AFFiNE. `kind: 'venus'`. Do not import this from mount-editor.js.
 *
 * `_gen` is per session: catalog + page sockets stay live together. Closing
 * one doc must not reject another session's `whenReady`.
 *
 * With a SharedWorker (`options.sockets`), each session's socket is a worker
 * channel on the same wire A URL; the `Y.Doc` stays here either way.
 */
import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import * as syncProtocol from 'y-protocols/sync';
import * as Y from 'yjs';
import { collaborationSocketUrl } from '../ids.js';

const MSG_SYNC = 0;
const MSG_AWARENESS = 1;
const MSG_AUTH = 2;
const MSG_QUERY_AWARENESS = 3;
const REMOTE = 'hub';
/** Empty Yjs state is ~2 bytes. A seeded affine page is much larger. */
const MIN_SEEDED_STEP2_BYTES = 64;
const CONFIRM_TIMEOUT_MS = 15_000;
/** First retry waits about this long; later tries double until the cap. */
export const RECONNECT_BASE_MS = 250;
export const RECONNECT_CAP_MS = 10_000;

/**
 * Jittered delay for attempt 0, 1, 2… Half to all of the exponential delay,
 * capped at {@link RECONNECT_CAP_MS}.
 *
 * @param {number} attempt
 * @param {() => number} [random]
 */
export function reconnectDelayMs(attempt, random = Math.random) {
  const step = Math.max(0, attempt);
  const exp = Math.min(RECONNECT_CAP_MS, RECONNECT_BASE_MS * 2 ** step);
  const jitter = 0.5 + random() * 0.5;
  return Math.floor(exp * jitter);
}

function sendStep1(ws, ydoc) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, MSG_SYNC);
  syncProtocol.writeSyncStep1(encoder, ydoc);
  ws.send(encoding.toUint8Array(encoder));
}

function sendUpdate(ws, update) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, MSG_SYNC);
  syncProtocol.writeUpdate(encoder, update);
  ws.send(encoding.toUint8Array(encoder));
}

/**
 * Byte length of a sync step 2 update, or 0 for any other sync message.
 * Does not advance `decoder`.
 *
 * @param {decoding.Decoder} decoder
 */
function peekSyncStep2Length(decoder) {
  const peek = decoding.clone(decoder);
  const syncType = decoding.readVarUint(peek);
  if (syncType !== syncProtocol.messageYjsSyncStep2) return 0;
  return decoding.readVarUint8Array(peek).byteLength;
}

function asBytes(data) {
  if (data instanceof Uint8Array) return data;
  if (data instanceof ArrayBuffer) return new Uint8Array(data);
  return null;
}

export class VenusHubProvider {
  kind = 'venus';
  synced = false;
  /** `reconnecting` after a synced socket drops. First connect stays `synced` here. */
  connection = 'synced';

  /**
   * @param {string} url full WS URL, e.g. ws://127.0.0.1:28710/collaboration/<workspace uuid>
   * @param {{ sockets?: { failed: boolean, open: (url: string) => WebSocket, stats?: () => Promise<unknown> } | null }} [options]
   *   `sockets` = SharedWorker fan-in of the same wire A URLs. Unset or
   *   `failed` → one WebSocket per session in this tab.
   */
  constructor(url, options = {}) {
    this.url = url;
    this._sockets = options.sockets ?? null;
    /** @type {Map<string, Session>} */
    this._sessions = new Map();
    /** @type {Set<() => void>} */
    this._syncListeners = new Set();
    /** @type {Set<() => void>} */
    this._connectionListeners = new Set();
    /** Incremented for each new session (not a single global ready generation). */
    this._gen = 0;
  }

  /** Who holds the wire A sockets for new sessions. */
  get transport() {
    return this._sockets && !this._sockets.failed ? 'shared-worker' : 'tab';
  }

  /** Worker socket table (debug / e2e). `null` on the per-tab path. */
  hubStats() {
    if (this.transport !== 'shared-worker' || !this._sockets?.stats) {
      return Promise.resolve(null);
    }
    return this._sockets.stats();
  }

  /**
   * @param {'sync' | 'connection'} event
   * @param {() => void} fn
   */
  on(event, fn) {
    if (event === 'connection') {
      this._connectionListeners.add(fn);
      if (this.connection === 'reconnecting') queueMicrotask(fn);
      return () => this._connectionListeners.delete(fn);
    }
    if (event !== 'sync') return () => {};
    this._syncListeners.add(fn);
    if (this.synced) queueMicrotask(fn);
    return () => this._syncListeners.delete(fn);
  }

  whenReady(docId) {
    if (docId != null) {
      const session = this._sessions.get(docId);
      if (!session) {
        return Promise.reject(
          new Error('hub websocket disconnected before sync'),
        );
      }
      return session.ready.then(() => {});
    }
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
    /** @type {Session} */
    const session = {
      gen,
      ws: /** @type {WebSocket} */ (/** @type {unknown} */ (null)),
      url,
      ydoc,
      onUpdate: () => {},
      ready,
      resolveReady,
      rejectReady,
      synced: false,
      everSynced: false,
      confirm: null,
      pending: [],
      reconnectTimer: null,
      reconnectAttempt: 0,
    };
    const onUpdate = (update, origin) => {
      if (origin === REMOTE) return;
      const socket = session.ws;
      if (!socket || socket.readyState !== WebSocket.OPEN) {
        session.pending.push(update);
        return;
      }
      sendUpdate(socket, update);
    };
    session.onUpdate = onUpdate;
    ydoc.on('update', onUpdate);
    this._sessions.set(docId, session);
    this._openSocket(docId, session);
  }

  /**
   * @param {string} docId
   * @param {Session} session
   */
  _openSocket(docId, session) {
    const ws =
      this.transport === 'shared-worker'
        ? this._sockets.open(session.url)
        : new WebSocket(session.url, ['AFFiNE']);
    ws.binaryType = 'arraybuffer';
    session.ws = ws;

    ws.addEventListener('open', () => {
      if (this._sessions.get(docId) !== session || session.ws !== ws) return;
      sendStep1(ws, session.ydoc);
      this._flushPending(session);
    });

    ws.addEventListener('message', (ev) => {
      if (this._sessions.get(docId) !== session || session.ws !== ws) return;
      if (typeof ev.data === 'string') return;
      const bytes = asBytes(ev.data);
      if (!bytes) return;
      this._handleBinary(ws, session.ydoc, bytes, session);
    });

    ws.addEventListener('error', () => {
      if (this._sessions.get(docId) !== session || session.ws !== ws) return;
      this._onSocketDrop(docId, session, new Error('hub websocket error'));
    });

    ws.addEventListener('close', () => {
      if (this._sessions.get(docId) !== session || session.ws !== ws) return;
      const message = session.everSynced
        ? 'hub websocket closed'
        : 'hub websocket closed before sync';
      this._onSocketDrop(docId, session, new Error(message));
    });
  }

  /**
   * A drop before the first step 2 rejects `whenReady`. After that, keep the
   * doc and open the same URL again. Offline updates stay queued.
   *
   * @param {string} docId
   * @param {Session} session
   * @param {Error} err
   */
  _onSocketDrop(docId, session, err) {
    this._failConfirm(session, err);
    if (!session.everSynced) {
      session.rejectReady(
        new Error(
          err.message === 'hub websocket error'
            ? 'hub websocket error'
            : 'hub websocket closed before sync',
        ),
      );
      return;
    }
    if (session.synced) {
      session.synced = false;
      this._recomputeSynced();
    }
    this._scheduleReconnect(docId, session);
  }

  /**
   * @param {string} docId
   * @param {Session} session
   */
  _scheduleReconnect(docId, session) {
    if (session.reconnectTimer != null) return;
    const delay = reconnectDelayMs(session.reconnectAttempt);
    session.reconnectAttempt += 1;
    const timer = setTimeout(() => {
      session.reconnectTimer = null;
      if (this._sessions.get(docId) !== session) return;
      this._openSocket(docId, session);
    }, delay);
    timer.unref?.();
    session.reconnectTimer = timer;
    this._publishConnection();
  }

  /**
   * @param {Session} session
   */
  _flushPending(session) {
    const socket = session.ws;
    while (session.pending.length > 0 && socket.readyState === WebSocket.OPEN) {
      const update = session.pending[0];
      try {
        sendUpdate(socket, update);
      } catch {
        return;
      }
      session.pending.shift();
    }
  }

  disconnect(docId) {
    const session = this._sessions.get(docId);
    if (!session) return;
    if (session.reconnectTimer != null) {
      clearTimeout(session.reconnectTimer);
      session.reconnectTimer = null;
    }
    this._failConfirm(
      session,
      new Error('hub websocket disconnected before the page seed was applied'),
    );
    session.ydoc.off('update', session.onUpdate);
    this._sessions.delete(docId);
    if (!session.everSynced) {
      session.rejectReady(new Error('hub websocket disconnected before sync'));
    }
    const { ws } = session;
    if (
      ws &&
      (ws.readyState === WebSocket.CONNECTING ||
        ws.readyState === WebSocket.OPEN)
    ) {
      ws.close();
    }
    this._recomputeSynced();
  }

  /**
   * @param {Session} session
   */
  _markSessionSynced(session) {
    session.everSynced = true;
    session.reconnectAttempt = 0;
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
    this._publishConnection();
  }

  _publishConnection() {
    const next = [...this._sessions.values()].some(
      (session) => session.everSynced && !session.synced,
    )
      ? 'reconnecting'
      : 'synced';
    if (next === this.connection) return;
    this.connection = next;
    for (const fn of this._connectionListeners) fn();
  }

  /**
   * The join handshake sends a step 1, and the hub's step 2 reply can arrive
   * after `whenReady` (which resolves on the hello step 2). Drain that reply
   * before arming `confirmApplied`, or the empty reply is mistaken for the
   * seed check.
   *
   * @param {string} docId
   * @returns {Promise<void>}
   */
  nextStep2(docId) {
    return this._expectStep2(docId, 0);
  }

  /**
   * After the page seed's update is already queued on this socket, ask the
   * hub for its full doc (sync step 1 against an empty state vector).
   * TCP orders that step 1 behind the seed, so the reply contains the page
   * only if the hub applied it. A step 1 of the local doc would be empty
   * even when the hub has the seed, because the hub does not echo updates.
   *
   * @param {string} docId
   * @returns {Promise<void>}
   */
  confirmApplied(docId) {
    const session = this._sessions.get(docId);
    if (!session?.synced || session.ws.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error('hub page seed is not synced'));
    }
    const pending = this._expectStep2(docId, MIN_SEEDED_STEP2_BYTES);
    const encoder = encoding.createEncoder();
    encoding.writeVarUint(encoder, MSG_SYNC);
    const empty = new Y.Doc();
    try {
      syncProtocol.writeSyncStep1(encoder, empty);
    } finally {
      empty.destroy();
    }
    try {
      session.ws.send(encoding.toUint8Array(encoder));
    } catch (err) {
      this._failConfirm(
        session,
        err instanceof Error ? err : new Error(String(err)),
      );
    }
    return pending;
  }

  /**
   * @param {string} docId
   * @param {number} minBytes
   * @returns {Promise<void>}
   */
  _expectStep2(docId, minBytes) {
    const session = this._sessions.get(docId);
    if (!session?.synced || session.ws.readyState !== WebSocket.OPEN) {
      return Promise.reject(new Error('hub page seed is not synced'));
    }
    this._failConfirm(session, new Error('page seed confirm replaced'));
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        if (session.confirm == null) return;
        session.confirm = null;
        reject(new Error('hub did not apply the page seed'));
      }, CONFIRM_TIMEOUT_MS);
      timer.unref?.();
      session.confirm = { resolve, reject, timer, minBytes };
    });
  }

  /**
   * @param {Session} session
   * @param {Error} err
   */
  _failConfirm(session, err) {
    const pending = session.confirm;
    if (!pending) return;
    session.confirm = null;
    clearTimeout(pending.timer);
    pending.reject(err);
  }

  /**
   * @param {Session} session
   * @param {number} step2Len
   */
  _settleConfirm(session, step2Len) {
    const pending = session.confirm;
    if (!pending) return;
    session.confirm = null;
    clearTimeout(pending.timer);
    if (step2Len < pending.minBytes) {
      pending.reject(new Error('hub did not apply the page seed'));
      return;
    }
    pending.resolve();
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
        const step2Len = peekSyncStep2Length(decoder);
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
          this._settleConfirm(session, step2Len);
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
 *   url: string,
 *   ydoc: import('yjs').Doc,
 *   onUpdate: (u: Uint8Array, origin: unknown) => void,
 *   ready: Promise<void>,
 *   resolveReady: () => void,
 *   rejectReady: (err: Error) => void,
 *   synced: boolean,
 *   everSynced: boolean,
 *   pending: Uint8Array[],
 *   reconnectTimer: ReturnType<typeof setTimeout> | null,
 *   reconnectAttempt: number,
 *   confirm: null | {
 *     resolve: () => void,
 *     reject: (err: Error) => void,
 *     timer: ReturnType<typeof setTimeout>,
 *     minBytes: number,
 *   },
 * }} Session
 */
