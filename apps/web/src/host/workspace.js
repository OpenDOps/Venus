import { StoreExtensionManager } from '@blocksuite/affine/ext-loader';
import { getInternalStoreExtensions } from '@blocksuite/affine/extensions/store';
import { Text } from '@blocksuite/affine/store';
import { TestWorkspace } from '@blocksuite/affine/store/test';
import * as Y from 'yjs';
import { SEED_TITLE, seedHomeNote } from './seed.js';
import { MemoryNoopProvider } from './sync-provider.js';
import { PAGE_DOC_ID, WORKSPACE_ID } from './ids.js';

export const SYNC_TIMEOUT_MS = 15_000;
export const OPEN_PAGE_ATTEMPTS = 3;
/** Backoff between transport retries. Seconds, not 250ms — hydrate races must not be hammered. */
export const OPEN_PAGE_RETRY_MS = 2_000;

function abortError() {
  const err = new Error('workspace create aborted');
  err.name = 'AbortError';
  return err;
}

function isAbortError(err) {
  return err instanceof Error && err.name === 'AbortError';
}

/**
 * Retry only transport timeout / refused connect. Hub `ensure_doc` hydrate
 * shows up as websocket error/close — retrying that storms nested txs.
 *
 * @param {unknown} err
 */
export function isOpenPageTransportError(err) {
  const msg = err instanceof Error ? err.message : String(err);
  if (/did not sync within \d+ms/i.test(msg)) return true;
  if (/ECONNREFUSED/i.test(msg)) return true;
  return false;
}

/**
 * @param {number} ms
 * @param {AbortSignal} [signal]
 */
function delay(ms, signal) {
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(abortError());
      return;
    }
    /** @type {(() => void) | undefined} */
    let onAbort;
    const timer = setTimeout(() => {
      if (signal && onAbort) signal.removeEventListener('abort', onAbort);
      resolve();
    }, ms);
    if (signal) {
      onAbort = () => {
        clearTimeout(timer);
        reject(abortError());
      };
      signal.addEventListener('abort', onAbort, { once: true });
    }
  });
}

