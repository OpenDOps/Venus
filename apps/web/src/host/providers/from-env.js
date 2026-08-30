import { MemoryNoopProvider } from '../sync-provider.js';
import { OctoBaseKeckProvider } from './octobase-keck-provider.js';

/**
 * App-only factory. Vitest calls createM0Workspace() with the memory default.
 * Unset / empty VITE_SYNC_URL → MemoryNoopProvider (no socket).
 *
 * @param {{ VITE_SYNC_URL?: string }} [env]
 */
export function providerFromEnv(env = import.meta.env) {
  const url = typeof env.VITE_SYNC_URL === 'string' ? env.VITE_SYNC_URL.trim() : '';
  if (!url) return new MemoryNoopProvider();
  return new OctoBaseKeckProvider(url);
}
