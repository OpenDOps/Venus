import { expect, test } from 'vitest';
import { createM0Workspace } from './workspace.js';
import pkg from '../../package.json' with { type: 'json' };

test('single page default tree', () => {
  const { workspace, store, docId } = createM0Workspace();
  expect(docId).toBe('doc:home');
  expect(workspace.docs.size).toBe(1);

  const root = store.root;
  expect(root?.flavour).toBe('affine:page');
  expect(root?.children.map((c) => c.flavour).sort()).toEqual([
    'affine:note',
    'affine:surface',
  ]);
  const note = root?.children.find((c) => c.flavour === 'affine:note');
  expect(note?.children[0]?.flavour).toBe('affine:paragraph');
  expect(note?.children.some((c) => c.flavour === 'affine:paragraph')).toBe(
    true,
  );
  expect(store.canUndo).toBe(false);
});

test('no sync socket on create', () => {
  const Ws = globalThis.WebSocket;
  const constructed: unknown[] = [];
  // @ts-expect-error stub
  globalThis.WebSocket = class {
    constructor(...args: unknown[]) {
      constructed.push(args);
      throw new Error('WebSocket must not open in M0');
    }
  };

  try {
    createM0Workspace();
    expect(constructed).toEqual([]);
  } finally {
    globalThis.WebSocket = Ws;
  }

  const deps: Record<string, string | undefined> = {
    ...pkg.dependencies,
    ...pkg.devDependencies,
  };
  for (const name of [
    'y-websocket',
    'y-indexeddb',
    'hocuspocus',
    '@affine/core',
    '@affine/graphql',
    '@blocksuite/integration-test',
  ]) {
    expect(deps[name]).toBeUndefined();
  }
});
