#!/usr/bin/env node
/**
 * M3 step-verify: `git clone` the nested wiki and read spec/home.md.
 * No hub / Postgres required to read the clone.
 *
 * From repo root (after a Flush): pnpm wiki:clone
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const wiki = process.env.WIKI_DIR
  ? join(process.cwd(), process.env.WIKI_DIR)
  : join(repoRoot, 'wiki');
const dest = process.env.VENUS_WIKI_CLONE ?? '/tmp/venus-wiki-clone';

if (!existsSync(join(wiki, '.git'))) {
  throw new Error(
    `${wiki} has no .git. Start sidecar, type in the editor, wait ~2s, Flush, then retry.`,
  );
}

rmSync(dest, { recursive: true, force: true });
execFileSync('git', ['clone', '--', wiki, dest], { stdio: 'inherit' });

const home = join(dest, 'spec/home.md');
const bytes = readFileSync(home);
if (bytes.includes(0)) {
  throw new Error(`${home} contains NUL; clone must be markdown, not Yjs`);
}
const md = bytes.toString('utf8');
if (!md.startsWith('#')) {
  throw new Error(`${home} does not start with # (not markdown source)`);
}
if (!md.includes('Why Venus')) {
  throw new Error(`${home} missing seed H1 Why Venus`);
}
if (!md.includes('Empty host')) {
  throw new Error(`${home} missing seed H2 Empty host`);
}

console.log(`ok: ${home} (${bytes.length} bytes)`);
console.log(md.slice(0, 240));
