import type { MemoryNoopProvider } from '../sync-provider.js';
import type { VenusBlobSource } from './blob-source.js';
import type { VenusHubProvider } from './venus-hub-provider.js';

export const SAME_ORIGIN_SYNC: 'same-origin';
export const SAME_ORIGIN_SYNC_PATH: string;

export function resolveSyncUrl(env?: { VITE_SYNC_URL?: string }): string;

export function providerFromEnv(env?: {
  VITE_SYNC_URL?: string;
}): MemoryNoopProvider | VenusHubProvider;

export function blobSourcesFromEnv(env?: { VITE_SYNC_URL?: string }):
  | { main: VenusBlobSource }
  | undefined;

export function sidecarUrlFromEnv(env?: { VITE_SIDECAR_URL?: string }): string;

export function flagFromEnv(env: Record<string, unknown>, key: string): boolean;

export function debugFromEnv(env?: Record<string, unknown>): boolean;

export function testidsFromEnv(env?: Record<string, unknown>): boolean;

export function testidProps(
  name: string,
  env?: Record<string, unknown>,
): Record<string, string>;
