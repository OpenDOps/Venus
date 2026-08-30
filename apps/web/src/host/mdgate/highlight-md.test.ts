import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import { highlight } from './highlight-md.js';

const here = dirname(fileURLToPath(import.meta.url));
const seedMd = readFileSync(join(here, 'goldens/seed.fromDoc.md'), 'utf8');

function decodeHighlighted(html: string) {
  return html
    .replace(/<[^>]+>/g, '')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#x27;/g, "'");
}

test('highlight wraps markdown tokens and round-trips seed bytes as text', () => {
  const html = highlight(seedMd);
  expect(html).toContain('<span');
  expect(html).toMatch(/hljs-/);
  expect(decodeHighlighted(html)).toBe(seedMd);
  expect(decodeHighlighted(html)).toContain('# Why Venus');
});
