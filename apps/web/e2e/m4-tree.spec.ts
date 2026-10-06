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
  await page.getByTestId('venus-tree').waitFor({ timeout: 30_000 });
}

async function expectOpenDoc(page: Page, docId: string) {
  await expect
    .poll(() => page.evaluate(() => window.__VENUS_OPEN_DOC_ID__ ?? ''), {
      timeout: 30_000,
    })
    .toBe(docId);
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

test('tree lists spec and home; outline stays Why Venus', async ({ page }) => {
  await waitForCatalog(page);
  const tree = page.getByTestId('venus-tree');
  await expect(tree).toBeVisible();
  await expect(tree).toContainText('spec');
  await expect(page.getByTestId('venus-tree-home')).toContainText('home');
  await expect(tree).not.toContainText('Why Venus');
  await expect(
    page.locator('[data-testid="outline-block-preview-h1"]'),
  ).toContainText('Why Venus');

  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  await expect(page.locator(`[data-catalog-id="${created.id}"]`)).toBeVisible();
  await page.evaluate((id) => {
    window.__VENUS_CATALOG_OPS__?.deleteNode(id);
  }, created.id);
});

test('left-click spec selects without collapsing children', async ({ page }) => {
  await waitForCatalog(page);
  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  await expect(page.locator(`[data-catalog-id="${created.id}"]`)).toBeVisible();
  await page.locator('[data-catalog-id="folder:spec"]').click();
  await expect(page.locator(`[data-catalog-id="${created.id}"]`)).toBeVisible();
  await expect(page.getByTestId('venus-delete-node')).toHaveCount(0);
  await page.locator('[data-catalog-expand="folder:spec"]').click();
  await expect(page.locator(`[data-catalog-id="${created.id}"]`)).toHaveCount(0);
  await page.locator('[data-catalog-expand="folder:spec"]').click();
  await expect(page.locator(`[data-catalog-id="${created.id}"]`)).toBeVisible();
  await page.evaluate((id) => {
    window.__VENUS_CATALOG_OPS__?.deleteNode(id);
  }, created.id);
});

test('click a created page opens that Store', async ({ page }) => {
  await waitForCatalog(page);
  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  await page.locator(`[data-catalog-id="${created.id}"]`).click();
  await expectOpenDoc(page, created.id);
  await expect(page.getByTestId('venus-page-title')).toHaveValue(created.id);
  await expect(
    page.locator('[data-testid="outline-block-preview-h1"]'),
  ).toHaveCount(0);
  await page.getByTestId('venus-tree-home').click();
  await expectOpenDoc(page, PAGE_DOC_ID);
  await page.evaluate((id) => {
    window.__VENUS_CATALOG_OPS__?.deleteNode(id);
  }, created.id);
});

test('drop reparents via published drop API; home stays under spec', async ({
  page,
  context,
}) => {
  const pageA = page;
  await waitForCatalog(pageA);
  const pageB = await context.newPage();
  await waitForCatalog(pageB);

  const created = await pageA.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    const folder = ops.createFolder(null, `design-${Date.now()}`);
    const doc = ops.createDoc('folder:spec');
    return {
      folderId: folder.id,
      folderGitPath: folder.gitPath,
      docId: doc.id,
    };
  });

  const dropped = await pageA.evaluate(
    ({ docId, folderId }) => {
      const ops = window.__VENUS_CATALOG_OPS__;
      if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
      return ops.drop(docId, folderId);
    },
    { docId: created.docId, folderId: created.folderId },
  );
  expect(dropped.ok).toBe(true);
  expect(dropped.parentId).toBe(created.folderId);
  expect(dropped.gitPath).toBe(`${created.folderGitPath}/${created.docId}.md`);

  await expect
    .poll(async () => {
      return pageB.evaluate((id) => {
        const node = window.__VENUS_CATALOG_OPS__?.getNode(id);
        return node ? { gitPath: node.gitPath, parentId: node.parentId } : null;
      }, created.docId);
    })
    .toEqual({ gitPath: dropped.gitPath, parentId: created.folderId });

  const homeDrop = await pageA.evaluate((folderId) => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return {
      can: ops.canDrop('doc:home', folderId),
      drop: ops.drop('doc:home', folderId),
    };
  }, created.folderId);
  expect(homeDrop.can).toBe(false);
  expect(homeDrop.drop.ok).toBe(false);
  expect(homeDrop.drop.parentId).toBe('folder:spec');
  await expect(pageA.getByTestId('venus-tree-home')).toBeVisible();

  await pageA.evaluate(
    ({ pageId, folderId }) => {
      window.__VENUS_CATALOG_OPS__?.deleteNode(pageId);
      window.__VENUS_CATALOG_OPS__?.deleteNode(folderId);
    },
    { pageId: created.docId, folderId: created.folderId },
  );
});

test('delete is hidden on spec; deleting an open leaf switches to home', async ({
  page,
  context,
}) => {
  await waitForCatalog(page);
  await page.locator('[data-catalog-id="folder:spec"]').click({ button: 'right' });
  await expect(page.getByTestId('venus-delete-node')).toHaveCount(0);

  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  await expect(page.locator(`[data-catalog-id="${created.id}"]`)).toBeVisible();
  await page.locator(`[data-catalog-id="${created.id}"]`).click();
  await expectOpenDoc(page, created.id);
  await expect(page.getByTestId('venus-delete-node')).toBeVisible();

  const pageB = await context.newPage();
  await waitForCatalog(pageB);
  await pageB.evaluate((id) => {
    window.__VENUS_OPEN_DOC__?.(id);
  }, created.id);
  await expectOpenDoc(pageB, created.id);

  await page.getByTestId('venus-delete-node').click();
  await expectOpenDoc(page, PAGE_DOC_ID);
  await expect(page.getByTestId('venus-page-title')).toHaveValue('home');
  await expectOpenDoc(pageB, PAGE_DOC_ID);
  await expect(pageB.getByTestId('venus-page-title')).toHaveValue('home');

  await page.evaluate((id) => {
    window.__VENUS_OPEN_DOC__?.(id);
  }, created.id);
  await expectOpenDoc(page, PAGE_DOC_ID);

  const specBlocked = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    try {
      ops.deleteNode('folder:spec');
      return { threw: false, code: null };
    } catch (err) {
      return {
        threw: true,
        code:
          err && typeof err === 'object' && 'code' in err
            ? String((err as { code: unknown }).code)
            : null,
      };
    }
  });
  expect(specBlocked).toEqual({ threw: true, code: 'node_not_empty' });
});

