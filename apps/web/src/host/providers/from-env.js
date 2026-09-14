import { MemoryNoopProvider } from '../sync-provider.js';
import { OctoBaseBlobSource, blobOriginFromSyncUrl } from './blob-source.js';
import { OctoBaseKeckProvider } from './octobase-keck-provider.js';
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
 * Unset / empty VITE_SYNC_URL → MemoryNoopProvider (no socket).
 *
 * @param {{ VITE_SYNC_URL?: string }} [env]
 */
export function providerFromEnv(env = import.meta.env) {
  const url = resolveSyncUrl(env);
  if (!url) return new MemoryNoopProvider();
  return new OctoBaseKeckProvider(url);
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
      main: new OctoBaseBlobSource({
        workspaceId: WORKSPACE_ID,
        origin: '',
      }),
    };
  }
  return {
    main: new OctoBaseBlobSource({
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
