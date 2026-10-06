import { commitNewDoc, createDoc, mintDocId } from './ops.js';
import { seedEmptyPageIfNeeded, waitUntilSynced } from '../workspace.js';

function abortError() {
  const err = new Error('workspace create aborted');
  err.name = 'AbortError';
  return err;
}

/**
 * Create a catalog page whose body is already on the hub.
 *
 * Memory (and a missing provider) keep the synchronous local seed.
 * A live provider connects the new Y.Doc, seeds only after sync, and
 * inserts the catalog node only after `confirmApplied` resolves.
 * The socket is closed either way. A failure leaves no tree row.
 *
 * @param {import('yjs').Doc} catalog
 * @param {unknown} workspace
 * @param {import('../sync-provider.js').SyncProvider | null | undefined} provider
 * @param {{ createAt: string | null, signal?: AbortSignal }} options
 */
export async function createPublishedDoc(catalog, workspace, provider, options) {
  const createAt = options?.createAt;
  if (!provider || provider.kind === 'memory') {
    return createDoc(catalog, workspace, { createAt });
  }

  const id = mintDocId(catalog, createAt);
  const doc = workspace?.createDoc?.(id);
  const store = doc?.getStore?.();
  if (!store?.spaceDoc) {
    throw new Error('createPublishedDoc could not mint a workspace page');
  }

  try {
    provider.connect(id, store.spaceDoc);
    await waitUntilSynced(provider, options.signal, id);
    if (options.signal?.aborted) throw abortError();
    if (typeof provider.nextStep2 === 'function') {
      await provider.nextStep2(id);
    }
    if (options.signal?.aborted) throw abortError();
    seedEmptyPageIfNeeded(store);
    if (typeof provider.confirmApplied !== 'function') {
      throw new Error('sync provider cannot confirm a page seed');
    }
    await provider.confirmApplied(id);
    if (options.signal?.aborted) throw abortError();
    return commitNewDoc(catalog, workspace, { id, createAt });
  } finally {
    provider.disconnect(id);
  }
}
