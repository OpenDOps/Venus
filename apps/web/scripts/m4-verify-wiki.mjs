#!/usr/bin/env node
/**
 * M4 step-verify: does the committed wiki match the catalog it was flushed from?
 *
 * Clones `wiki/` HEAD, decodes the catalog Y.Doc from Postgres (snapshot +
 * trail), derives every `gitPath` with the host catalog code, and checks:
 *   files       clone `.md` set == catalog doc gitPaths (no missing, no stale)
 *   pages.yaml  pages + folders == catalog (uuid, docId, name)
 *   page_identity live rows == pages.yaml; no tombstone is in pages.yaml
 *   .venus/ids  one sidecar per catalog page, none for removed pages
 *   links       each `<!-- venus:doc:… -->` card: target in catalog, file in
 *               the clone, href relative to the source file, text = name
 *   links.json  outbound/inbound == the comments in the clone
 * Refuses to judge while the catalog or a catalog page has unflushed edits.
 *
 * From repo root, after Flush: pnpm wiki:verify
 * Env: WIKI_DIR (default wiki), VENUS_WIKI_CLONE (default /tmp/venus-m4-verify),
 *      VENUS_PSQL (default `docker compose exec -T postgres psql -U venus -d venus`).
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { dirname, join, posix, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as Y from 'yjs';
import { CATALOG_SQL_ID, PAGE_DOC_ID, PAGE_SQL_ID, WORKSPACE_ID } from '../src/host/ids.js';
import { KIND_DOC, KIND_FOLDER, listNodes } from '../src/host/catalog/schema.js';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '../../..');
const wiki = resolve(repoRoot, process.env.WIKI_DIR ?? 'wiki');
const dest = process.env.VENUS_WIKI_CLONE ?? '/tmp/venus-m4-verify';
const psqlCmd = (
  process.env.VENUS_PSQL ?? 'docker compose exec -T postgres psql -U venus -d venus'
).split(/\s+/);

const failures = [];
const fail = (area, msg) => failures.push(`${area}: ${msg}`);

function sql(query) {
  const [cmd, ...args] = psqlCmd;
  return execFileSync(cmd, [...args, '-tAc', query], {
    cwd: repoRoot,
    encoding: 'utf8',
  }).trim();
}

function rows(query) {
  const out = sql(query);
  return out ? out.split('\n').map((line) => line.split('|')) : [];
}

function git(...args) {
  return execFileSync('git', ['-C', wiki, ...args], { encoding: 'utf8' }).trim();
}

/** `pages:` / `folders:` maps of the sidecar's fixed YAML layout. */
function parsePagesYaml(text) {
  const out = { pages: new Map(), folders: new Map() };
  let section = null;
  let current = null;
  const unquote = (v) => {
    const s = v.trim();
    if (s.startsWith('"') && s.endsWith('"')) return JSON.parse(s);
    if (s.startsWith("'") && s.endsWith("'")) return s.slice(1, -1).replace(/''/g, "'");
    return s;
  };
  for (const line of text.split('\n')) {
    if (!line.trim() || line.trimStart().startsWith('#')) continue;
    const top = /^(\w+):\s*(\{\})?\s*$/.exec(line);
    if (top) {
      section = out[top[1]] ?? null;
      current = null;
      continue;
    }
    const key = /^ {2}(\S.*?):\s*$/.exec(line);
    if (key && section) {
      current = {};
      section.set(unquote(key[1]), current);
      continue;
    }
    const field = /^ {4}(\w+):\s*(.*)$/.exec(line);
    if (field && current) current[field[1]] = unquote(field[2]);
  }
  return out;
}

function listMarkdown(root, dir = '') {
  const out = [];
  for (const entry of readdirSync(join(root, dir), { withFileTypes: true })) {
    const rel = dir ? `${dir}/${entry.name}` : entry.name;
    if (entry.isDirectory()) {
      if (rel === '.git' || rel === '.venus' || rel === 'assets') continue;
      out.push(...listMarkdown(root, rel));
    } else if (entry.name.endsWith('.md')) {
      out.push(rel);
    }
  }
  return out;
}

