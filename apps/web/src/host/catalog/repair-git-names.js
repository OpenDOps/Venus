import { PAGE_DOC_ID, PAGE_SQL_ID } from '../ids.js';
import { caseFoldName, gitNameFromStem, stemOfGitName } from './git-path.js';
import { childrenIndex, putNode } from './schema.js';

/**
 * First 8 characters of the published id. Home uses the SQL uuid, not `doc:home`.
 * Folders use the uuid after `folder:`.
 *
 * @param {{ id: string, kind: string, docId?: string }} node
 * @param {boolean} full
 */
function publishedId(node, full) {
  let body;
  if (node.id === PAGE_DOC_ID || node.docId === PAGE_DOC_ID) {
    body = PAGE_SQL_ID;
  } else if (node.kind === 'folder' && node.id.startsWith('folder:')) {
    body = node.id.slice('folder:'.length);
  } else {
    body = node.docId || node.id;
  }
  return full ? body : body.slice(0, 8);
}

/**
 * @param {string} stem
 * @param {{ id: string, kind: string, docId?: string }} node
 * @param {Set<string>} taken case-folded stems
 */
function allocateStem(stem, node, taken) {
  const short = publishedId(node, false);
  const full = publishedId(node, true);
  for (const tag of [short, full]) {
    const candidate = `${stem}-${tag}`;
    if (!taken.has(caseFoldName(candidate))) return candidate;
  }
  let n = 2;
  while (taken.has(caseFoldName(`${stem}-${full}-${n}`))) n += 1;
  return `${stem}-${full}-${n}`;
}

/**
 * After a merge, siblings can share a case-folded `gitName`. The lowest node
 * id keeps it. The others get `stem-<id8>`, the same suffix the sidecar
 * publishes, so the tree matches git. Display `name` is unchanged.
 * Idempotent: a clean tree is not transacted.
 *
 * @param {import('yjs').Doc} catalog
 * @returns {boolean} true when a gitName was rewritten
 */
export function repairDuplicateGitNames(catalog) {
  /** @type {{ id: string, gitName: string }[]} */
  const edits = [];
  for (const siblings of childrenIndex(catalog).values()) {
    /** @type {Map<string, NonNullable<ReturnType<typeof import('./schema.js').readNode>>[]>} */
    const groups = new Map();
    for (const node of siblings) {
      const stem = stemOfGitName(node.gitName, /** @type {'folder' | 'doc'} */ (node.kind));
      const key = caseFoldName(stem);
      if (!key) continue;
      const group = groups.get(key);
      if (group) group.push(node);
      else groups.set(key, [node]);
    }
    for (const group of groups.values()) {
      if (group.length < 2) continue;
      group.sort((a, b) => (a.id < b.id ? -1 : a.id > b.id ? 1 : 0));
      const taken = new Set(
        siblings.map((node) =>
          caseFoldName(
            stemOfGitName(node.gitName, /** @type {'folder' | 'doc'} */ (node.kind)),
          ),
        ),
      );
      for (const loser of group.slice(1)) {
        const stem = stemOfGitName(
          loser.gitName,
          /** @type {'folder' | 'doc'} */ (loser.kind),
        );
        const next = allocateStem(stem, loser, taken);
        taken.add(caseFoldName(next));
        edits.push({
          id: loser.id,
          gitName: gitNameFromStem(next, /** @type {'folder' | 'doc'} */ (loser.kind)),
        });
      }
    }
  }
  if (edits.length === 0) return false;
  catalog.transact(() => {
    for (const edit of edits) putNode(catalog, edit);
  });
  return true;
}
