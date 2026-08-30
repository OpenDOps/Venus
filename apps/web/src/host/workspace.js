import { StoreExtensionManager } from '@blocksuite/affine/ext-loader';
import { getInternalStoreExtensions } from '@blocksuite/affine/extensions/store';
import { Text } from '@blocksuite/affine/store';
import { TestWorkspace } from '@blocksuite/affine/store/test';
import { SEED_TITLE, seedHomeNote } from './seed.js';
import { MemoryNoopProvider } from './sync-provider.js';

/**
 * One in-memory collection, one page, default block tree (M0 step 4).
 * 0.22.4: TestWorkspace (not DocCollection). getStore does not take schema;
 * flavours come from store extensions. Do not pass storeExtensions into
 * getStore — TestDoc.getStore already concatenates workspace.storeExtensions.
 *
 * Sync: pass a SyncProvider (default MemoryNoopProvider). M1 live provider
 * is OctoBaseKeckProvider from providerFromEnv(); mount-editor stays unaware.
 */
export function createM0Workspace(provider = new MemoryNoopProvider()) {
  const manager = new StoreExtensionManager(getInternalStoreExtensions());
  const workspace = new TestWorkspace({ id: 'venus-m0' });
  workspace.storeExtensions = manager.get('store');
  workspace.meta.initialize();

  const docId = 'doc:home';
  const doc = workspace.createDoc(docId);
  const store = doc.getStore();

  store.load(() => {
    const pageId = store.addBlock('affine:page', {
      title: new Text(SEED_TITLE),
    });
    store.addBlock('affine:surface', {}, pageId);
    const noteId = store.addBlock('affine:note', {}, pageId);
    store.addBlock('affine:paragraph', {}, noteId);
    seedHomeNote(store, noteId);
  });

  store.resetHistory();
  provider.connect(docId, store.spaceDoc);

  return { workspace, store, docId, provider };
}
