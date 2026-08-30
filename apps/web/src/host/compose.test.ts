import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test } from 'vitest';

const hostDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(hostDir, '../../../..');
const composeFile = join(repoRoot, 'docker-compose.yml');

function serviceNames(src: string): string[] {
  const names: string[] = [];
  let inServices = false;
  for (const line of src.split('\n')) {
    if (/^services:\s*$/.test(line)) {
      inServices = true;
      continue;
    }
    if (inServices && /^[A-Za-z]/.test(line)) break;
    if (!inServices) continue;
    const m = line.match(/^  ([a-zA-Z0-9_-]+):\s*$/);
    if (m) names.push(m[1]);
  }
  return names;
}

test('Three services: postgres, octobase, and web are separate Compose services', () => {
  const src = readFileSync(composeFile, 'utf8');
  const names = serviceNames(src);
  expect(names, 'docker-compose.yml service keys').toEqual([
    'postgres',
    'octobase',
    'web',
  ]);
  expect(src).toMatch(/image:\s*postgres:16/);
  expect(src).toMatch(/pg-data:/);
  expect(src).toMatch(/deploy\/octobase/);
  expect(src).toMatch(/deploy\/web\/Dockerfile/);
  expect(src).not.toMatch(/^\s*USE_MEMORY_SQLITE:/m);
  expect(src).toMatch(/DATABASE_URL:\s*postgres:\/\//);
  expect(new Set(names).size).toBe(3);
});

test('docker compose config --services lists postgres octobase web', () => {
  let out: string;
  try {
    out = execFileSync('docker', ['compose', 'config', '--services'], {
      cwd: repoRoot,
      encoding: 'utf8',
      timeout: 15_000,
    });
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    if (
      /enoent|not found|cannot connect|permission denied|daemon/i.test(msg)
    ) {
      console.warn(
        `compose.test.ts: skipping docker compose config (${msg}). File parse still ran.`,
      );
      return;
    }
    throw err;
  }
  const names = out
    .trim()
    .split('\n')
    .map((s) => s.trim())
    .filter(Boolean)
    .sort();
  expect(names).toEqual(['octobase', 'postgres', 'web'].sort());
});
