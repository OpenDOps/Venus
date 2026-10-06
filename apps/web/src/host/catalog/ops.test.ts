import { generateKeyBetween } from 'fractional-indexing';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Y from 'yjs';
import { expect, test } from 'vitest';
import { PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import type { SyncProvider } from '../sync-provider.js';
import { filenameOfGitPath } from './git-path.js';
import {
  createDoc,
  createFolder,
  CatalogError,
  deleteNode,
  reparent,
  rename,
  seedOnce,
  setOrder,
} from './ops.js';
import { FOLDER_SPEC_ID, childrenOf, getNode, listNodes, nodesMap } from './schema.js';

const here = dirname(fileURLToPath(import.meta.url));

async function seededCatalog() {
  const { workspace, store } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace, store };
}

test('seed catalog writes folder:spec + doc:home once; no protocol', async () => {
  const { catalog, workspace } = await seededCatalog();
  const spec = getNode(catalog, FOLDER_SPEC_ID);
  const home = getNode(catalog, PAGE_DOC_ID);
  expect(spec).toMatchObject({
    id: FOLDER_SPEC_ID,
    kind: 'folder',
    name: 'spec',
    parentId: null,
    gitPath: 'spec',
  });
  expect(home).toMatchObject({
    id: PAGE_DOC_ID,
    kind: 'doc',
    name: 'home',
    parentId: FOLDER_SPEC_ID,
    docId: PAGE_DOC_ID,
    gitPath: 'spec/home.md',
  });
  expect(listNodes(catalog).some((n) => n.id === 'doc:protocol')).toBe(false);
  expect(workspace.docs.size).toBe(1);
  expect(workspace.meta.getDocMeta?.(PAGE_DOC_ID)?.title).toBe('home');

  seedOnce(catalog, workspace);
  expect(listNodes(catalog)).toHaveLength(2);
  expect(getNode(catalog, FOLDER_SPEC_ID)?.gitPath).toBe('spec');

  const ops = readFileSync(join(here, 'ops.js'), 'utf8');
  expect(ops).not.toMatch(/doc:protocol/);
  expect(ops).not.toMatch(/refreshSubtreeGitPaths/);
  expect(ops).not.toMatch(/openWorkspaceDoc/);
  expect(ops).not.toMatch(/\.connect\(/);
  const pkg = JSON.parse(
    readFileSync(join(here, '../../../package.json'), 'utf8'),
  ) as { dependencies: Record<string, string> };
  expect(pkg.dependencies['fractional-indexing']).toBe('3.4.0');
  expect(ops).toMatch(/from 'fractional-indexing'/);
});

test('createDoc then rename: uuid.md then protocol.md; slash and sibling _1', async () => {
  const { catalog, workspace } = await seededCatalog();
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(created.id).toBe(created.docId);
  expect(created.id).not.toBe('doc:protocol');
  expect(created.gitPath).toBe(`spec/${created.id}.md`);
  expect(workspace.docs.size).toBe(2);
  expect(workspace.meta.getDocMeta?.(created.id)?.title).toBe(created.id);
  expect(workspace.getDoc?.(created.id)?.getStore()?.root?.flavour).toBe(
    'affine:page',
  );

  const renamed = rename(catalog, workspace, created.id, 'protocol');
  expect(renamed.id).toBe(created.id);
  expect(renamed.docId).toBe(created.id);
  expect(renamed.name).toBe('protocol');
  expect(renamed.gitPath).toBe('spec/protocol.md');
  expect(workspace.meta.getDocMeta?.(created.id)?.title).toBe('protocol');

  const slashed = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const slashName = rename(catalog, workspace, slashed.id, 'foo/bar');
  expect(slashName.name).toBe('foo/bar');
  expect(slashName.gitPath).toBe('spec/foo_bar.md');

  const clash = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const clashed = rename(catalog, workspace, clash.id, 'protocol');
  expect(clashed.gitPath).toBe('spec/protocol_1.md');
});

test('createDoc mints a workspace doc and does not connect a page socket', async () => {
  const connected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: true,
    connect(id) {
      connected.push(id);
    },
    disconnect() {},
    whenReady() {
      return Promise.resolve();
    },
  };
  const { workspace } = await createM0Workspace(provider);
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  const before = [...connected];
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(connected).toEqual(before);
  expect(connected).not.toContain(created.id);
  expect(workspace.getDoc?.(created.id) ?? workspace.docs.has(created.id)).toBeTruthy();
});

