/**
 * POSIX filename / gitPath helpers for catalog nodes.
 * Docname (`name`) stays UTF; filename is the git stem.
 */

/**
 * Strip controls / unprintables from a docname. Keeps `/` `\`.
 *
 * @param {unknown} name
 * @returns {string}
 */
export function sanitizeDocname(name) {
  return String(name ?? '')
    .replace(/\p{Cc}/gu, '')
    .replace(/\p{Cs}/gu, '')
    .trim();
}

/**
 * @param {string} gitPath
 * @param {'folder' | 'doc'} kind
 */
export function filenameOfGitPath(gitPath, kind) {
  const leaf = String(gitPath ?? '').split('/').pop() ?? '';
  if (kind === 'doc') return leaf.replace(/\.md$/i, '');
  return leaf;
}

/**
 * POSIX leaf stored on the node (`gitName`). Docs include `.md`.
 *
 * @param {string} stem
 * @param {'folder' | 'doc'} kind
 */
export function gitNameFromStem(stem, kind) {
  const s = String(stem ?? '');
  return kind === 'doc' ? `${s}.md` : s;
}

/**
 * Stem used for sibling uniqueness (docs drop `.md`).
 *
 * @param {string} gitName
 * @param {'folder' | 'doc'} kind
 */
export function stemOfGitName(gitName, kind) {
  const leaf = String(gitName ?? '');
  if (kind === 'doc') return leaf.replace(/\.md$/i, '');
  return leaf;
}

/**
 * Join parent folder gitPath + filename. Docs always end in `.md`.
 *
 * @param {string | null | undefined} parentGitPath
 * @param {string} filename
 * @param {'folder' | 'doc'} kind
 */
export function joinGitPath(parentGitPath, filename, kind) {
  const leaf = gitNameFromStem(filename, kind);
  const parent = String(parentGitPath ?? '').replace(/\/+$/, '');
  return parent ? `${parent}/${leaf}` : leaf;
}

/**
 * Join ancestor `gitName`s. Leaves already include `.md` for docs.
 *
 * @param {string | null | undefined} parentGitPath
 * @param {string} gitName
 */
export function joinGitNames(parentGitPath, gitName) {
  const parent = String(parentGitPath ?? '').replace(/\/+$/, '');
  const leaf = String(gitName ?? '');
  return parent ? `${parent}/${leaf}` : leaf;
}

/**
 * Derive a unique POSIX filename from a docname.
 *
 * @param {unknown} name
 * @param {{ fallback: string, taken?: Iterable<string> }} options
 */
export function filenameFromDocname(name, options) {
  const fallback = options.fallback;
  const taken = new Set(options.taken ?? []);
  let stem = sanitizeDocname(name).replace(/[/\\]/g, '_');
  stem = stem.replace(/_+/g, '_');
  stem = stem.replace(/^[._\s]+|[._\s]+$/g, '');
  if (!stem || stem === '.' || stem === '..' || /\.md$/i.test(stem)) {
    stem = fallback;
  }
  if (!taken.has(stem)) return stem;
  let n = 1;
  while (taken.has(`${stem}_${n}`)) n += 1;
  return `${stem}_${n}`;
}
