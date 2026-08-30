import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

const hostDir = dirname(fileURLToPath(import.meta.url));

test('outline is BlockSuite heading TOC, not a wiki folder tree', () => {
  const mount = readFileSync(join(hostDir, 'mount-outline.js'), 'utf8');
  const app = readFileSync(join(hostDir, '../App.tsx'), 'utf8');

  expect(mount).toMatch(
    /from\s+['"]@blocksuite\/affine\/fragments\/outline['"]/,
  );
  expect(mount).toMatch(/new OutlinePanel\(/);
  expect(mount).toMatch(/panel\.editor\s*=/);
  expect(mount).not.toMatch(/folder|wiki sidebar|pages list/i);

  expect(app).toMatch(/outline-host/);
  expect(app).toMatch(/mountOutline/);
  expect(app).not.toMatch(/folder-tree|page-list|wiki-toc/i);
});
