import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

import { sidecarUrlFromEnv } from './providers/from-env.js';

const here = dirname(fileURLToPath(import.meta.url));

test('Flush chrome is host App, not mount-editor; no store.readonly', () => {
  const app = readFileSync(join(here, '../App.tsx'), 'utf8');
  const editor = readFileSync(join(here, 'mount-editor.js'), 'utf8');
  const container = readFileSync(join(here, 'editor-container.js'), 'utf8');
  const boot = readFileSync(join(here, 'boot.js'), 'utf8');

  expect(app).toMatch(/data-testid=["']venus-flush["']/);
  expect(app).toMatch(/data-testid=["']venus-git-log["']/);
  expect(app).toMatch(/SIDECAR_URL.*\/flush/);
  expect(app).toMatch(/\/git\/log\?path=/);
  expect(app).toMatch(/void fetch\(`\$\{SIDECAR_URL\}\/flush`/);
  expect(app).not.toMatch(/async function onFlush/);
  expect(app).not.toMatch(/await fetch/);
  expect(app).not.toMatch(/store\.readonly/);
  expect(app).not.toMatch(/store\.history/);
  expect(app).not.toMatch(/readonly\s*=\s*true/);
  expect(app).not.toMatch(/isomorphic-git|simple-git|wiki\/\.git/);

  for (const [name, src] of [
    ['mount-editor.js', editor],
    ['editor-container.js', container],
    ['boot.js', boot],
  ] as const) {
    expect(src, name).not.toMatch(/venus-flush/);
    expect(src, name).not.toMatch(/venus-git-log/);
    expect(src, name).not.toMatch(/\/flush/);
    expect(src, name).not.toMatch(/\/git\/log/);
    expect(src, name).not.toMatch(/git2|simple-git|git commit|isomorphic-git/);
    expect(src, name).not.toMatch(/venus-sidecar/);
    expect(src, name).not.toMatch(/from-doc\.js|pin-from-doc/);
  }
});

test('sidecarUrlFromEnv is empty without VITE_SIDECAR_URL', () => {
  expect(sidecarUrlFromEnv({})).toBe('');
  expect(sidecarUrlFromEnv({ VITE_SIDECAR_URL: '  ' })).toBe('');
  expect(sidecarUrlFromEnv({ VITE_SIDECAR_URL: 'http://127.0.0.1:3002/' })).toBe(
    'http://127.0.0.1:3002',
  );
});