test('rename empty / whitespace keeps the previous name', async () => {
  const { catalog, workspace } = await seededCatalog();
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  rename(catalog, workspace, created.id, 'protocol');
  const before = getNode(catalog, created.id);
  const titleBefore = workspace.meta.getDocMeta?.(created.id)?.title;

  const blank = rename(catalog, workspace, created.id, '   \n\t');
  expect(blank).toEqual(before);
  expect(getNode(catalog, created.id)).toEqual(before);
  expect(workspace.meta.getDocMeta?.(created.id)?.title).toBe(titleBefore);

  const homeBefore = getNode(catalog, PAGE_DOC_ID);
  expect(rename(catalog, workspace, PAGE_DOC_ID, '').name).toBe('home');
  expect(getNode(catalog, PAGE_DOC_ID)).toEqual(homeBefore);
  expect(rename(catalog, workspace, PAGE_DOC_ID, 'home')).toEqual(homeBefore);
});

test('reparent moves gitPath not page body bytes', async () => {
  const { catalog, workspace } = await seededCatalog();
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  rename(catalog, workspace, created.id, 'protocol');
  const pageDoc = workspace.getDoc?.(created.id)?.spaceDoc;
  if (!pageDoc) throw new Error('createDoc did not mint a workspace page');
  pageDoc.getMap('probe').set('k', 'v');
  const before = Y.encodeStateAsUpdate(pageDoc);

  const design = createFolder(catalog, { createAt: null, name: 'design' });
  expect(design.gitPath).toBe('design');
  const moved = reparent(catalog, created.id, { parentId: design.id });
  expect(moved.gitPath).toBe('design/protocol.md');
  expect(getNode(catalog, PAGE_DOC_ID)?.gitPath).toBe('spec/home.md');
  expect(getNode(catalog, PAGE_DOC_ID)?.parentId).toBe(FOLDER_SPEC_ID);

  const after = Y.encodeStateAsUpdate(pageDoc);
  expect(after).toEqual(before);
  expect(pageDoc.getMap('probe').get('k')).toBe('v');
});

test('gitPath follows ancestor filenames; home reparent is home_protected', async () => {
  const { catalog, workspace } = await seededCatalog();
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const crdt = createFolder(catalog, {
    createAt: FOLDER_SPEC_ID,
    name: 'crdt',
  });
  expect(crdt.gitPath).toBe('spec/crdt');
  const nested = reparent(catalog, created.id, { parentId: crdt.id });
  const filename = filenameOfGitPath(nested.gitPath, 'doc');
  expect(nested.gitPath).toBe(`spec/crdt/${filename}.md`);

  rename(catalog, workspace, FOLDER_SPEC_ID, 'SPEC');
  expect(getNode(catalog, FOLDER_SPEC_ID)?.gitPath).toBe('SPEC');
  expect(getNode(catalog, FOLDER_SPEC_ID)?.id).toBe(FOLDER_SPEC_ID);
  expect(getNode(catalog, PAGE_DOC_ID)?.parentId).toBe(FOLDER_SPEC_ID);
  expect(getNode(catalog, PAGE_DOC_ID)?.gitPath).toBe('SPEC/home.md');
  expect(getNode(catalog, crdt.id)?.gitPath).toBe('SPEC/crdt');
  expect(getNode(catalog, created.id)?.gitPath).toBe(`SPEC/crdt/${filename}.md`);

  const homeBefore = getNode(catalog, PAGE_DOC_ID);
  expect(() => reparent(catalog, PAGE_DOC_ID, { parentId: crdt.id })).toThrow(
    CatalogError,
  );
  try {
    reparent(catalog, PAGE_DOC_ID, { parentId: crdt.id });
    throw new Error('expected home_protected');
  } catch (err) {
    expect(err).toBeInstanceOf(CatalogError);
    if (err instanceof CatalogError) expect(err.code).toBe('home_protected');
  }
  expect(getNode(catalog, PAGE_DOC_ID)).toEqual(homeBefore);
});

