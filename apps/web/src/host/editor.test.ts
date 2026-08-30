import { readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';
import pkg from '../../package.json' with { type: 'json' };

const hostDir = dirname(fileURLToPath(import.meta.url));

test('no @affine/core in @venus/web', () => {
  const deps: Record<string, string | undefined> = {
    ...pkg.dependencies,
    ...pkg.devDependencies,
  };
  expect(deps['@affine/core']).toBeUndefined();
  expect(deps['@blocksuite/integration-test']).toBeUndefined();
});

test('host sources do not import AFFiNE app shell', () => {
  const files: string[] = [];
  const walk = (dir: string) => {
    for (const ent of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, ent.name);
      if (ent.isDirectory()) walk(p);
      else if (/\.(js|ts|tsx)$/.test(ent.name)) files.push(p);
    }
  };
  walk(hostDir);
  expect(files.length).toBeGreaterThan(0);
  const importOf = (pkg: string) =>
    new RegExp(`(?:from|import)\\s+['"]${pkg}(?:/[^'"]*)?['"]`);
  for (const path of files) {
    const src = readFileSync(path, 'utf8');
    expect(src, path).not.toMatch(importOf('@affine/core'));
    expect(src, path).not.toMatch(importOf('@blocksuite/integration-test'));
  }
});
