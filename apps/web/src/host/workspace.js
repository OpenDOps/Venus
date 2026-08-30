import { StoreExtensionManager } from '@blocksuite/affine/ext-loader';
import { getInternalStoreExtensions } from '@blocksuite/affine/extensions/store';
import { Text } from '@blocksuite/affine/store';
import { TestWorkspace } from '@blocksuite/affine/store/test';
import * as Y from 'yjs';
import { SEED_TITLE, seedHomeNote } from './seed.js';
import { MemoryNoopProvider } from './sync-provider.js';

export const SYNC_TIMEOUT_MS = 15_000;

function abortError() {
  const err = new Error('workspace create aborted');
  err.name = 'AbortError';
  return err;
}

async function waitUntilSynced(provider, signal) {
  if (provider.synced) return;
  if (signal?.aborted) throw abortError();

  let timer;
  /** @type {(() => void) | undefined} */
  let onAbort;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => {
      reject(
        new Error(
          `SyncProvider kind=${provider.kind} did not sync within ${SYNC_TIMEOUT_MS}ms. Is Compose keck up (pnpm sync:up)?`,
        ),
      );
    }, SYNC_TIMEOUT_MS);
  });
  const aborted = signal
    ? new Promise((_, reject) => {
        onAbort = () => reject(abortError());
        signal.addEventListener('abort', onAbort, { once: true });
      })
    : null;
  try {
    await Promise.race(
      aborted
        ? [provider.whenReady(), timeout, aborted]
        : [provider.whenReady(), timeout],
    );
  } finally {
    clearTimeout(timer);
    if (signal && onAbort) signal.removeEventListener('abort', onAbort);
  }
}

function hasPageRoot(store) {
  if (store.root?.flavour === 'affine:page') return true;
  for (const yBlock of store.doc.yBlocks.values()) {
    if (yBlock.get('sys:flavour') === 'affine:page') return true;
  }
  return false;
}

function seedHomePage(store) {
  const pageId = store.addBlock('affine:page', {
    title: new Text(SEED_TITLE),
  });
  store.addBlock('affine:surface', {}, pageId);
  const noteId = store.addBlock('affine:note', {}, pageId);
  store.addBlock('affine:paragraph', {}, noteId);
  seedHomeNote(store, noteId);
}

function openBareM0Workspace(options = {}) {
  const manager = new StoreExtensionManager(getInternalStoreExtensions());
  const workspace = new TestWorkspace({
    id: 'venus-m0',
    ...(options.blobSources ? { blobSources: options.blobSources } : {}),
  });
  workspace.storeExtensions = manager.get('store');
  workspace.meta.initialize();
  const docId = 'doc:home';
  const doc = workspace.createDoc(docId);
  const store = doc.getStore();
  return { workspace, store, docId };
}

/**
 * Offline clone from Yjs update v1. Does **not** seed, does **not** connect
 * a SyncProvider (keck never sees this pin). Convert with `fromDoc` on the
 * returned store — never `fromDoc` the live published Store for git / T0.
 */
export function hydrateM0FromUpdate(bytes, options = {}) {
  if (!bytes || bytes.byteLength <= 2) {
    throw new Error('Yjs pin is empty');
  }
  const { workspace, store, docId } = openBareM0Workspace(options);
  Y.applyUpdate(store.spaceDoc, bytes);
  if (!hasPageRoot(store)) {
    throw new Error('Yjs pin has no affine:page root');
  }
  store.load();
  return { workspace, store, docId };
}

/**
 * One collection, one page (M0). Sync order (M1 hydrate): connect → wait
 * until the provider is synced → seed only if there is no `affine:page` root.
 * Two clients on an empty room: open one tab first so the second hydrates.
 *
 * 0.22.4: TestWorkspace (not DocCollection). getStore does not take schema.
 */
export async function createM0Workspace(provider, options = {}) {
  const sync = provider ?? new MemoryNoopProvider();
  const signal = options.signal;
  const { workspace, store, docId } = openBareM0Workspace(options);

  sync.connect(docId, store.spaceDoc);
  try {
    await waitUntilSynced(sync, signal);
    await Promise.resolve();
  } catch (err) {
    sync.disconnect(docId);
    throw err;
  }

  if (signal?.aborted) {
    sync.disconnect(docId);
    throw abortError();
  }

  if (hasPageRoot(store)) {
    store.load();
  } else {
    store.load(() => seedHomePage(store));
    store.resetHistory();
  }

  if (signal?.aborted) {
    sync.disconnect(docId);
    throw abortError();
  }

  return { workspace, store, docId, provider: sync };
}
