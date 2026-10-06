import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));

test('CatalogTree is a view: custom onDrop, no createOnDropHandler', () => {
  const src = readFileSync(join(here, 'CatalogTree.tsx'), 'utf8');
  expect(src).toMatch(/@headless-tree\/react/);
  expect(src).toMatch(/useTree/);
  expect(src).toMatch(/onDrop/);
  expect(src).toMatch(/applyCatalogDrop/);
  expect(src).toMatch(/canCatalogDrop/);
  expect(src).toMatch(/onPrimaryAction/);
  expect(src).toMatch(/venus-create-page/);
  expect(src).toMatch(/venus-tree-home/);
  expect(src).toMatch(/testidProps/);
  expect(src).not.toMatch(/createOnDropHandler/);
  expect(src).not.toMatch(/@affine\/core/);
  expect(src).not.toMatch(/git mv|git2|simple-git/);
  expect(src).not.toMatch(/openWorkspaceDoc/);
  expect(src).not.toMatch(/observeDeep/);
  expect(src).not.toMatch(/setRev/);
  expect(src).toMatch(/rebuildRef/);
  expect(src).toMatch(/memo\(function CatalogTree/);
});

test('CatalogTree CSS indents with --level, not per-row style objects', () => {
  const css = readFileSync(join(here, 'CatalogTree.css'), 'utf8');
  expect(css).toMatch(/--level/);
  expect(css).toMatch(/padding-inline-start:\s*calc\(var\(--level/);
  expect(css).toMatch(/data-level/);
  expect(css).toMatch(/venus-tree-chevron/);
});

test('CatalogTree rebuild uses one childrenIndex and skips gitPath join', () => {
  const src = readFileSync(join(here, 'CatalogTree.tsx'), 'utf8');
  expect(src).toMatch(/childrenIndex\(catalog\)/);
  expect(src).toMatch(/loadChildrenIndex\(\)/);
  expect(src).toMatch(/gitPath:\s*false/);
  expect(src).not.toMatch(/\.\.\.WIKI_ROOT,\s*id:/);
  expect(src).toMatch(/kind:\s*['"]unknown['"]/);
  expect(src).toMatch(/pruneGoneTreeItems/);
  expect(src).toMatch(/hasChild\(catalog, selectedNode\.id, loadChildrenIndex\(\)\)/);
  expect(src).toMatch(/data-catalog-expand/);
  expect(src).toMatch(/activateTreeRow/);
  expect(src).toMatch(/startRowRename/);
  expect(src).toMatch(/onDoubleClick/);
  expect(src).toMatch(/startRenaming/);
  expect(src).toMatch(/data-catalog-rename/);
  expect(src).toMatch(/PencilIcon/);
  expect(src).not.toMatch(/>Edit</);
  expect(src).toMatch(/event\.detail > 1/);
  expect(src).toMatch(/data-level/);
  expect(src).not.toMatch(/paddingLeft/);
  expect(src).not.toMatch(/style=\{\{ paddingLeft/);
});
