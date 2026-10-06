import * as Y from 'yjs';
import { expect, test } from 'vitest';
import { PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import { createDoc, createFolder, seedOnce } from './ops.js';
import {
  FOLDER_SPEC_ID,
  KIND_FOLDER,
  childrenIndex,
  childrenOf,
  getNode,
  hasChild,
  nodesMap,
  putNode,
} from './schema.js';

test('childrenIndex is one scan; childrenOf reuses it', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const crdt = createFolder(catalog, {
    createAt: FOLDER_SPEC_ID,
    name: 'crdt',
  });
  createDoc(catalog, workspace, { createAt: crdt.id });

  const index = childrenIndex(catalog);
  expect(childrenOf(catalog, FOLDER_SPEC_ID, index).map((n) => n.id).sort()).toEqual(
    [PAGE_DOC_ID, crdt.id].sort(),
  );
  expect(childrenOf(catalog, crdt.id, index)).toHaveLength(1);
  expect(childrenOf(catalog, null, index).map((n) => n.id)).toEqual([
    FOLDER_SPEC_ID,
  ]);
  expect(hasChild(catalog, FOLDER_SPEC_ID)).toBe(true);
  expect(hasChild(catalog, crdt.id)).toBe(true);
  expect(hasChild(catalog, PAGE_DOC_ID)).toBe(false);
  expect(hasChild(catalog, FOLDER_SPEC_ID, index)).toBe(true);
  expect(hasChild(catalog, crdt.id, index)).toBe(true);
  expect(hasChild(catalog, PAGE_DOC_ID, index)).toBe(false);
  expect(hasChild(catalog, 'missing', index)).toBe(false);
});

test('childrenIndex skips gitPath join; getNode still joins', async () => {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const index = childrenIndex(catalog);
  const home = childrenOf(catalog, FOLDER_SPEC_ID, index).find(
    (n) => n.id === PAGE_DOC_ID,
  );
  expect(home?.gitName).toBe('home.md');
  expect(home?.gitPath).toBe('home.md');
  expect(getNode(catalog, PAGE_DOC_ID, { gitPath: false })?.gitPath).toBe(
    'home.md',
  );
  expect(getNode(catalog, PAGE_DOC_ID)?.gitPath).toBe('spec/home.md');
});

test('childrenOf tie-breaks equal order by id (total order)', () => {
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  putNode(catalog, {
    id: 'uuid-2',
    kind: KIND_FOLDER,
    name: 'two',
    parentId: FOLDER_SPEC_ID,
    order: 'a1',
    gitName: 'two',
  });
  putNode(catalog, {
    id: 'uuid-1',
    kind: KIND_FOLDER,
    name: 'one',
    parentId: FOLDER_SPEC_ID,
    order: 'a1',
    gitName: 'one',
  });
  putNode(catalog, {
    id: 'uuid-0',
    kind: KIND_FOLDER,
    name: 'zero',
    parentId: FOLDER_SPEC_ID,
    order: 'a0',
    gitName: 'zero',
  });

  const ids = childrenOf(catalog, FOLDER_SPEC_ID).map((n) => n.id);
  expect(ids).toEqual(['uuid-0', 'uuid-1', 'uuid-2']);
  expect(childrenOf(catalog, FOLDER_SPEC_ID).map((n) => n.id)).toEqual(ids);
});

test('gitPathOf joins gitNames; leftover gitPath is leaf-only', () => {
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  putNode(catalog, {
    id: FOLDER_SPEC_ID,
    kind: KIND_FOLDER,
    name: 'SPEC',
    parentId: null,
    order: 'a0',
    gitName: 'SPEC',
  });
  const page = new Y.Map();
  page.set('id', 'p');
  page.set('kind', 'doc');
  page.set('name', 'protocol');
  page.set('parentId', FOLDER_SPEC_ID);
  page.set('order', 'a1');
  page.set('gitPath', 'spec/stale/protocol.md');
  nodesMap(catalog).set('p', page);

  expect(getNode(catalog, FOLDER_SPEC_ID)?.gitPath).toBe('SPEC');
  expect(getNode(catalog, 'p')?.gitPath).toBe('SPEC/protocol.md');
  expect(getNode(catalog, 'p')?.gitName).toBe('protocol.md');
});