test('delete rejects non-empty folder and home; leaf page is catalog-only', async () => {
  const { catalog, workspace } = await seededCatalog();
  const created = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const before = listNodes(catalog).map((n) => n.id).sort();

  expect(() => deleteNode(catalog, FOLDER_SPEC_ID)).toThrow(CatalogError);
  try {
    deleteNode(catalog, FOLDER_SPEC_ID);
    throw new Error('expected node_not_empty');
  } catch (err) {
    expect(err).toBeInstanceOf(CatalogError);
    if (err instanceof CatalogError) expect(err.code).toBe('node_not_empty');
  }
  expect(listNodes(catalog).map((n) => n.id).sort()).toEqual(before);

  deleteNode(catalog, created.id);
  expect(getNode(catalog, created.id)).toBeNull();
  expect(getNode(catalog, PAGE_DOC_ID)?.gitPath).toBe('spec/home.md');
  expect(workspace.getDoc?.(created.id)).toBeTruthy();

  expect(() => deleteNode(catalog, PAGE_DOC_ID)).toThrow(CatalogError);
  try {
    deleteNode(catalog, PAGE_DOC_ID);
    throw new Error('expected home_protected');
  } catch (err) {
    expect(err).toBeInstanceOf(CatalogError);
    if (err instanceof CatalogError) expect(err.code).toBe('home_protected');
  }
  expect(getNode(catalog, PAGE_DOC_ID)?.id).toBe(PAGE_DOC_ID);
});

test('merge cycle X⇄Y: rename skips the loop and returns', async () => {
  const { catalog, workspace } = await seededCatalog();
  const x = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'x' });
  const y = createFolder(catalog, { createAt: FOLDER_SPEC_ID, name: 'y' });

  const replica = new Y.Doc({ guid: catalog.guid });
  Y.applyUpdate(replica, Y.encodeStateAsUpdate(catalog));
  const sv = Y.encodeStateVector(catalog);

  reparent(catalog, x.id, { parentId: y.id });
  reparent(replica, y.id, { parentId: x.id });

  Y.applyUpdate(catalog, Y.encodeStateAsUpdate(replica, sv));
  Y.applyUpdate(replica, Y.encodeStateAsUpdate(catalog, sv));

  expect(getNode(catalog, x.id)?.parentId).toBe(y.id);
  expect(getNode(catalog, y.id)?.parentId).toBe(x.id);

  const renamed = rename(catalog, workspace, x.id, 'X');
  expect(renamed.id).toBe(x.id);
  expect(renamed.name).toBe('X');
  expect(getNode(catalog, x.id)?.parentId).toBe(y.id);
  expect(getNode(catalog, y.id)?.parentId).toBe(x.id);
});

test('concurrent createDoc: duplicate order is a fixed permutation; setOrder between twins does not throw', async () => {
  expect(() => generateKeyBetween('a1', 'a1')).toThrow();

  const { catalog, workspace } = await seededCatalog();
  const { workspace: workspaceB } = await createM0Workspace();
  const replica = new Y.Doc({ guid: catalog.guid });
  Y.applyUpdate(replica, Y.encodeStateAsUpdate(catalog));
  const sv = Y.encodeStateVector(catalog);

  const a = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const b = createDoc(replica, workspaceB, { createAt: FOLDER_SPEC_ID });
  expect(a.order).toBe(b.order);

  Y.applyUpdate(catalog, Y.encodeStateAsUpdate(replica, sv));
  Y.applyUpdate(replica, Y.encodeStateAsUpdate(catalog, sv));

  expect(getNode(catalog, a.id)?.order).toBe(getNode(catalog, b.id)?.order);

  const twins = childrenOf(catalog, FOLDER_SPEC_ID).filter(
    (n) => n.order === a.order,
  );
  expect(twins.map((n) => n.id)).toEqual([a.id, b.id].sort());

  const ids = childrenOf(catalog, FOLDER_SPEC_ID).map((n) => n.id);
  expect(childrenOf(catalog, FOLDER_SPEC_ID).map((n) => n.id)).toEqual(ids);
  expect(childrenOf(replica, FOLDER_SPEC_ID).map((n) => n.id)).toEqual(ids);

  const placed = setOrder(catalog, twins[0].id, {
    afterId: twins[0].id,
    beforeId: twins[1].id,
  });
  expect(placed.id).toBe(twins[0].id);
  expect(placed.order).not.toBe(twins[1].order);
});

