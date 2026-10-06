import { readdirSync, readFileSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

/** pageId in `<!-- venus:doc:… -->` — same allowlist as convert comments. */
const SAFE_PAGE_ID = /^[A-Za-z0-9_.:-]+$/;
const COMMENT_RE = /<!--\s*venus:doc:([A-Za-z0-9_.:-]+)(?:\s+missing)?\s*-->/g;

/**
 * @typedef {{ inbound: Record<string, string[]>, outbound: Record<string, string[]> }} LinkIndex
 */

export function emptyIndex() {
  return { inbound: {}, outbound: {} };
}

function isSafePageId(pageId) {
  return (
    typeof pageId === 'string' &&
    pageId.length > 0 &&
    pageId.length <= 128 &&
    SAFE_PAGE_ID.test(pageId)
  );
}

function uniqueSorted(ids) {
  return [...new Set(ids.filter(isSafePageId))].sort();
}

/**
 * Targets named by `<!-- venus:doc:<id> -->` only. Ordinary markdown URLs
 * without that comment are ignored.
 *
 * @param {string} markdown
 * @returns {string[]}
 */
export function targetsInMarkdown(markdown) {
  if (typeof markdown !== 'string' || markdown.length === 0) return [];
  const found = [];
  COMMENT_RE.lastIndex = 0;
  let m;
  while ((m = COMMENT_RE.exec(markdown))) {
    found.push(m[1]);
  }
  return uniqueSorted(found);
}

/**
 * O(1) map lookup. Does not read the wiki.
 *
 * @param {LinkIndex} index
 * @param {string} targetDocId
 * @returns {string[]}
 */
export function inbound(index, targetDocId) {
  return index?.inbound?.[targetDocId] ?? [];
}

/**
 * O(1) map lookup. Does not read the wiki.
 *
 * @param {LinkIndex} index
 * @param {string} sourceDocId
 * @returns {string[]}
 */
export function outbound(index, sourceDocId) {
  return index?.outbound?.[sourceDocId] ?? [];
}

function removeSourceFromInbound(index, sourceId) {
  for (const [target, sources] of Object.entries(index.inbound)) {
    const next = sources.filter((id) => id !== sourceId);
    if (next.length === 0) delete index.inbound[target];
    else index.inbound[target] = next;
  }
}

function addInbound(index, targetId, sourceId) {
  const cur = index.inbound[targetId] ?? [];
  if (cur.includes(sourceId)) return;
  index.inbound[targetId] = uniqueSorted([...cur, sourceId]);
}

/**
 * Replace this source’s outbound row and rebuild inbound for those targets
 * (drop stale edges from this source).
 *
 * @param {LinkIndex} index
 * @param {string} sourceId
 * @param {string[]} targets
 * @returns {LinkIndex}
 */
export function upsertOutbound(index, sourceId, targets) {
  if (!isSafePageId(sourceId)) return index;
  removeSourceFromInbound(index, sourceId);
  const next = uniqueSorted(targets).filter((id) => id !== sourceId);
  if (next.length === 0) delete index.outbound[sourceId];
  else index.outbound[sourceId] = next;
  for (const target of next) addInbound(index, target, sourceId);
  return index;
}

/**
 * Drop an id from both maps (leaf delete / git rm).
 *
 * @param {LinkIndex} index
 * @param {string} docId
 * @returns {LinkIndex}
 */
export function dropId(index, docId) {
  delete index.outbound[docId];
  delete index.inbound[docId];
  removeSourceFromInbound(index, docId);
  for (const [source, targets] of Object.entries(index.outbound)) {
    const next = targets.filter((id) => id !== docId);
    if (next.length === 0) delete index.outbound[source];
    else index.outbound[source] = next;
  }
  return index;
}

/**
 * @param {Array<{ id: string, markdown: string }>} pages
 * @returns {LinkIndex}
 */
export function buildFromPages(pages) {
  const index = emptyIndex();
  for (const page of pages ?? []) {
    upsertOutbound(index, page.id, targetsInMarkdown(page.markdown));
  }
  return index;
}

function sortRecord(record) {
  const out = {};
  for (const key of Object.keys(record).sort()) {
    const ids = uniqueSorted(record[key] ?? []);
    if (ids.length > 0) out[key] = ids;
  }
  return out;
}

/**
 * @param {LinkIndex} index
 * @returns {string}
 */
export function serialize(index) {
  const inboundMap = sortRecord(index?.inbound ?? {});
  const outboundMap = sortRecord(index?.outbound ?? {});
  return `${JSON.stringify({ inbound: inboundMap, outbound: outboundMap }, null, 2)}\n`;
}

/**
 * @param {string} json
 * @returns {LinkIndex | null}
 */
export function parse(json) {
  if (typeof json !== 'string') return null;
  let raw;
  try {
    raw = JSON.parse(json);
  } catch {
    return null;
  }
  if (!raw || typeof raw !== 'object' || Array.isArray(raw)) return null;
  if (raw.inbound != null && (typeof raw.inbound !== 'object' || Array.isArray(raw.inbound))) {
    return null;
  }
  if (raw.outbound != null && (typeof raw.outbound !== 'object' || Array.isArray(raw.outbound))) {
    return null;
  }
  const index = emptyIndex();
  for (const [target, sources] of Object.entries(raw.inbound ?? {})) {
    if (!Array.isArray(sources) || !isSafePageId(target)) continue;
    index.inbound[target] = uniqueSorted(sources);
    if (index.inbound[target].length === 0) delete index.inbound[target];
  }
  for (const [source, targets] of Object.entries(raw.outbound ?? {})) {
    if (!Array.isArray(targets) || !isSafePageId(source)) continue;
    upsertOutbound(index, source, targets);
  }
  return index;
}

/**
 * `pages.yaml` path → docId (pages: only).
 *
 * @param {string} yaml
 * @returns {Record<string, string>}
 */
export function pathToDocIdFromPagesYaml(yaml) {
  const map = {};
  if (typeof yaml !== 'string') return map;
  let inPages = false;
  let path = null;
  for (const line of yaml.split('\n')) {
    if (/^pages:\s*$/.test(line)) {
      inPages = true;
      path = null;
      continue;
    }
    if (/^[A-Za-z]/.test(line) && !line.startsWith(' ')) {
      inPages = line.startsWith('pages:');
      path = null;
      continue;
    }
    if (!inPages) continue;
    const key = line.match(/^\s{2}([^:\s][^:]*):\s*$/);
    if (key) {
      path = key[1].replace(/^["']|["']$/g, '');
      continue;
    }
    const docId = line.match(/^\s+docId:\s*(.+?)\s*$/);
    if (docId && path) {
      let id = docId[1].trim();
      if (
        (id.startsWith('"') && id.endsWith('"')) ||
        (id.startsWith("'") && id.endsWith("'"))
      ) {
        id = id.slice(1, -1);
      }
      if (isSafePageId(id)) map[path] = id;
    }
  }
  return map;
}

function posixRel(root, file) {
  return relative(root, file).split(sep).join('/');
}

function listMarkdown(dir, root, out) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return;
  }
  for (const ent of entries) {
    if (ent.name === '.git' || ent.name === '.venus') continue;
    const full = join(dir, ent.name);
    if (ent.isDirectory()) {
      listMarkdown(full, root, out);
      continue;
    }
    if (ent.isFile() && ent.name.endsWith('.md')) out.push(full);
  }
}

/**
 * Rebuild from HEAD markdown + `pages.yaml` path→docId. Does not read pin bytes.
 *
 * @param {string} wikiDir
 * @returns {LinkIndex}
 */
export function rebuildFromWiki(wikiDir) {
  const yamlPath = join(wikiDir, '.venus', 'pages.yaml');
  let yaml = '';
  try {
    yaml = readFileSync(yamlPath, 'utf8');
  } catch {
    yaml = '';
  }
  const pathToDoc = pathToDocIdFromPagesYaml(yaml);
  const files = [];
  listMarkdown(wikiDir, wikiDir, files);
  const pages = [];
  for (const file of files) {
    const rel = posixRel(wikiDir, file);
    const id = pathToDoc[rel];
    if (!id) continue;
    pages.push({ id, markdown: readFileSync(file, 'utf8') });
  }
  return buildFromPages(pages);
}

export function linksJsonRel() {
  return join('.venus', 'links.json');
}
