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

export const COLLABORATION_PATH = `/collaboration/${WORKSPACE_ID}`;
