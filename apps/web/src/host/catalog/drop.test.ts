import { expect, test } from 'vitest';
import * as Y from 'yjs';
import { PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import {
  applyCatalogDrop,
  canCatalogDrop,
  destFromDrop,
  WIKI_ROOT_ID,
  catalogParentId,
} from './drop.js';
import { resolveOpenDocId } from './open-doc.js';
import {
  createDoc,
  createFolder,
  seedOnce,
} from './ops.js';
import { FOLDER_SPEC_ID, childrenOf, getNode } from './schema.js';

async function seeded() {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace };
}

test('destFromDrop appends without childIndex; inserts between siblings', async () => {
  const { catalog, workspace } = await seeded();
  const a = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const b = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const append = destFromDrop(catalog, [a.id], { parentId: FOLDER_SPEC_ID });
  expect(append).toEqual({ parentId: FOLDER_SPEC_ID });
  const first = destFromDrop(catalog, [b.id], {
    parentId: FOLDER_SPEC_ID,
    childIndex: 0,
  });
  expect(first.parentId).toBe(FOLDER_SPEC_ID);
  expect(first.afterId).toBeNull();
  expect(first.beforeId).toBe(PAGE_DOC_ID);
});

test('canCatalogDrop rejects home leaving spec and drop onto a doc', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const design = createFolder(catalog, { createAt: null, name: 'design' });
  expect(canCatalogDrop(catalog, [PAGE_DOC_ID], FOLDER_SPEC_ID)).toBe(true);
  expect(canCatalogDrop(catalog, [PAGE_DOC_ID], design.id)).toBe(false);
  expect(canCatalogDrop(catalog, [PAGE_DOC_ID], null)).toBe(false);
  expect(canCatalogDrop(catalog, [page.id], page.id)).toBe(false);
  expect(canCatalogDrop(catalog, [page.id], design.id)).toBe(true);
  expect(canCatalogDrop(catalog, [design.id], design.id)).toBe(false);
});

test('applyCatalogDrop reparents a page and refuses to move home', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const body = page.id;
  const design = createFolder(catalog, { createAt: null, name: 'design' });
  expect(
    applyCatalogDrop(catalog, [PAGE_DOC_ID], { parentId: design.id }),
  ).toBe(false);
  expect(getNode(catalog, PAGE_DOC_ID)?.parentId).toBe(FOLDER_SPEC_ID);
  expect(
    applyCatalogDrop(catalog, [body], { parentId: design.id }),
  ).toBe(true);
  expect(getNode(catalog, body)?.parentId).toBe(design.id);
  expect(getNode(catalog, body)?.gitPath).toBe(`design/${body}.md`);
});

test('wiki:root dest parent is null; resolveOpenDocId falls back to home', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(catalogParentId(WIKI_ROOT_ID)).toBeNull();
  expect(resolveOpenDocId(catalog, PAGE_DOC_ID)).toBe(PAGE_DOC_ID);
  expect(resolveOpenDocId(catalog, page.id)).toBe(page.id);
  expect(resolveOpenDocId(catalog, FOLDER_SPEC_ID)).toBe(PAGE_DOC_ID);
  expect(resolveOpenDocId(catalog, 'missing')).toBe(PAGE_DOC_ID);
});

test('applyCatalogDrop multi-item uses a fresh dest slot per id', async () => {
  const { catalog, workspace } = await seeded();
  const p1 = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const p2 = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const dest = destFromDrop(catalog, [p1.id, p2.id], {
    parentId: FOLDER_SPEC_ID,
    childIndex: 0,
  });
  expect(dest).toEqual({
    parentId: FOLDER_SPEC_ID,
    afterId: null,
    beforeId: PAGE_DOC_ID,
  });
  expect(applyCatalogDrop(catalog, [p1.id, p2.id], dest)).toBe(true);
  const ids = childrenOf(catalog, FOLDER_SPEC_ID).map((n) => n.id);
  expect(ids.slice(0, 2)).toEqual([p1.id, p2.id]);
  expect(ids).toContain(PAGE_DOC_ID);
  const o1 = getNode(catalog, p1.id)?.order;
  const o2 = getNode(catalog, p2.id)?.order;
  expect(o1).toBeTruthy();
  expect(o2).toBeTruthy();
  expect(o1).not.toBe(o2);
  expect(o1 < o2).toBe(true);
});

test('applyCatalogDrop multi-item reparent keeps drag order and distinct keys', async () => {
  const { catalog, workspace } = await seeded();
  const p1 = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const p2 = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const design = createFolder(catalog, { createAt: null, name: 'design' });
  const dest = destFromDrop(catalog, [p1.id, p2.id], {
    parentId: design.id,
    childIndex: 0,
  });
  expect(applyCatalogDrop(catalog, [p1.id, p2.id], dest)).toBe(true);
  const kids = childrenOf(catalog, design.id);
  expect(kids.map((n) => n.id)).toEqual([p1.id, p2.id]);
  expect(kids[0].order).not.toBe(kids[1].order);
  expect(kids[0].order < kids[1].order).toBe(true);
  expect(getNode(catalog, p1.id)?.parentId).toBe(design.id);
  expect(getNode(catalog, p2.id)?.parentId).toBe(design.id);
});

test('applyCatalogDrop multi-item is one catalog transaction', async () => {
  const { catalog, workspace } = await seeded();
  const p1 = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const p2 = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const dest = destFromDrop(catalog, [p1.id, p2.id], {
    parentId: FOLDER_SPEC_ID,
    childIndex: 0,
  });
  let updates = 0;
  const onUpdate = () => {
    updates += 1;
  };
  catalog.on('update', onUpdate);
  expect(applyCatalogDrop(catalog, [p1.id, p2.id], dest)).toBe(true);
  catalog.off('update', onUpdate);
  expect(updates).toBe(1);
});
