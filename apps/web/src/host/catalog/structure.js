/**
 * Parent used for git paths and for the structure repair.
 * An orphan (missing parent) and the greatest id in a cycle are roots.
 * `links` maps every live node id to its stored parent id.
 *
 * @param {string} id
 * @param {Map<string, string | null>} links
 * @returns {string | null}
 */
export function repairedParent(id, links) {
  if (!links.has(id)) return null;
  const parent = links.get(id) ?? null;
  if (parent == null) return null;
  if (!links.has(parent)) return null;
  if (isCycleBreak(id, links)) return null;
  return parent;
}

/**
 * @param {string} id
 * @param {Map<string, string | null>} links
 */
function isCycleBreak(id, links) {
  const members = cycleMembers(id, links);
  if (!members || !members.includes(id)) return false;
  let max = members[0];
  for (const member of members) {
    if (member > max) max = member;
  }
  return id === max;
}

/**
 * The cycle reached by following parents from `id`, including `id` when
 * `id` is on that cycle.
 *
 * @param {string} id
 * @param {Map<string, string | null>} links
 * @returns {string[] | null}
 */
function cycleMembers(id, links) {
  /** @type {Map<string, number>} */
  const seen = new Map();
  /** @type {string[]} */
  const chain = [];
  let cur = id;
  while (cur) {
    if (!links.has(cur)) return null;
    const start = seen.get(cur);
    if (start != null) return chain.slice(start);
    seen.set(cur, chain.length);
    chain.push(cur);
    const parent = links.get(cur) ?? null;
    if (parent == null || !links.has(parent)) return null;
    cur = parent;
  }
  return null;
}
