import { expect, test, type Page } from '@playwright/test';
import { CATALOG_SQL_ID, PAGE_DOC_ID, collaborationSocketUrl } from '../src/host/ids.js';
import { expectedCollaborationWs, assertHubOn3000 } from './hub-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const HUB_WS = expectedCollaborationWs();
const CATALOG_WS = `${HUB_WS}?doc=${CATALOG_SQL_ID}`;

test.describe.configure({ mode: 'serial' });

async function waitForCatalog(page: Page) {
  const pageErrors: string[] = [];
  const failOnError = new Promise<never>((_, reject) => {
    page.on('pageerror', (err) => {
      pageErrors.push(String(err));
      reject(err);
    });
  });
  await Promise.race([
    (async () => {
      await page.goto('/', { waitUntil: 'domcontentloaded' });
      await page.waitForFunction(
        () => window.__VENUS_CATALOG_READY__ === true,
        null,
        { timeout: 120_000 },
      );
      await page.locator('affine-editor-container').waitFor({
        timeout: 120_000,
      });
    })(),
    failOnError,
  ]).catch((err) => {
    throw new Error(
      `catalog did not become ready.\n${pageErrors.join('\n') || String(err)}`,
    );
  });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
}

function collectHubSockets(page: Page) {
  const urls: string[] = [];
  page.on('websocket', (ws) => {
    urls.push(ws.url());
  });
  return urls;
}

test.beforeAll(async () => {
  await assertHubOn3000();
});

test('A createDoc appears on B catalog without reload', async ({
  page,
  context,
}) => {
  const pageA = page;
  const wsA = collectHubSockets(pageA);
  await waitForCatalog(pageA);

  const pageB = await context.newPage();
  const wsB = collectHubSockets(pageB);
  await waitForCatalog(pageB);

  expect(
    wsA.find((u) => u === HUB_WS),
    'tab A must open the home hub websocket',
  ).toBe(HUB_WS);
  expect(
    wsA.find((u) => u === CATALOG_WS),
    'tab A must open the catalog hub websocket',
  ).toBe(CATALOG_WS);
  expect(
    wsB.find((u) => u === CATALOG_WS),
    'tab B must open the catalog hub websocket',
  ).toBe(CATALOG_WS);

  const seedB = await pageB.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    const ids = ops.snapshot().map((n) => n.id);
    const spec = ops.getNode('folder:spec');
    const home = ops.getNode('doc:home');
    return {
      ids,
      specGitPath: spec?.gitPath ?? null,
      homeParent: home?.parentId ?? null,
      homeGitPath: home?.gitPath ?? null,
    };
  });
  expect(seedB.ids).toContain('folder:spec');
  expect(seedB.ids).toContain(PAGE_DOC_ID);
  expect(seedB.ids).not.toContain('doc:protocol');
  expect(seedB.specGitPath).toBeTruthy();
  expect(seedB.homeParent).toBe('folder:spec');
  expect(seedB.homeGitPath).toMatch(/home\.md$/);

  const created = await pageA.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  expect(created.id).toMatch(
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i,
  );
  expect(created.gitPath).toBe(`spec/${created.id}.md`);
  expect(
    wsA.find((u) => u === collaborationSocketUrl(HUB_WS, created.id)),
    'createDoc must not open a page hub websocket (step 5 openWorkspaceDoc)',
  ).toBeUndefined();
  expect(
    wsB.find((u) => u === collaborationSocketUrl(HUB_WS, created.id)),
  ).toBeUndefined();

  await expect
    .poll(async () => {
      return pageB.evaluate((id) => {
        return window.__VENUS_CATALOG_OPS__?.getNode(id)?.gitPath ?? null;
      }, created.id);
    })
    .toBe(created.gitPath);

  const moved = await pageA.evaluate((id) => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    const folder = ops.createFolder(null, `design-${id.slice(0, 8)}`);
    return { ...ops.reparent(id, folder.id), folderId: folder.id };
  }, created.id);

  await expect
    .poll(async () => {
      return pageB.evaluate((id) => {
        const node = window.__VENUS_CATALOG_OPS__?.getNode(id);
        return node ? { gitPath: node.gitPath, parentId: node.parentId } : null;
      }, created.id);
    })
    .toEqual({ gitPath: moved.gitPath, parentId: moved.folderId });

  await pageA.evaluate(
    ({ pageId, folderId }) => {
      window.__VENUS_CATALOG_OPS__?.deleteNode(pageId);
      window.__VENUS_CATALOG_OPS__?.deleteNode(folderId);
    },
    { pageId: created.id, folderId: moved.folderId },
  );
  await expect
    .poll(async () => {
      return pageB.evaluate((id) => {
        return window.__VENUS_CATALOG_OPS__?.getNode(id) ?? null;
      }, created.id);
    })
    .toBeNull();

  const dest = await pageA.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createFolder(null, `hp-${Date.now()}`);
  });
  const blocked = await pageA.evaluate((folderId) => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    const out: { op: string; threw: boolean; code: string | null }[] = [];
    try {
      ops.reparent('doc:home', folderId);
      out.push({ op: 'reparent', threw: false, code: null });
    } catch (err) {
      out.push({
        op: 'reparent',
        threw: true,
        code:
          err && typeof err === 'object' && 'code' in err
            ? String((err as { code: unknown }).code)
            : null,
      });
    }
    try {
      ops.deleteNode('doc:home');
      out.push({ op: 'delete', threw: false, code: null });
    } catch (err) {
      out.push({
        op: 'delete',
        threw: true,
        code:
          err && typeof err === 'object' && 'code' in err
            ? String((err as { code: unknown }).code)
            : null,
      });
    }
    return out;
  }, dest.id);
  expect(blocked).toEqual([
    { op: 'reparent', threw: true, code: 'home_protected' },
    { op: 'delete', threw: true, code: 'home_protected' },
  ]);
  await expect
    .poll(async () => {
      return pageB.evaluate(() => {
        const home = window.__VENUS_CATALOG_OPS__?.getNode('doc:home');
        return home
          ? { id: home.id, parentId: home.parentId }
          : null;
      });
    })
    .toEqual({ id: PAGE_DOC_ID, parentId: 'folder:spec' });
  await pageA.evaluate((folderId) => {
    window.__VENUS_CATALOG_OPS__?.deleteNode(folderId);
  }, dest.id);
});
