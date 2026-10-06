import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import * as Y from 'yjs';
import { PAGE_DOC_ID } from '../ids.js';
import { createM0Workspace } from '../workspace.js';
import {
  applyCatalogHostChrome,
  listenCatalogHost,
  openNodeTitle,
  shouldAutoHome,
} from './listen.js';
import { createDoc, createFolder, deleteNode, rename, seedOnce } from './ops.js';
import { FOLDER_SPEC_ID, nodesMap } from './schema.js';

const here = dirname(fileURLToPath(import.meta.url));

async function seeded() {
  const { workspace } = await createM0Workspace();
  const catalog = new Y.Doc({ guid: 'venus:catalog' });
  seedOnce(catalog, workspace);
  return { catalog, workspace };
}

test('openNodeTitle and shouldAutoHome follow the open catalog node', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  expect(openNodeTitle(catalog, PAGE_DOC_ID)).toBe('home');
  expect(shouldAutoHome(catalog, PAGE_DOC_ID)).toBe(false);
  expect(openNodeTitle(catalog, page.id)).toBe(page.name);
  expect(shouldAutoHome(catalog, page.id)).toBe(false);
  deleteNode(catalog, page.id);
  expect(openNodeTitle(catalog, page.id)).toBe('');
  expect(shouldAutoHome(catalog, page.id)).toBe(true);
  expect(shouldAutoHome(catalog, PAGE_DOC_ID)).toBe(false);
});

test('listenCatalogHost is one observeDeep for title, auto-home, and change', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  let openId: string = page.id;
  const titles: string[] = [];
  let missing = 0;
  let changes = 0;
  const nodes = nodesMap(catalog);
  const observe = nodes.observeDeep.bind(nodes);
  let attaches = 0;
  nodes.observeDeep = (fn) => {
    attaches += 1;
    observe(fn);
  };
  const stop = listenCatalogHost(catalog, () => openId, {
    onTitle: (title) => {
      titles.push(title);
    },
    onMissingOpen: () => {
      missing += 1;
    },
    onChange: () => {
      changes += 1;
    },
  });
  expect(attaches).toBe(1);
  expect(titles.at(-1)).toBe(page.name);
  expect(changes).toBe(0);
  expect(missing).toBe(0);

  rename(catalog, workspace, page.id, 'renamed');
  expect(titles.at(-1)).toBe('renamed');
  expect(changes).toBe(1);
  expect(missing).toBe(0);

  createFolder(catalog, { createAt: null, name: 'design' });
  expect(titles.at(-1)).toBe('renamed');
  expect(changes).toBe(2);
  expect(missing).toBe(0);

  openId = PAGE_DOC_ID;
  applyCatalogHostChrome(catalog, openId, {
    onTitle: (title) => {
      titles.push(title);
    },
    onMissingOpen: () => {
      missing += 1;
    },
  });
  expect(titles.at(-1)).toBe('home');
  expect(missing).toBe(0);

  openId = page.id;
  deleteNode(catalog, page.id);
  expect(missing).toBe(1);
  expect(changes).toBe(3);
  stop();
});

test('listenCatalogHost reads getOpenDocId live without resubscribing', async () => {
  const { catalog, workspace } = await seeded();
  const page = createDoc(catalog, workspace, { createAt: FOLDER_SPEC_ID });
  let openId: string = PAGE_DOC_ID;
  const titles: string[] = [];
  const stop = listenCatalogHost(catalog, () => openId, {
    onTitle: (title) => {
      titles.push(title);
    },
    onMissingOpen: () => {},
    onChange: () => {},
  });
  expect(titles.at(-1)).toBe('home');
  openId = page.id;
  rename(catalog, workspace, page.id, 'live-open');
  expect(titles.at(-1)).toBe('live-open');
  stop();
});

test('App uses one listenCatalogHost; CatalogTree does not observeDeep', () => {
  const app = readFileSync(join(here, '../../App.tsx'), 'utf8');
  const tree = readFileSync(join(here, 'CatalogTree.tsx'), 'utf8');
  expect(app).toMatch(/listenCatalogHost/);
  expect(app).toMatch(/applyCatalogHostChrome/);
  expect(app).not.toMatch(/observeDeep/);
  expect(app).not.toMatch(/unobserveDeep/);
  expect(tree).not.toMatch(/observeDeep/);
  expect(tree).not.toMatch(/unobserveDeep/);
  expect(tree).not.toMatch(/setRev/);
  expect(tree).toMatch(/rebuildRef/);
  expect(tree).toMatch(/memo\(function CatalogTree/);
});
