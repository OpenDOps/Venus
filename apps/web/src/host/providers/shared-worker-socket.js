/**
 * Tab side of the hub SharedWorker. `open(url)` returns a WebSocket-shaped
 * channel so `VenusHubProvider` keeps one code path. The worker owns the
 * TCP; the tab keeps the `Y.Doc`. Missing `SharedWorker`, a constructor that
 * throws, or a worker that fails to load → callers use per-tab WebSockets.
 */

const CONNECTING = 0;
const OPEN = 1;
const CLOSING = 2;
const CLOSED = 3;

export const HUB_WORKER_NAME_PREFIX = 'venus-hub:';

/** @type {Map<string, SharedWorkerSockets>} */
const tabConnections = new Map();

/**
 * One worker per origin + hub URL (the URL names the workspace), and one
 * port per tab: providers built again in this tab reuse the connection.
 *
 * @param {string} hubUrl
 * @returns {SharedWorkerSockets | null}
 */
export function createSharedWorkerSockets(hubUrl) {
  if (typeof SharedWorker !== 'function') return null;
  const cached = tabConnections.get(hubUrl);
  if (cached && !cached.failed) return cached;
  let worker;
  try {
    worker = new SharedWorker(new URL('./hub-shared-worker.js', import.meta.url), {
      type: 'module',
      name: `${HUB_WORKER_NAME_PREFIX}${hubUrl}`,
    });
  } catch {
    return null;
  }
  const sockets = new SharedWorkerSockets(worker.port, worker);
  tabConnections.set(hubUrl, sockets);
  return sockets;
}

class WorkerSocket {
  /**
   * @param {SharedWorkerSockets} owner
   * @param {number} ch
   * @param {string} url
   */
  constructor(owner, ch, url) {
    this._owner = owner;
    this._ch = ch;
    this.url = url;
    this.protocol = 'AFFiNE';
    this.binaryType = 'arraybuffer';
    this.readyState = CONNECTING;
    /** @type {Map<string, Set<(ev: any) => void>>} */
    this._listeners = new Map();
  }

  /**
   * @param {string} type
   * @param {(ev: any) => void} fn
   */
  addEventListener(type, fn) {
    let set = this._listeners.get(type);
    if (!set) {
      set = new Set();
      this._listeners.set(type, set);
    }
    set.add(fn);
  }

  /**
   * @param {string} type
   * @param {(ev: any) => void} fn
   */
  removeEventListener(type, fn) {
    this._listeners.get(type)?.delete(fn);
  }

  /** @param {Uint8Array | ArrayBuffer} data */
  send(data) {
    if (this.readyState !== OPEN) {
      throw new Error('hub worker socket is not open');
    }
    const view = data instanceof Uint8Array ? data : new Uint8Array(data);
    const copy = view.slice().buffer;
    this._owner._post({ t: 'send', ch: this._ch, data: copy }, [copy]);
  }

  close() {
    if (this.readyState === CLOSING || this.readyState === CLOSED) return;
    this._owner._post({ t: 'close', ch: this._ch });
    this.readyState = CLOSING;
    this._owner._forget(this._ch);
    queueMicrotask(() => this._closed());
  }

  _opened() {
    if (this.readyState !== CONNECTING) return;
    this.readyState = OPEN;
    this._emit('open', {});
  }

  /** @param {ArrayBuffer} data */
  _message(data) {
    if (this.readyState !== OPEN) return;
    this._emit('message', { data });
  }

  _closed() {
    if (this.readyState === CLOSED) return;
    this.readyState = CLOSED;
    this._emit('close', {});
  }

  /**
   * @param {string} type
   * @param {unknown} ev
   */
  _emit(type, ev) {
    for (const fn of [...(this._listeners.get(type) ?? [])]) fn(ev);
  }
}

export class SharedWorkerSockets {
  kind = 'shared-worker';
  /** True after the worker failed to load; new opens use per-tab sockets. */
  failed = false;

  /**
   * @param {MessagePort} port
   * @param {{ addEventListener?: Function } | null} [worker]
   */
  constructor(port, worker = null) {
    this._port = port;
    this._nextCh = 0;
    this._nextStats = 0;
    /** @type {Map<number, WorkerSocket>} */
    this._open = new Map();
    /** @type {Map<number, (s: unknown) => void>} */
    this._statsWaiters = new Map();
    port.addEventListener('message', (ev) => this._onMessage(ev.data));
    port.start();
    worker?.addEventListener?.('error', () => this._fail());
    const win = globalThis.window;
    if (win && typeof win.addEventListener === 'function') {
      win.addEventListener('pagehide', () => this._post({ t: 'bye' }));
      win.addEventListener('pageshow', (ev) => {
        if (ev.persisted) this._dropAll();
      });
    }
  }

  /** @param {string} url */
  open(url) {
    const ch = ++this._nextCh;
    const socket = new WorkerSocket(this, ch, url);
    this._open.set(ch, socket);
    this._post({ t: 'open', ch, url });
    return /** @type {WebSocket} */ (/** @type {unknown} */ (socket));
  }

  /**
   * Sockets the worker holds right now (debug / e2e).
   *
   * @returns {Promise<{ ports: number, sockets: Array<{ url: string, readyState: number, channels: number, ports: number }> }>}
   */
  stats() {
    const id = ++this._nextStats;
    return new Promise((resolve) => {
      this._statsWaiters.set(id, /** @type {(s: unknown) => void} */ (resolve));
      this._post({ t: 'stats', id });
    });
  }

  /**
   * @param {unknown} msg
   * @param {Transferable[]} [transfer]
   */
  _post(msg, transfer) {
    if (this.failed) return;
    this._port.postMessage(msg, transfer ?? []);
  }

  /** @param {number} ch */
  _forget(ch) {
    this._open.delete(ch);
  }

  /** @param {any} msg */
  _onMessage(msg) {
    if (!msg || typeof msg !== 'object') return;
    if (msg.t === 'stats') {
      const resolve = this._statsWaiters.get(msg.id);
      this._statsWaiters.delete(msg.id);
      resolve?.({ ports: msg.ports, sockets: msg.sockets });
      return;
    }
    const socket = this._open.get(msg.ch);
    if (!socket) return;
    if (msg.t === 'open') socket._opened();
    else if (msg.t === 'message') socket._message(msg.data);
    else if (msg.t === 'close') {
      this._open.delete(msg.ch);
      socket._closed();
    }
  }

  _dropAll() {
    const sockets = [...this._open.values()];
    this._open.clear();
    for (const socket of sockets) socket._closed();
  }

  _fail() {
    if (this.failed) return;
    this.failed = true;
    this._dropAll();
    for (const resolve of this._statsWaiters.values()) {
      resolve({ ports: 0, sockets: [] });
    }
    this._statsWaiters.clear();
  }
}
