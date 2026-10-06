export const CATALOG_GIT_LOG_PATH = 'spec/home.md';

/**
 * @typedef {{ subject: string, sha: string }} GitLogEntry
 */

/**
 * @param {unknown} data
 * @returns {GitLogEntry[]}
 */
export function parseGitLog(data) {
  if (!Array.isArray(data)) return [];
  /** @type {GitLogEntry[]} */
  const out = [];
  for (const row of data) {
    if (!row || typeof row !== 'object') continue;
    const subject = /** @type {{ subject?: unknown }} */ (row).subject;
    const sha = /** @type {{ sha?: unknown }} */ (row).sha;
    if (typeof subject !== 'string' || typeof sha !== 'string') continue;
    out.push({ subject, sha });
  }
  return out;
}

/**
 * Same commit list (sha order). Subjects may differ; poll skips `setState`.
 *
 * @param {GitLogEntry[]} a
 * @param {GitLogEntry[]} b
 */
export function sameGitLogShas(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i]?.sha !== b[i]?.sha) return false;
  }
  return true;
}
