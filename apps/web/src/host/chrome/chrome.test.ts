import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import {
  debugFromEnv,
  testidProps,
  testidsFromEnv,
} from '../providers/from-env.js';
import { TITLE_COMMIT_MS, createTitleCommit } from './title-commit.js';

const here = dirname(fileURLToPath(import.meta.url));

test('VenusHeader subscribes to canUndo$ / canRedo$, does not poll', () => {
  const src = readFileSync(join(here, 'VenusHeader.tsx'), 'utf8');
  expect(src).toMatch(/canUndo\$/);
  expect(src).toMatch(/canRedo\$/);
  expect(src).toMatch(/\.subscribe\(/);
  expect(src).toMatch(/store\.undo\(\)/);
  expect(src).toMatch(/store\.redo\(\)/);
  expect(src).toMatch(/venus-page-title/);
  expect(src).toMatch(/onTitleChange/);
  expect(src).toMatch(/createTitleCommit/);
  expect(src).toMatch(/\.push\(/);
  expect(src).toMatch(/\.flush\(\)/);
  expect(src).not.toMatch(/onBlur=\{\(\) => \{\s*focusedRef\.current = false;\s*setDraft\(title\)/);
  expect(src).toMatch(/reconnecting/);
  expect(src).toMatch(/beforeunload/);
  expect(src).toMatch(/venus-connection/);
  expect(src).toMatch(/<input/);
  expect(src).not.toMatch(/setInterval/);
  expect(src).not.toMatch(/store\.canUndo[^.]/);
  expect(src).not.toMatch(/undoManager/);
  expect(src).not.toMatch(/@affine\/core/);
});

test('App layout: header + tree slot; debug bar gated; md pane not in tree slot', () => {
  const app = readFileSync(join(here, '../../App.tsx'), 'utf8');
  expect(app).toMatch(/VenusHeader/);
  expect(app).toMatch(/onTitleChange=\{onPageTitleChange\}/);
  expect(app).toMatch(/rename\(/);
  expect(app).toMatch(/CatalogTree/);
  expect(app).toMatch(/tree-host/);
  expect(app).toMatch(/openPageStore/);
  expect(app).toMatch(/listenCatalogHost/);
  expect(app).toMatch(/onOpenDoc=\{openDoc\}/);
  expect(app).not.toMatch(/onOpenDoc=\{\(id\) =>/);
  expect(app).toMatch(/VenusDebugBar/);
  expect(app).toMatch(/debugFromEnv/);
  expect(app).toMatch(/SHOW_DEBUG/);
  expect(app).not.toMatch(/store\.history/);
  expect(app).not.toMatch(/createOnDropHandler/);
});

test('git-log poll is not App state; debug bar is a sibling of the tree columns', () => {
  const app = readFileSync(join(here, '../../App.tsx'), 'utf8');
  const bar = readFileSync(join(here, 'VenusDebugBar.tsx'), 'utf8');
  const tree = readFileSync(join(here, '../catalog/CatalogTree.tsx'), 'utf8');
  expect(app).toMatch(/<VenusDebugBar sidecarUrl=\{SIDECAR_URL\} \/>/);
  expect(app).not.toMatch(/setGitLog/);
  expect(app).not.toMatch(/setInterval/);
  expect(app).not.toMatch(/parseGitLog/);
  expect(app).not.toMatch(/\/git\/log/);
  expect(app).toMatch(/onOpenDoc=\{openDoc\}/);
  expect(tree).toMatch(/memo\(function CatalogTree/);
  expect(bar).toMatch(/setInterval\(loadFlushStatus, 2000\)/);
  expect(bar).toMatch(/nextGitLogPoll/);
  expect(bar).toMatch(/sameGitLogShas/);
  expect(bar).toMatch(/venus-flush/);
  expect(bar).toMatch(/venus-git-log/);
  expect(bar).not.toMatch(/CatalogTree/);
  expect(app.indexOf('<VenusDebugBar')).toBeLessThan(app.indexOf('<CatalogTree'));
});

test('editor host files stay ignorant of chrome and catalog', () => {
  const host = join(here, '..');
  for (const name of ['mount-editor.js', 'editor-container.js', 'boot.js']) {
    const src = readFileSync(join(host, name), 'utf8');
    expect(src, name).not.toMatch(/VenusHeader/);
    expect(src, name).not.toMatch(/CatalogTree/);
    expect(src, name).not.toMatch(/catalog\//);
    expect(src, name).not.toMatch(/venus-header/);
    expect(src, name).not.toMatch(/store\.history/);
  }
});

test('title draft commits once on blur, Enter, or idle', () => {
  expect(TITLE_COMMIT_MS).toBe(300);
  const written: string[] = [];
  const pending: Array<() => void> = [];
  const commit = createTitleCommit((text) => {
    written.push(text);
    return text;
  }, {
    later(fn) {
      pending.push(fn);
      return pending.length;
    },
    cancel(id) {
      pending[Number(id) - 1] = () => {};
    },
  });
  commit.push('p');
  commit.push('pr');
  commit.push('pro');
  expect(written).toEqual([]);
  expect(pending).toHaveLength(3);
  commit.flush();
  expect(written).toEqual(['pro']);
  pending[2]();
  expect(written).toEqual(['pro']);

  commit.push('proto');
  commit.flush();
  commit.flush();
  expect(written).toEqual(['pro', 'proto']);

  commit.push('a');
  commit.push('ab');
  pending[pending.length - 1]();
  expect(written).toEqual(['pro', 'proto', 'ab']);
});

test('VITE_DEBUG / VITE_TESTIDS flags', () => {
  expect(debugFromEnv({})).toBe(false);
  expect(debugFromEnv({ VITE_DEBUG: '1' })).toBe(true);
  expect(testidsFromEnv({})).toBe(false);
  expect(testidsFromEnv({ VITE_TESTIDS: '1' })).toBe(true);
  expect(testidProps('venus-tree', {})).toEqual({});
  expect(testidProps('venus-tree', { VITE_TESTIDS: '1' })).toEqual({
    'data-testid': 'venus-tree',
  });
});
