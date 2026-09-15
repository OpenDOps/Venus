/**
 * M0 wiki id. UUID v5 (DNS) of `venus-m0`. Must match
 * `venus_hub::DEFAULT_WORKSPACE_ID`.
 */
export const WORKSPACE_ID = '77e4a2b1-8b40-5979-a73c-fd4477216d00';

/**
 * BlockSuite page guid / sidecar `docId`. SQL `doc_id` is a different
 * UUID (`venus_hub::PAGE_DOC_ID`, v5 of `doc:home`).
 */
export const PAGE_DOC_ID = 'doc:home';

/**
 * Hub SQL `doc_id` for `doc:home`. Wire A `?doc=` uses this alphabet, not the
 * guid. Omit `?doc=` for home (same bind).
 */
export const PAGE_SQL_ID = '395cd07b-bdb1-5f54-ada8-e9a3fabb6a20';

/**
 * Catalog Y.Doc guid. Plain Y.Doc, not `affine:page`. Hub SQL uuid is
 * `CATALOG_SQL_ID` (v5 DNS of this string).
 */
export const CATALOG_GUID = 'venus:catalog';

/** Hub SQL `doc_id` for `venus:catalog`. Must match `venus_hub::CATALOG_DOC_ID`. */
export const CATALOG_SQL_ID = '4fe5c16e-4be3-5700-a456-ecc8e86cdf1a';

export const COLLABORATION_PATH = `/collaboration/${WORKSPACE_ID}`;

const SQL_UUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/**
 * Map a BlockSuite guid or SQL uuid to the hub query value (A1).
 * Unknown guids throw — do not fall through to home.
 *
 * @param {string} guidOrSql
 * @returns {string}
 */
export function sqlIdForDoc(guidOrSql) {
  if (guidOrSql === PAGE_DOC_ID) return PAGE_SQL_ID;
  if (guidOrSql === CATALOG_GUID) return CATALOG_SQL_ID;
  if (typeof guidOrSql === 'string' && SQL_UUID.test(guidOrSql)) {
    return guidOrSql.toLowerCase();
  }
  throw new Error(
    `sqlIdForDoc: expected ${PAGE_DOC_ID}, ${CATALOG_GUID}, or a SQL uuid, got ${String(guidOrSql)}`,
  );
}

/**
 * Socket URL for wire A. Bare path = home (B). Other docs `?doc=<sql uuid>`.
 *
 * @param {string} base collaboration WS URL (no `doc` query)
 * @param {string} guidOrSql
 */
export function collaborationSocketUrl(base, guidOrSql) {
  const path = String(base).split('?')[0];
  const sql = sqlIdForDoc(guidOrSql);
  if (sql === PAGE_SQL_ID) return path;
  return `${path}?doc=${sql}`;
}