const unescapeMd = (s) => s.replace(/\\([\\[\]()*_`#!<>|-])/g, '$1');
const sameSet = (a, b) => a.size === b.size && [...a].every((x) => b.has(x));
const sorted = (xs) => [...xs].sort();

// --- 0. wiki + freshness -----------------------------------------------------
if (!existsSync(join(wiki, '.git'))) {
  throw new Error(`${wiki} has no .git. Start the sidecar and Flush first.`);
}
const dirtyWiki = git('status', '--porcelain');
if (dirtyWiki) fail('wiki', `working tree has uncommitted changes:\n${dirtyWiki}`);
const head = git('rev-parse', 'HEAD');

// --- 1. catalog from Postgres -----------------------------------------------
const ws = WORKSPACE_ID;
const blobs = rows(
  `SELECT encode(bin, 'hex') FROM (
     SELECT bin, 0 AS k, 0::bigint AS seq FROM crdt_snapshot
       WHERE workspace_id = '${ws}' AND doc_id = '${CATALOG_SQL_ID}'
     UNION ALL
     SELECT bin, 1, seq FROM crdt_update
       WHERE workspace_id = '${ws}' AND doc_id = '${CATALOG_SQL_ID}'
   ) t ORDER BY k, seq`,
).map(([hex]) => Buffer.from(hex, 'hex'));
if (blobs.length === 0) throw new Error('no catalog rows in Postgres; is the hub DB up?');
const catalog = new Y.Doc();
for (const bin of blobs) Y.applyUpdate(catalog, new Uint8Array(bin));
const nodes = listNodes(catalog);
const docs = nodes.filter((n) => n.kind === KIND_DOC);
const folders = nodes.filter((n) => n.kind === KIND_FOLDER);
const docIdOf = (n) => n.docId ?? n.id;
const sqlIdOf = (docId) => (docId === PAGE_DOC_ID ? PAGE_SQL_ID : docId);
const byDocId = new Map(docs.map((n) => [docIdOf(n), n]));
const byPath = new Map(docs.map((n) => [n.gitPath, n]));

const catalogIds = [CATALOG_SQL_ID, ...docs.map((n) => sqlIdOf(docIdOf(n)))];
const pending = rows(
  `SELECT d.doc_id, d.clock, COALESCE(lf.clock, -1)
     FROM dirty d LEFT JOIN last_flushed lf USING (workspace_id, doc_id)
    WHERE d.workspace_id = '${ws}' AND d.clock > COALESCE(lf.clock, -1)
      AND d.doc_id IN (${catalogIds.map((id) => `'${id}'`).join(',')})`,
);
if (pending.length > 0) {
  console.error(
    `not flushed yet (doc_id, dirty clock, last flushed):\n${pending.map((r) => r.join(' ')).join('\n')}\nFlush, wait for it to finish, then rerun.`,
  );
  process.exit(2);
}

// --- 2. clone HEAD -----------------------------------------------------------
rmSync(dest, { recursive: true, force: true });
// Hardlinked objects become unreadable to the Compose sidecar (Docker Desktop bind mount).
execFileSync('git', ['clone', '--quiet', '--no-hardlinks', '--', wiki, dest]);
const read = (rel) => readFileSync(join(dest, rel), 'utf8');

// --- 3. files ----------------------------------------------------------------
const files = new Set(listMarkdown(dest));
const expected = new Set(byPath.keys());
for (const path of sorted(expected)) {
  if (!files.has(path)) {
    const n = byPath.get(path);
    const rowsForDoc = sql(
      `SELECT (SELECT count(*) FROM crdt_snapshot WHERE workspace_id='${ws}' AND doc_id='${sqlIdOf(docIdOf(n))}')
            + (SELECT count(*) FROM crdt_update WHERE workspace_id='${ws}' AND doc_id='${sqlIdOf(docIdOf(n))}')`,
    );
    const why = rowsForDoc === '0' ? ' (no page body on the hub: H2 legacy page)' : '';
    fail('files', `catalog page "${n.name}" ${path} is missing from the clone${why}`);
  }
}
for (const path of sorted(files)) {
  if (!expected.has(path)) fail('files', `${path} is in git but not in the catalog (stale file)`);
}
for (const path of files) {
  const bytes = readFileSync(join(dest, path));
  if (bytes.includes(0)) fail('files', `${path} contains NUL (not markdown)`);
}
const home = byDocId.get(PAGE_DOC_ID);
if (!home) fail('catalog', 'doc:home is missing from the catalog');

// --- 4. pages.yaml -----------------------------------------------------------
const yamlPath = '.venus/pages.yaml';
const yaml = existsSync(join(dest, yamlPath))
  ? parsePagesYaml(read(yamlPath))
  : (fail('pages.yaml', 'missing'), { pages: new Map(), folders: new Map() });
for (const [path, n] of byPath) {
  const entry = yaml.pages.get(path);
  if (!entry) {
    fail('pages.yaml', `no entry for ${path}`);
    continue;
  }
  const docId = docIdOf(n);
  if (entry.docId !== docId) fail('pages.yaml', `${path} docId ${entry.docId} != catalog ${docId}`);
  if (entry.uuid !== sqlIdOf(docId)) fail('pages.yaml', `${path} uuid ${entry.uuid} != ${sqlIdOf(docId)}`);
  if (entry.name !== n.name) fail('pages.yaml', `${path} name "${entry.name}" != catalog "${n.name}"`);
}
for (const path of yaml.pages.keys()) {
  if (!byPath.has(path)) fail('pages.yaml', `entry ${path} is not a catalog page`);
}
for (const f of folders) {
  const entry = yaml.folders.get(f.gitPath);
  if (!entry) fail('pages.yaml', `no folders: entry for ${f.gitPath} (${f.id})`);
  else if (entry.id !== f.id || entry.name !== f.name) {
    fail('pages.yaml', `folder ${f.gitPath} is ${entry.id}/${entry.name}, catalog ${f.id}/${f.name}`);
  }
}
const folderPaths = new Set(folders.map((f) => f.gitPath));
for (const path of yaml.folders.keys()) {
  if (!folderPaths.has(path)) fail('pages.yaml', `folders: entry ${path} is not a catalog folder`);
}

// --- 5. page_identity --------------------------------------------------------
const identityRows = rows(
  `SELECT uuid, doc_id, name, git_path, deleted_at IS NOT NULL FROM page_identity WHERE workspace_id = '${ws}'`,
);
const identity = new Map(
  identityRows
    .filter((r) => r[4] !== 't')
    .map(([uuid, docId, name, path]) => [uuid, { docId, name, path }]),
);
const tombstones = identityRows.filter((r) => r[4] === 't').map(([uuid]) => uuid);
for (const [path, entry] of yaml.pages) {
  const row = identity.get(entry.uuid);
  if (!row) fail('page_identity', `no row for ${entry.uuid} (${path})`);
  else if (row.path !== path || row.docId !== entry.docId || row.name !== entry.name) {
    fail('page_identity', `${entry.uuid} is ${row.path}/${row.docId}/${row.name}, yaml ${path}/${entry.docId}/${entry.name}`);
  }
}
const yamlUuids = new Set([...yaml.pages.values()].map((e) => e.uuid));
for (const [uuid, row] of identity) {
  if (!yamlUuids.has(uuid)) fail('page_identity', `row ${uuid} ${row.path} is not in pages.yaml`);
}
for (const uuid of tombstones) {
  if (yamlUuids.has(uuid)) fail('page_identity', `${uuid} is in pages.yaml but tombstoned`);
}

// --- 6. .venus/ids -----------------------------------------------------------
const idsDir = join(dest, '.venus/ids');
const idFiles = new Set(
  existsSync(idsDir) ? readdirSync(idsDir).filter((f) => f.endsWith('.json')) : [],
);
for (const n of docs) {
  const docId = docIdOf(n);
  if (!files.has(n.gitPath)) continue;
  const name = `${docId}.json`;
  if (!idFiles.has(name)) {
    fail('ids', `.venus/ids/${name} missing for ${n.gitPath}`);
    continue;
  }
  const json = JSON.parse(read(`.venus/ids/${name}`));
  if (json.docId !== docId) fail('ids', `.venus/ids/${name} docId ${json.docId}`);
}
for (const f of idFiles) {
  if (!byDocId.has(f.slice(0, -'.json'.length))) fail('ids', `.venus/ids/${f} is for a page not in the catalog`);
}

// --- 7. linked-doc cards -----------------------------------------------------
/** @type {Map<string, Set<string>>} source docId → target docIds */
const outbound = new Map();
let cards = 0;
for (const path of files) {
  const src = byPath.get(path);
  if (!src) continue;
  const lines = read(path).split('\n');
  const targets = new Set();
  lines.forEach((line, i) => {
    const m = /<!--\s*venus:doc:(\S+?)( missing)?\s*-->/.exec(line);
    if (!m) return;
    cards += 1;
    const targetId = m[1];
    targets.add(targetId);
    const where = `${path}:${i + 1}`;
    const target = byDocId.get(targetId);
    const prev = lines[i - 1] ?? '';
    if (/\.\/workspace\//.test(prev)) fail('links', `${where} uses a ./workspace/ URL`);
    if (m[2]) {
      if (target) fail('links', `${where} marks ${targetId} missing, but "${target.name}" is in the catalog`);
      if (!/~~.*~~\s*$/.test(prev)) fail('links', `${where} missing card has no ~~name~~ above`);
      return;
    }
    const link = /\[((?:\\.|[^\]\\])*)\]\(([^)]*)\)\s*$/.exec(prev);
    if (!target) {
      fail('links', `${where} points at ${targetId}, which is not in the catalog`);
      return;
    }
    if (!link) {
      fail('links', `${where} card for "${target.name}" has no [name](path) on the line above`);
      return;
    }
    const want = posix.relative(posix.dirname(path), target.gitPath);
    const href = link[2].split('/').map(decodeURIComponent).join('/');
    if (href !== want) fail('links', `${where} href ${href} != ${want} (catalog gitPath ${target.gitPath})`);
    if (!files.has(posix.normalize(posix.join(posix.dirname(path), href)))) {
      fail('links', `${where} href ${href} does not resolve to a file in the clone`);
    }
    if (unescapeMd(link[1]) !== target.name) {
      fail('links', `${where} link text "${unescapeMd(link[1])}" != catalog name "${target.name}"`);
    }
  });
  if (targets.size > 0) outbound.set(docIdOf(src), targets);
}

// --- 8. links.json -----------------------------------------------------------
const linksPath = '.venus/links.json';
if (!existsSync(join(dest, linksPath))) {
  if (cards > 0) fail('links.json', 'missing while pages have linked-doc cards');
} else {
  const index = JSON.parse(read(linksPath));
  const out = new Map(Object.entries(index.outbound ?? {}).map(([k, v]) => [k, new Set(v)]));
  for (const [srcId, targets] of outbound) {
    const got = out.get(srcId) ?? new Set();
    if (!sameSet(got, targets)) {
      fail('links.json', `outbound[${srcId}] ${JSON.stringify(sorted(got))} != markdown ${JSON.stringify(sorted(targets))}`);
    }
  }
  for (const [srcId, got] of out) {
    if (got.size > 0 && !outbound.has(srcId)) {
      fail('links.json', `outbound[${srcId}] ${JSON.stringify(sorted(got))} but the markdown has no cards`);
    }
  }
  const inbound = new Map();
  for (const [srcId, targets] of outbound) {
    for (const t of targets) {
      if (!inbound.has(t)) inbound.set(t, new Set());
      inbound.get(t).add(srcId);
    }
  }
  const inn = new Map(Object.entries(index.inbound ?? {}).map(([k, v]) => [k, new Set(v)]));
  for (const id of new Set([...inbound.keys(), ...inn.keys()])) {
    const want = inbound.get(id) ?? new Set();
    const got = inn.get(id) ?? new Set();
    if (!sameSet(want, got)) {
      fail('links.json', `inbound[${id}] ${JSON.stringify(sorted(got))} != markdown ${JSON.stringify(sorted(want))}`);
    }
  }
}

// --- report ------------------------------------------------------------------
console.log(`wiki HEAD   ${head}`);
console.log(`clone       ${dest}`);
console.log(`catalog     ${docs.length} pages, ${folders.length} folders (Postgres ${blobs.length} rows)`);
console.log(`clone       ${files.size} .md files, ${cards} linked-doc cards`);
console.log(`yaml / db   ${yaml.pages.size} pages, ${yaml.folders.size} folders / ${identity.size} live page_identity rows, ${tombstones.length} tombstoned`);
if (failures.length > 0) {
  console.error(`\nFAIL (${failures.length})`);
  for (const f of failures) console.error(`  - ${f}`);
  process.exit(1);
}
console.log('\nok: git tree, pages.yaml, page_identity, .venus/ids and links match the catalog');
