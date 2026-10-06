import { expect, test } from 'vitest';
import * as Y from 'yjs';
import { createM0Workspace } from '../workspace.js';
import { createDoc, createFolder, seedOnce } from './ops.js';
import {
  catalogTreeChildIds,
  repairCatalogStructure,
  UNFILED_ID,
} from './repair-structure.js';
import { childrenIndex, FOLDER_SPEC_ID, getNode, nodesMap, putNode } from './schema.js';
import { WIKI_ROOT_ID } from './drop.js';
import { listenCatalogHost } from './listen.js';

async function seeded() {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace };
}

test('orphan publishes at the root, shows under Unfiled, then repair files it', async () => {
  const { catalog, workspace } = await seeded();
  const folder = createFolder(catalog, { createAt: null, name: 'drawer' });
  const inner = createFolder(catalog, { createAt: folder.id, name: 'inner' });
  const page = createDoc(catalog, workspace, { createAt: inner.id });
  nodesMap(catalog).delete(folder.id);

  expect(getNode(catalog, inner.id)?.parentId).toBe(folder.id);
  expect(getNode(catalog, inner.id)?.gitPath).toBe(inner.gitName);
  expect(getNode(catalog, page.id)?.parentId).toBe(inner.id);
  expect(getNode(catalog, page.id)?.gitPath).toBe(`${inner.gitName}/${page.id}.md`);

  const hidden = childrenIndex(catalog);
  const rootBefore = catalogTreeChildIds(catalog, WIKI_ROOT_ID, hidden);
  expect(rootBefore).toContain(UNFILED_ID);
  expect(rootBefore).not.toContain(inner.id);
  expect(catalogTreeChildIds(catalog, UNFILED_ID, hidden)).toEqual([inner.id]);
  expect(catalogTreeChildIds(catalog, inner.id, hidden)).toEqual([page.id]);

  expect(repairCatalogStructure(catalog)).toBe(true);
  expect(getNode(catalog, inner.id)?.parentId).toBe(null);
  expect(getNode(catalog, page.id)?.parentId).toBe(inner.id);
  expect(getNode(catalog, page.id)?.gitPath).toBe(`${inner.gitName}/${page.id}.md`);
  const shown = childrenIndex(catalog);
  expect(catalogTreeChildIds(catalog, WIKI_ROOT_ID, shown)).not.toContain(UNFILED_ID);
  expect(catalogTreeChildIds(catalog, WIKI_ROOT_ID, shown)).toContain(inner.id);
  expect(repairCatalogStructure(catalog)).toBe(false);
});

test('cycle breaks at the greatest id and keeps the other attached', async () => {
  const { catalog } = await seeded();
  const x = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'x' });
  const y = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'y' });
  putNode(catalog, { id: x.id, parentId: y.id });
  putNode(catalog, { id: y.id, parentId: x.id });
  const high = x.id > y.id ? x : y;
  const low = high.id === x.id ? y : x;

  expect(getNode(catalog, high.id)?.gitPath).toBe(high.gitName);
  expect(getNode(catalog, low.id)?.gitPath).toBe(`${high.gitName}/${low.gitName}`);

  expect(repairCatalogStructure(catalog)).toBe(true);
  expect(getNode(catalog, high.id)?.parentId).toBe(null);
  expect(getNode(catalog, low.id)?.parentId).toBe(high.id);
  expect(getNode(catalog, low.id)?.gitPath).toBe(`${high.gitName}/${low.gitName}`);
  expect(repairCatalogStructure(catalog)).toBe(false);
});

test('self-parent and a tail into a cycle break only the cycle', async () => {
  const { catalog } = await seeded();
  const self = createFolder(catalog, { createAt: null, name: 'loop' });
  putNode(catalog, { id: self.id, parentId: self.id });
  expect(getNode(catalog, self.id)?.gitPath).toBe(self.gitName);
  expect(repairCatalogStructure(catalog)).toBe(true);
  expect(getNode(catalog, self.id)?.parentId).toBe(null);

  const b = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'b' });
  const c = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'c' });
  const a = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'a' });
  putNode(catalog, { id: b.id, parentId: c.id });
  putNode(catalog, { id: c.id, parentId: b.id });
  putNode(catalog, { id: a.id, parentId: b.id });
  const high = b.id > c.id ? b : c;
  const low = high.id === b.id ? c : b;
  expect(repairCatalogStructure(catalog)).toBe(true);
  expect(getNode(catalog, high.id)?.parentId).toBe(null);
  expect(getNode(catalog, low.id)?.parentId).toBe(high.id);
  expect(getNode(catalog, a.id)?.parentId).toBe(b.id);
  const aPath =
    high.id === b.id
      ? `${b.gitName}/${a.gitName}`
      : `${c.gitName}/${b.gitName}/${a.gitName}`;
  expect(getNode(catalog, a.id)?.gitPath).toBe(aPath);
});

test('listenCatalogHost repairs an orphan after a microtask', async () => {
  const { catalog, workspace } = await seeded();
  const folder = createFolder(catalog, { createAt: null, name: 'drawer' });
  const page = createDoc(catalog, workspace, { createAt: folder.id });
  nodesMap(catalog).delete(folder.id);
  const stop = listenCatalogHost(catalog, () => 'doc:home', {
    onTitle: () => {},
    onMissingOpen: () => {},
    onChange: () => {},
  });
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(getNode(catalog, page.id)?.parentId).toBe(null);
  await new Promise((resolve) => setTimeout(resolve, 0));
  expect(getNode(catalog, page.id)?.parentId).toBe(null);
  stop();
});
