import { MemoryNoopProvider } from '../sync-provider.js';
import { VenusBlobSource, blobOriginFromSyncUrl } from './blob-source.js';
import { createSharedWorkerSockets } from './shared-worker-socket.js';
import { VenusHubProvider } from './venus-hub-provider.js';
import { COLLABORATION_PATH, WORKSPACE_ID } from '../ids.js';

/** Bake this in Compose/k8s web. Browser WS is same-origin `/collaboration/…`. */
export const SAME_ORIGIN_SYNC = 'same-origin';
export const SAME_ORIGIN_SYNC_PATH = COLLABORATION_PATH;

/**
 * Turn Vite env into a WebSocket URL. `same-origin` needs `location.host`
 * (Compose nginx / future Ingress). Absolute `ws://` stays as-is (Vite + Playwright).
 *
 * @param {{ VITE_SYNC_URL?: string }} [env]
 */
export function resolveSyncUrl(env = import.meta.env) {
  const raw =
    typeof env.VITE_SYNC_URL === 'string' ? env.VITE_SYNC_URL.trim() : '';
  if (!raw) return '';
  if (raw !== SAME_ORIGIN_SYNC) return raw;
  const loc = globalThis.location;
  if (!loc?.host) {
    throw new Error(
      'VITE_SYNC_URL=same-origin needs window.location (Compose web). For Vite use an absolute ws://…/collaboration/<workspace uuid>.',
    );
  }
  const proto = loc.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${proto}//${loc.host}${SAME_ORIGIN_SYNC_PATH}`;
}

/**
 * App-only factory. Vitest calls createM0Workspace() with the memory default.
 * Unset / empty VITE_SYNC_URL → MemoryNoopProvider (no socket, no worker).
 * Where `SharedWorker` exists, the hub provider's sockets live in one worker
 * per origin + hub URL; otherwise each tab opens its own.
 *
 * @param {{ VITE_SYNC_URL?: string }} [env]
 */
export function providerFromEnv(env = import.meta.env) {
  const url = resolveSyncUrl(env);
  if (!url) return new MemoryNoopProvider();
  return new VenusHubProvider(url, { sockets: createSharedWorkerSockets(url) });
}

/**
 * HTTP blob store when sync env is set. Otherwise TestWorkspace keeps
 * MemoryBlobSource. Browser (and Compose `same-origin`) uses same-origin
 * `/api` (Vite or nginx proxy). Node uses the hub origin from VITE_SYNC_URL.
 *
 * @param {{ VITE_SYNC_URL?: string }} [env]
 */
export function blobSourcesFromEnv(env = import.meta.env) {
  const raw =
    typeof env.VITE_SYNC_URL === 'string' ? env.VITE_SYNC_URL.trim() : '';
  if (!raw) return undefined;
  const sameOrigin =
    typeof window !== 'undefined' || raw === SAME_ORIGIN_SYNC;
  if (sameOrigin) {
    return {
      main: new VenusBlobSource({
        workspaceId: WORKSPACE_ID,
        origin: '',
      }),
    };
  }
  return {
    main: new VenusBlobSource({
      workspaceId: WORKSPACE_ID,
      origin: blobOriginFromSyncUrl(raw),
    }),
  };
}

/**
 * Sidecar Flush URL. Unset → no host Flush chrome (memory / default e2e).
 *
 * @param {{ VITE_SIDECAR_URL?: string }} [env]
 */
export function sidecarUrlFromEnv(env = import.meta.env) {
  const raw =
    typeof env.VITE_SIDECAR_URL === 'string' ? env.VITE_SIDECAR_URL.trim() : '';
  return raw.replace(/\/$/, '');
}

/**
 * Truthy Vite flag (`1` / `true` / `yes`). Used for `VITE_DEBUG` and
 * `VITE_TESTIDS`.
 *
 * @param {Record<string, unknown>} env
 * @param {string} key
 */
export function flagFromEnv(env, key) {
  const raw = env?.[key];
  if (raw === true || raw === 1) return true;
  if (typeof raw !== 'string') return false;
  const v = raw.trim().toLowerCase();
  return v === '1' || v === 'true' || v === 'yes';
}

/**
 * Debug bar (Flush / git-log). Playwright m3/m4 sets this. Not a hub env.
 *
 * @param {Record<string, unknown>} [env]
 */
export function debugFromEnv(env = import.meta.env) {
  return flagFromEnv(env, 'VITE_DEBUG');
}

/**
 * Product `data-testid`s (header / tree). Playwright m4 sets this.
 *
 * @param {Record<string, unknown>} [env]
 */
export function testidsFromEnv(env = import.meta.env) {
  return flagFromEnv(env, 'VITE_TESTIDS');
}

/**
 * @param {string} name
 * @param {Record<string, unknown>} [env]
 * @returns {Record<string, string>}
 */
export function testidProps(name, env = import.meta.env) {
  return testidsFromEnv(env) ? { 'data-testid': name } : {};
}
