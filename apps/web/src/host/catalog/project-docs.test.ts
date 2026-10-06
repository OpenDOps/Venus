import * as Y from 'yjs';
import { expect, test } from 'vitest';
import { PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import { listenCatalogHost } from './listen.js';
import { createDoc, deleteNode, rename, seedOnce } from './ops.js';
import { projectCatalogDocs } from './project-docs.js';
import { FOLDER_SPEC_ID } from './schema.js';

async function seeded() {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace };
}

type PageDoc = {
  getStore: (opts?: { id?: string }) => {
    root: { flavour?: string; children?: unknown[] } | null;
    spaceDoc: Y.Doc;
    load: (fn?: () => void) => void;
  };
};

function blockCount(doc: PageDoc) {
  return doc.getStore().spaceDoc.getMap('blocks').size;
}

test('a second workspace gets an empty stub and the catalog title', async () => {
  const { catalog, workspace: creator } = await seeded();
  const page = createDoc(catalog, creator, { createAt: FOLDER_SPEC_ID });
  const creatorBytes = Y.encodeStateAsUpdate(
    creator.getDoc?.(page.id)?.getStore().spaceDoc as Y.Doc,
  );

  const { workspace: other } = await createM0Workspace();
  projectCatalogDocs(catalog, other, { keepId: PAGE_DOC_ID });

  const doc = other.getDoc?.(page.id) as unknown as (PageDoc | null);
  if (!doc) throw new Error('missing stub');
  expect(other.meta.getDocMeta?.(page.id)?.title).toBe(page.name);
  expect(other.meta.getDocMeta?.(PAGE_DOC_ID)?.title).toBe('home');
  expect(blockCount(doc)).toBe(0);
  expect(doc.getStore().root).toBeNull();
  const card = doc.getStore({ id: page.id });
  expect(card.root?.flavour).toBe('venus:stub');
  expect(card.root?.children).toEqual([]);
  card.load(() => {
    throw new Error('stub load must not run');
  });
  expect(blockCount(doc)).toBe(0);

  expect(
    Y.encodeStateAsUpdate(
      creator.getDoc?.(page.id)?.getStore().spaceDoc as Y.Doc,
    ),
  ).toEqual(creatorBytes);
});

test('rename updates the stub title; delete removes it unless it is open', async () => {
  const { catalog, workspace: creator } = await seeded();
  const page = createDoc(catalog, creator, { createAt: FOLDER_SPEC_ID });
  const { workspace: other } = await createM0Workspace();
  projectCatalogDocs(catalog, other, { keepId: PAGE_DOC_ID });

  rename(catalog, creator, page.id, 'protocol');
  projectCatalogDocs(catalog, other, { keepId: PAGE_DOC_ID });
  expect(other.meta.getDocMeta?.(page.id)?.title).toBe('protocol');
  expect(blockCount(other.getDoc?.(page.id) as unknown as PageDoc)).toBe(0);

  deleteNode(catalog, page.id);
  projectCatalogDocs(catalog, other, { keepId: page.id });
  expect(other.docs.has(page.id)).toBe(true);

  projectCatalogDocs(catalog, other, { keepId: PAGE_DOC_ID });
  expect(other.docs.has(page.id)).toBe(false);
  expect(other.getDoc?.(page.id) ?? null).toBeNull();
  expect(other.docs.has(PAGE_DOC_ID)).toBe(true);
});

test('listenCatalogHost projects stubs and does not seed them', async () => {
  const { catalog, workspace: creator } = await seeded();
  const { workspace: other } = await createM0Workspace();
  const stop = listenCatalogHost(
    catalog,
    () => PAGE_DOC_ID,
    {
      onTitle: () => {},
      onMissingOpen: () => {},
      onChange: () => {},
    },
    other,
  );
  expect(other.meta.getDocMeta?.(PAGE_DOC_ID)?.title).toBe('home');

  const page = createDoc(catalog, creator, { createAt: FOLDER_SPEC_ID });
  const doc = other.getDoc?.(page.id) as unknown as (PageDoc | null);
  if (!doc) throw new Error('listener did not project the page');
  expect(other.meta.getDocMeta?.(page.id)?.title).toBe(page.name);
  expect(blockCount(doc)).toBe(0);
  expect(doc.getStore({ id: page.id }).root?.flavour).toBe('venus:stub');

  rename(catalog, creator, page.id, 'renamed');
  expect(other.meta.getDocMeta?.(page.id)?.title).toBe('renamed');

  deleteNode(catalog, page.id);
  expect(other.docs.has(page.id)).toBe(false);
  stop();
});

test('a deleted page runs onRemoveDoc while its Y.Doc is still in the workspace', async () => {
  const { catalog, workspace: creator } = await seeded();
  const { workspace: other } = await createM0Workspace();
  const removed: Array<{ id: string; stillThere: boolean }> = [];
  const stop = listenCatalogHost(
    catalog,
    () => PAGE_DOC_ID,
    {
      onTitle: () => {},
      onMissingOpen: () => {},
      onChange: () => {},
      onRemoveDoc: (id) => {
        removed.push({ id, stillThere: other.docs.has(id) });
      },
    },
    other,
  );
  const page = createDoc(catalog, creator, { createAt: FOLDER_SPEC_ID });
  expect(removed).toEqual([]);
  deleteNode(catalog, page.id);
  expect(removed).toEqual([{ id: page.id, stillThere: true }]);
  expect(other.docs.has(page.id)).toBe(false);
  stop();
});

test('an in-progress page that is not in the catalog yet is left in place', async () => {
  const { catalog, workspace } = await seeded();
  projectCatalogDocs(catalog, workspace, { keepId: PAGE_DOC_ID });
  const id = crypto.randomUUID();
  workspace.createDoc(id);
  projectCatalogDocs(catalog, workspace, { keepId: PAGE_DOC_ID });
  expect(workspace.docs.has(id)).toBe(true);
  expect(blockCount(workspace.getDoc?.(id) as unknown as PageDoc)).toBe(0);
});
