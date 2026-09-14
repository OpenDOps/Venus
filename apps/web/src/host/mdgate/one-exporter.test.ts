import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

const here = dirname(fileURLToPath(import.meta.url));
const host = join(here, '..');

test('one-exporter: pane imports from-doc.js; exporter has no highlight.js; editor mount is ignorant', () => {
  const pane = readFileSync(join(here, 'mount-md-pane.js'), 'utf8');
  const exporter = readFileSync(join(here, 'from-doc.js'), 'utf8');
  const editor = readFileSync(join(host, 'mount-editor.js'), 'utf8');

  expect(pane).toMatch(/from ['"]\.\/from-doc\.js['"]/);
  expect(pane).toMatch(/from ['"]\.\/highlight-md\.js['"]/);
  expect(exporter).not.toMatch(/highlight\.js/);
  expect(editor).not.toMatch(/MarkdownAdapter/);
  expect(editor).not.toMatch(/mdgate\/from-doc/);
  expect(editor).not.toMatch(/from-doc\.js/);
  expect(editor).not.toMatch(/highlight\.js/);
  expect(editor).not.toMatch(/highlight-md/);
  expect(editor).not.toMatch(/venus-flush/);
  expect(editor).not.toMatch(/\/flush/);
});