export async function waitUntilSynced(provider, signal, docId) {
  if (docId == null && provider.synced) return;
  if (signal?.aborted) throw abortError();

  let timer;
  /** @type {(() => void) | undefined} */
  let onAbort;
  const timeout = new Promise((_, reject) => {
    timer = setTimeout(() => {
      reject(
        new Error(
          `SyncProvider kind=${provider.kind} did not sync within ${SYNC_TIMEOUT_MS}ms. Is Compose hub up (pnpm sync:up)?`,
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
  const ready =
    docId != null ? provider.whenReady(docId) : provider.whenReady();
  try {
    await Promise.race(
      aborted ? [ready, timeout, aborted] : [ready, timeout],
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

/** Second page: affine:page root only. Not the home “Why Venus” seed. */
function seedEmptyPage(store) {
  const pageId = store.addBlock('affine:page', {
    title: new Text(''),
  });
  store.addBlock('affine:surface', {}, pageId);
  const noteId = store.addBlock('affine:note', {}, pageId);
  store.addBlock('affine:paragraph', {}, noteId);
}

/**
 * Local empty page for a minted uuid. No SyncProvider — catalog create
 * must not open a socket. First hub connect then syncs this seed up.
 *
 * @param {{ load: Function, resetHistory: Function, root?: { flavour?: string }, doc?: { yBlocks?: { values: () => Iterable<{ get: (k: string) => unknown }> } } }} store
 * @returns {boolean} true if a seed ran
 */
export function seedEmptyPageIfNeeded(store) {
  if (!store || hasPageRoot(store)) return false;
  store.load(() => seedEmptyPage(store));
  store.resetHistory();
  return true;
}

/**
 * Memory may seed an empty affine:page (tests / offline). Venus and
 * y-websocket: empty after sync is hydrate failure, not a new space,
 * unless `seedIfEmpty` is set.
 *
 * @param {{ kind?: string }} provider
 * @param {{ seedIfEmpty?: boolean }} [options]
 */
export function shouldSeedEmptyPage(provider, options = {}) {
  if (options.seedIfEmpty === true) return true;
  if (options.seedIfEmpty === false) return false;
  return provider.kind === 'memory';
}

function emptyPageSyncError(docId) {
  const err = new Error(`page ${docId} has no affine:page root after sync`);
  err.name = 'EmptyPageSyncError';
  return err;
}

function openBareM0Workspace(options = {}) {
  const manager = new StoreExtensionManager(getInternalStoreExtensions());
  const workspace = new TestWorkspace({
    id: WORKSPACE_ID,
    ...(options.blobSources ? { blobSources: options.blobSources } : {}),
  });
  workspace.storeExtensions = manager.get('store');
  workspace.meta.initialize();
  const docId = PAGE_DOC_ID;
  const doc = workspace.createDoc(docId);
  const store = doc.getStore();
  return { workspace, store, docId };
}

/**
 * Offline clone from Yjs update v1. Does **not** seed, does **not** connect
 * a SyncProvider (the hub never sees this pin). Convert with `fromDoc` on the
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
  const extras = options.connectDocs ?? [];
  const { workspace, store, docId } = openBareM0Workspace(options);

  const disconnectAll = () => {
    for (const extra of extras) sync.disconnect(extra.docId);
    sync.disconnect(docId);
  };

  sync.connect(docId, store.spaceDoc);
  for (const extra of extras) {
    sync.connect(extra.docId, extra.ydoc);
  }
  try {
    await waitUntilSynced(sync, signal);
    await Promise.resolve();
  } catch (err) {
    disconnectAll();
    throw err;
  }

  if (signal?.aborted) {
    disconnectAll();
    throw abortError();
  }

  if (hasPageRoot(store)) {
    store.load();
  } else {
    store.load(() => seedHomePage(store));
    store.resetHistory();
  }

  if (signal?.aborted) {
    disconnectAll();
    throw abortError();
  }

  return { workspace, store, docId, provider: sync };
}

/**
 * Open another page on an existing collection. `uuid` is the minted SQL
 * uuid / BlockSuite `createDoc` id (wire A `?doc=`). Connects that Y.Doc;
 * seeds an empty `affine:page` only in memory (or `seedIfEmpty`). Hub
 * empty after sync is hydrate failure — throw, do not broadcast a blank
 * page. Catalog `createDoc` seeds locally so a minted uuid still has a root.
 * Do not call from App boot — home is still `createM0Workspace`.
 */
export async function openWorkspaceDoc(workspace, provider, uuid, options = {}) {
  const signal = options.signal;
  const docId = String(uuid).toLowerCase();
  const existing =
    typeof workspace.getDoc === 'function' ? workspace.getDoc(docId) : null;
  const doc = existing ?? workspace.createDoc(docId);
  const store = doc.getStore();

  if (signal?.aborted) throw abortError();

  provider.connect(docId, store.spaceDoc);
  try {
    await waitUntilSynced(provider, signal, docId);
    await Promise.resolve();
  } catch (err) {
    provider.disconnect(docId);
    throw err;
  }

  if (signal?.aborted) {
    provider.disconnect(docId);
    throw abortError();
  }

  if (hasPageRoot(store)) {
    store.load();
  } else if (shouldSeedEmptyPage(provider, options)) {
    store.load(() => seedEmptyPage(store));
    store.resetHistory();
  } else {
    provider.disconnect(docId);
    throw emptyPageSyncError(docId);
  }

  if (signal?.aborted) {
    provider.disconnect(docId);
    throw abortError();
  }

  return { workspace, store, docId, provider };
}

/**
 * Open a page. Retry only transport timeout / ECONNREFUSED, with
 * `OPEN_PAGE_RETRY_MS` backoff. Websocket close/error (including hub
 * hydrate failure) is one connect — do not storm `ensure_doc`.
 * Abort `signal` to stop further connects.
 *
 * @param {{ getDoc?: Function, createDoc: Function }} workspace
 * @param {import('./sync-provider.js').SyncProvider} provider
 * @param {string} uuid
 * @param {{ signal?: AbortSignal }} [options]
 */
export async function openPageStore(workspace, provider, uuid, options = {}) {
  const signal = options.signal;
  const docId = String(uuid).toLowerCase();
  let last;
  for (let attempt = 0; attempt < OPEN_PAGE_ATTEMPTS; attempt++) {
    if (signal?.aborted) {
      provider.disconnect(docId);
      throw isAbortError(last) ? last : abortError();
    }
    try {
      return await openWorkspaceDoc(workspace, provider, uuid, { signal });
    } catch (err) {
      last = err;
      provider.disconnect(docId);
      if (isAbortError(err)) throw err;
      if (
        !isOpenPageTransportError(err) ||
        attempt >= OPEN_PAGE_ATTEMPTS - 1
      ) {
        throw err instanceof Error ? err : new Error(String(err));
      }
      await delay(OPEN_PAGE_RETRY_MS * (attempt + 1), signal);
    }
  }
  throw last instanceof Error ? last : new Error(String(last));
}
