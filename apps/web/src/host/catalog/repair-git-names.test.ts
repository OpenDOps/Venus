import { expect, test } from 'vitest';
import * as Y from 'yjs';
import { PAGE_DOC_ID, PAGE_SQL_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import { createDoc, createFolder, seedOnce } from './ops.js';
import { repairDuplicateGitNames } from './repair-git-names.js';
import { FOLDER_SPEC_ID, getNode, putNode } from './schema.js';

async function seeded() {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace };
}

test('repair keeps the lower id and suffixes the higher one', async () => {
  const { catalog, workspace } = await seeded();
  const a = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  const b = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  putNode(catalog, { id: a.id, gitName: 'notes.md' });
  putNode(catalog, { id: b.id, gitName: 'notes.md' });
  const [low, high] = [a.id, b.id].sort();
  expect(repairDuplicateGitNames(catalog)).toBe(true);
  expect(getNode(catalog, low)?.gitName).toBe('notes.md');
  expect(getNode(catalog, low)?.name).toBe(low);
  expect(getNode(catalog, high)?.gitName).toBe(`notes-${high.slice(0, 8)}.md`);
  expect(getNode(catalog, high)?.name).toBe(high);
  expect(repairDuplicateGitNames(catalog)).toBe(false);
  expect(getNode(catalog, high)?.gitName).toBe(`notes-${high.slice(0, 8)}.md`);
});

test('repair of home uses the SQL uuid prefix', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  putNode(catalog, { id: page.id, gitName: 'home.md' });
  expect(repairDuplicateGitNames(catalog)).toBe(true);
  const homeWins = PAGE_DOC_ID < page.id;
  if (homeWins) {
    expect(getNode(catalog, PAGE_DOC_ID)?.gitName).toBe('home.md');
    expect(getNode(catalog, page.id)?.gitName).toBe(
      `home-${page.id.slice(0, 8)}.md`,
    );
  } else {
    expect(getNode(catalog, page.id)?.gitName).toBe('home.md');
    expect(getNode(catalog, PAGE_DOC_ID)?.gitName).toBe(
      `home-${PAGE_SQL_ID.slice(0, 8)}.md`,
    );
  }
  expect(getNode(catalog, PAGE_DOC_ID)?.name).toBe('home');
});

test('repair suffixes a duplicate folder with the uuid after folder:', async () => {
  const { catalog } = await seeded();
  const left = createFolder(catalog, { createAt: null, name: 'Design' });
  const right = createFolder(catalog, { createAt: null, name: 'Other' });
  putNode(catalog, { id: left.id, gitName: 'Design' });
  putNode(catalog, { id: right.id, gitName: 'design' });
  const [low, high] = [left, right].sort((a, b) => (a.id < b.id ? -1 : 1));
  const highStem = high.id === left.id ? 'Design' : 'design';
  const tag = high.id.slice('folder:'.length, 'folder:'.length + 8);
  expect(repairDuplicateGitNames(catalog)).toBe(true);
  expect(getNode(catalog, low.id)?.gitName).toBe(
    low.id === left.id ? 'Design' : 'design',
  );
  expect(getNode(catalog, high.id)?.gitName).toBe(`${highStem}-${tag}`);
  expect(getNode(catalog, high.id)?.name).toBe(high.name);
});
