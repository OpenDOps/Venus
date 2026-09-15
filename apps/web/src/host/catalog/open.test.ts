import { expect, test } from 'vitest';
import * as Y from 'yjs';
import type { SyncProvider } from '../sync-provider.js';
import { CATALOG_GUID, PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import {
  createCatalogDoc,
  disposeCatalog,
  openCatalog,
} from './open.js';
import { FOLDER_SPEC_ID, getNode } from './schema.js';

test('openCatalog connects venus:catalog as a plain Y.Doc', async () => {
  const calls: { op: string; docId: string; guid?: string }[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: true,
    connect(id, ydoc) {
      calls.push({ op: 'connect', docId: id, guid: ydoc.guid });
    },
    disconnect(id) {
      calls.push({ op: 'disconnect', docId: id });
    },
    whenReady() {
      return Promise.resolve();
    },
  };
  const { workspace } = await createM0Workspace();
  const { catalog, docId } = await openCatalog(provider, workspace);
  expect(docId).toBe(CATALOG_GUID);
  expect(catalog.guid).toBe(CATALOG_GUID);
  expect(calls).toEqual([
    { op: 'connect', docId: CATALOG_GUID, guid: CATALOG_GUID },
  ]);
  expect(getNode(catalog, FOLDER_SPEC_ID)?.kind).toBe('folder');
  expect(getNode(catalog, PAGE_DOC_ID)?.gitPath).toBe('spec/home.md');
  expect([...catalog.share.keys()]).toEqual(['nodes']);
});

test('openCatalog disconnects on abort', async () => {
  const ac = new AbortController();
  const disconnected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect() {},
    disconnect(id) {
      disconnected.push(id);
    },
    whenReady() {
      ac.abort();
      return new Promise(() => {});
    },
  };
  const { workspace } = await createM0Workspace();
  await expect(
    openCatalog(provider, workspace, { signal: ac.signal }),
  ).rejects.toMatchObject({ name: 'AbortError' });
  expect(disconnected).toEqual([CATALOG_GUID]);
});

test('openCatalog abort destroys an owned catalog Y.Doc', async () => {
  const destroyed: string[] = [];
  const orig = Y.Doc.prototype.destroy;
  Y.Doc.prototype.destroy = function destroy() {
    destroyed.push(this.guid);
    return orig.call(this);
  };
  try {
    const ac = new AbortController();
    const provider: SyncProvider = {
      kind: 'memory',
      synced: false,
      connect() {},
      disconnect() {},
      whenReady() {
        ac.abort();
        return new Promise(() => {});
      },
    };
    const { workspace } = await createM0Workspace();
    await expect(
      openCatalog(provider, workspace, { signal: ac.signal }),
    ).rejects.toMatchObject({ name: 'AbortError' });
    expect(destroyed).toContain(CATALOG_GUID);
  } finally {
    Y.Doc.prototype.destroy = orig;
  }
});

test('connectDocs + alreadyConnected is one wait and dispose destroys', async () => {
  const calls: string[] = [];
  let ready = 0;
  const provider: SyncProvider = {
    kind: 'memory',
    synced: false,
    connect(id) {
      calls.push(id);
    },
    disconnect() {},
    whenReady() {
      ready += 1;
      return Promise.resolve();
    },
  };
  const catalog = createCatalogDoc();
  const { workspace, docId } = await createM0Workspace(provider, {
    connectDocs: [{ docId: CATALOG_GUID, ydoc: catalog }],
  });
  expect(calls).toEqual([docId, CATALOG_GUID]);
  expect(ready).toBe(1);
  await openCatalog(provider, workspace, {
    catalog,
    alreadyConnected: true,
  });
  expect(calls).toEqual([docId, CATALOG_GUID]);
  expect(ready).toBe(1);
  expect(getNode(catalog, FOLDER_SPEC_ID)?.kind).toBe('folder');
  expect(catalog.isDestroyed).toBe(false);
  disposeCatalog(provider, catalog, docId);
  expect(catalog.isDestroyed).toBe(true);
});

