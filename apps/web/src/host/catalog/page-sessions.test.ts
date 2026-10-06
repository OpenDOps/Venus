import { expect, test } from 'vitest';
import type { SyncProvider } from '../sync-provider.js';
import { CATALOG_GUID, PAGE_DOC_ID } from '../ids.js';
import { KEEP_PAGE_SESSIONS, PageSessions } from './page-sessions.js';

function fakeProvider() {
  const disconnected: string[] = [];
  const provider: SyncProvider = {
    kind: 'memory',
    synced: true,
    connect() {},
    disconnect(id) {
      disconnected.push(id);
    },
    whenReady: () => Promise.resolve(),
  };
  return { provider, disconnected };
}

test('keeps the open page and the one before it; the third open drops the oldest', () => {
  expect(KEEP_PAGE_SESSIONS).toBe(2);
  const { provider, disconnected } = fakeProvider();
  const pages = new PageSessions(provider);
  pages.touch('a');
  pages.touch('b');
  expect(pages.ids()).toEqual(['a', 'b']);
  expect(disconnected).toEqual([]);
  pages.touch('c');
  expect(pages.ids()).toEqual(['b', 'c']);
  expect(disconnected).toEqual(['a']);
});

test('revisiting a kept page makes it most recent and does not disconnect it', () => {
  const { provider, disconnected } = fakeProvider();
  const pages = new PageSessions(provider);
  pages.touch('a');
  pages.touch('b');
  pages.touch('a');
  expect(pages.ids()).toEqual(['b', 'a']);
  pages.touch('c');
  expect(pages.ids()).toEqual(['a', 'c']);
  expect(disconnected).toEqual(['b']);
});

test('home and the catalog are never kept or disconnected here', () => {
  const { provider, disconnected } = fakeProvider();
  const pages = new PageSessions(provider, { keep: 1 });
  pages.touch(PAGE_DOC_ID);
  pages.touch(CATALOG_GUID);
  pages.touch('a');
  expect(pages.ids()).toEqual(['a']);
  expect(pages.has(PAGE_DOC_ID)).toBe(false);
  expect(disconnected).toEqual([]);
});

test('release disconnects one kept page; unknown ids are a no-op; dispose drops the rest', () => {
  const { provider, disconnected } = fakeProvider();
  const pages = new PageSessions(provider);
  pages.touch('a');
  pages.touch('b');
  pages.release('missing');
  expect(disconnected).toEqual([]);
  pages.release('a');
  expect(pages.ids()).toEqual(['b']);
  expect(disconnected).toEqual(['a']);
  pages.dispose();
  expect(pages.ids()).toEqual([]);
  expect(disconnected).toEqual(['a', 'b']);
});
