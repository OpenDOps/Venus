import type { SyncProvider } from '../sync-provider.js';
import type { OctoBaseBlobSource } from './blob-source.js';

export const SAME_ORIGIN_SYNC: 'same-origin';
export const SAME_ORIGIN_SYNC_PATH: string;

export function resolveSyncUrl(env?: { VITE_SYNC_URL?: string }): string;

export function providerFromEnv(env?: {
  VITE_SYNC_URL?: string;
}): SyncProvider;

export function blobSourcesFromEnv(env?: { VITE_SYNC_URL?: string }):
  | { main: OctoBaseBlobSource }
  | undefined;

export function sidecarUrlFromEnv(env?: { VITE_SIDECAR_URL?: string }): string;
