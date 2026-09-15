import { describe, expect, test } from 'vitest';
import { COLLABORATION_PATH, CATALOG_GUID, PAGE_DOC_ID } from '../ids.js';
import { VenusHubProvider } from '../providers/venus-hub-provider.js';
import { createM0Workspace } from '../workspace.js';
import { openCatalog } from './open.js';
import { createDoc, createFolder, CatalogError, deleteNode, reparent } from './ops.js';
import { FOLDER_SPEC_ID, getNode } from './schema.js';

async function hubRootBody(): Promise<string | null> {
  try {
    const res = await fetch('http://127.0.0.1:3000/', {
      signal: AbortSignal.timeout(1500),
    });
    return (await res.text()).trim();
  } catch {
    return null;
  }
}

const hubBody = await hubRootBody();
const hubUp = hubBody === 'venus-hub';

if (hubBody && hubBody !== 'venus-hub') {
  throw new Error(
    `:3000 is not the Venus hub (GET / → ${JSON.stringify(hubBody)}). Fail if keck is the process.`,
  );
}

if (!hubUp) {
  console.warn(
    'catalog-sync.test.ts: skipping A→B — nothing on 127.0.0.1:3000. Start with pnpm sync:up.',
  );
}

describe.skipIf(!hubUp)('A→B catalog over hub WS', () => {
  test('createDoc on A appears on B without reload', async () => {
    const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
    const pa = new VenusHubProvider(url);
    const pb = new VenusHubProvider(url);
    const a = await createM0Workspace(pa);
    const b = await createM0Workspace(pb);
    const ca = await openCatalog(pa, a.workspace);
    const cb = await openCatalog(pb, b.workspace);
    try {
      expect(getNode(cb.catalog, FOLDER_SPEC_ID)?.id).toBe(FOLDER_SPEC_ID);
      expect(getNode(cb.catalog, PAGE_DOC_ID)?.gitPath).toMatch(/home\.md$/);

      const created = createDoc(ca.catalog, a.workspace, {
        createAt: FOLDER_SPEC_ID,
      });
      await expect
        .poll(() => getNode(cb.catalog, created.id)?.gitPath, {
          timeout: 10_000,
        })
        .toBe(created.gitPath);

      deleteNode(ca.catalog, created.id);
      await expect
        .poll(() => getNode(cb.catalog, created.id), { timeout: 10_000 })
        .toBeNull();
    } finally {
      pa.disconnect(CATALOG_GUID);
      pa.disconnect(a.docId);
      pb.disconnect(CATALOG_GUID);
      pb.disconnect(b.docId);
    }
  });

  test('home_protected on A leaves home under spec on B', async () => {
    const url = `ws://127.0.0.1:3000${COLLABORATION_PATH}`;
    const pa = new VenusHubProvider(url);
    const pb = new VenusHubProvider(url);
    const a = await createM0Workspace(pa);
    const b = await createM0Workspace(pb);
    const ca = await openCatalog(pa, a.workspace);
    const cb = await openCatalog(pb, b.workspace);
    try {
      expect(getNode(cb.catalog, FOLDER_SPEC_ID)?.id).toBe(FOLDER_SPEC_ID);
      expect(getNode(cb.catalog, PAGE_DOC_ID)?.parentId).toBe(FOLDER_SPEC_ID);

      const dest = createFolder(ca.catalog, {
        createAt: null,
        name: `hp-${crypto.randomUUID().slice(0, 8)}`,
      });
      try {
        reparent(ca.catalog, PAGE_DOC_ID, { parentId: dest.id });
        throw new Error('expected home_protected');
      } catch (err) {
        expect(err).toBeInstanceOf(CatalogError);
        if (err instanceof CatalogError) expect(err.code).toBe('home_protected');
      }
      try {
        deleteNode(ca.catalog, PAGE_DOC_ID);
        throw new Error('expected home_protected');
      } catch (err) {
        expect(err).toBeInstanceOf(CatalogError);
        if (err instanceof CatalogError) expect(err.code).toBe('home_protected');
      }

      await expect
        .poll(() => getNode(cb.catalog, PAGE_DOC_ID)?.parentId, {
          timeout: 10_000,
        })
        .toBe(FOLDER_SPEC_ID);
      expect(getNode(cb.catalog, PAGE_DOC_ID)?.id).toBe(PAGE_DOC_ID);
      deleteNode(ca.catalog, dest.id);
    } finally {
      pa.disconnect(CATALOG_GUID);
      pa.disconnect(a.docId);
      pb.disconnect(CATALOG_GUID);
      pb.disconnect(b.docId);
    }
  });
});