test('concurrent ancestor + child rename: read-time gitPath joins gitNames', async () => {
  const { catalog, workspace } = await seededCatalog();
  const crdt = createFolder(catalog, {
    createAt: FOLDER_SPEC_ID,
    name: 'crdt',
  });
  const page = createDoc(catalog, workspace, { createAt: crdt.id });

  const { workspace: workspaceB } = await createM0Workspace();
  const replica = new Y.Doc({ guid: catalog.guid });
  Y.applyUpdate(replica, Y.encodeStateAsUpdate(catalog));
  const sv = Y.encodeStateVector(catalog);

  rename(catalog, workspace, FOLDER_SPEC_ID, 'SPEC');
  rename(replica, workspaceB, page.id, 'protocol');

  expect(nodesMap(catalog).get(crdt.id)?.get('gitName')).toBe('crdt');
  expect(nodesMap(catalog).get(page.id)?.get('gitName')).toBe(`${page.id}.md`);
  expect(nodesMap(replica).get(FOLDER_SPEC_ID)?.get('gitName')).toBe('spec');

  Y.applyUpdate(catalog, Y.encodeStateAsUpdate(replica, sv));
  Y.applyUpdate(replica, Y.encodeStateAsUpdate(catalog, sv));

  expect(getNode(catalog, page.id)?.gitPath).toBe('SPEC/crdt/protocol.md');
  expect(getNode(replica, page.id)?.gitPath).toBe('SPEC/crdt/protocol.md');
  expect(getNode(catalog, crdt.id)?.gitPath).toBe('SPEC/crdt');
  expect(getNode(catalog, PAGE_DOC_ID)?.gitPath).toBe('SPEC/home.md');

  expect(nodesMap(catalog).get(page.id)?.get('gitName')).toBe('protocol.md');
  expect(nodesMap(catalog).get(FOLDER_SPEC_ID)?.get('gitName')).toBe('SPEC');
  expect(nodesMap(catalog).get(crdt.id)?.get('gitName')).toBe('crdt');
  expect(nodesMap(catalog).get(page.id)?.has('gitPath')).toBe(false);
  expect(nodesMap(catalog).get(FOLDER_SPEC_ID)?.has('gitPath')).toBe(false);
});

test('legacy stored gitPath leaf still joins after ancestor rename', async () => {
  const { catalog, workspace } = await seededCatalog();
  const crdt = createFolder(catalog, {
    createAt: FOLDER_SPEC_ID,
    name: 'crdt',
  });
  const page = createDoc(catalog, workspace, { createAt: crdt.id });
  rename(catalog, workspace, page.id, 'protocol');

  const pageMap = nodesMap(catalog).get(page.id);
  pageMap?.delete('gitName');
  pageMap?.set('gitPath', 'spec/crdt/stale-prefix/protocol.md');

  rename(catalog, workspace, FOLDER_SPEC_ID, 'SPEC');
  expect(getNode(catalog, page.id)?.gitPath).toBe('SPEC/crdt/protocol.md');
});

test('setOrder / reparent reject afterId from another folder', async () => {
  const { catalog, workspace } = await seededCatalog();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  rename(catalog, workspace, page.id, 'protocol');
  const design = createFolder(catalog, { createAt: null, name: 'design' });
  const alpha = createFolder(catalog, { createAt: design.id, name: 'alpha' });

  const specBefore = childrenOf(catalog, FOLDER_SPEC_ID).map((n) => ({
    id: n.id,
    order: n.order,
    parentId: n.parentId,
  }));
  const designBefore = childrenOf(catalog, design.id).map((n) => ({
    id: n.id,
    order: n.order,
  }));

  try {
    setOrder(catalog, page.id, { afterId: alpha.id });
    throw new Error('expected invalid_parent');
  } catch (err) {
    expect(err).toBeInstanceOf(CatalogError);
    if (err instanceof CatalogError) expect(err.code).toBe('invalid_parent');
  }
  expect(
    childrenOf(catalog, FOLDER_SPEC_ID).map((n) => ({
      id: n.id,
      order: n.order,
      parentId: n.parentId,
    })),
  ).toEqual(specBefore);

  try {
    reparent(catalog, page.id, { parentId: design.id, afterId: PAGE_DOC_ID });
    throw new Error('expected invalid_parent');
  } catch (err) {
    expect(err).toBeInstanceOf(CatalogError);
    if (err instanceof CatalogError) expect(err.code).toBe('invalid_parent');
  }
  expect(getNode(catalog, page.id)?.parentId).toBe(FOLDER_SPEC_ID);
  expect(
    childrenOf(catalog, design.id).map((n) => ({ id: n.id, order: n.order })),
  ).toEqual(designBefore);

  try {
    setOrder(catalog, page.id, { afterId: 'missing-node' });
    throw new Error('expected invalid_parent');
  } catch (err) {
    expect(err).toBeInstanceOf(CatalogError);
    if (err instanceof CatalogError) expect(err.code).toBe('invalid_parent');
  }

  const moved = reparent(catalog, page.id, {
    parentId: design.id,
    afterId: alpha.id,
  });
  expect(moved.parentId).toBe(design.id);
  expect(moved.gitPath).toBe('design/protocol.md');
  const destIds = childrenOf(catalog, design.id).map((n) => n.id);
  expect(destIds.indexOf(alpha.id)).toBeLessThan(destIds.indexOf(page.id));
});


