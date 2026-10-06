import { expect, test } from 'vitest';
import * as Y from 'yjs';
import { PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import { WIKI_ROOT_ID } from './drop.js';
import { createDoc, deleteNode, seedOnce } from './ops.js';
import { FOLDER_SPEC_ID } from './schema.js';
import {
  isRenamingGone,
  keepSelectedIds,
  pruneGoneTreeItems,
} from './tree-prune.js';

test('keepSelectedIds drops deleted ids; wiki root stays', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(
    keepSelectedIds(catalog, [WIKI_ROOT_ID, PAGE_DOC_ID, created.id, 'gone']),
  ).toEqual([WIKI_ROOT_ID, PAGE_DOC_ID, created.id]);
  deleteNode(catalog, created.id);
  expect(keepSelectedIds(catalog, [created.id, PAGE_DOC_ID])).toEqual([
    PAGE_DOC_ID,
  ]);
});

test('isRenamingGone is true only for a missing rename target', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(isRenamingGone(catalog, null)).toBe(false);
  expect(isRenamingGone(catalog, created.id)).toBe(false);
  deleteNode(catalog, created.id);
  expect(isRenamingGone(catalog, created.id)).toBe(true);
});

test('pruneGoneTreeItems clears selection and aborts rename', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  let selectedItems = [created.id, PAGE_DOC_ID];
  let renamingItem: string | null = created.id;
  const tree = {
    getState() {
      return { selectedItems, renamingItem };
    },
    setSelectedItems(ids: string[]) {
      selectedItems = ids;
    },
    abortRenaming() {
      renamingItem = null;
    },
  };
  pruneGoneTreeItems(tree, catalog);
  expect(selectedItems).toEqual([created.id, PAGE_DOC_ID]);
  expect(renamingItem).toBe(created.id);
  deleteNode(catalog, created.id);
  pruneGoneTreeItems(tree, catalog);
  expect(selectedItems).toEqual([PAGE_DOC_ID]);
  expect(renamingItem).toBeNull();
});
