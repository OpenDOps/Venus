import type { SyncProvider } from '../sync-provider.js';

export function providerFromEnv(env?: {
  VITE_SYNC_URL?: string;
}): SyncProvider;
