import { expect, test, type BrowserContext, type Page } from '@playwright/test';
import { CATALOG_SQL_ID } from '../src/host/ids.js';
import {
  expectedCollaborationWs,
  assertHubOn3000,
  forceTabSockets,
} from './hub-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const HUB_WS = expectedCollaborationWs();
const CATALOG_WS = `${HUB_WS}?doc=${CATALOG_SQL_ID}`;

test.describe.configure({ mode: 'serial' });

async function waitForCatalog(page: Page) {
  const pageErrors: string[] = [];
  page.on('pageerror', (err) => pageErrors.push(String(err)));
  await page.goto('/', { waitUntil: 'domcontentloaded' });
  await page
    .waitForFunction(() => window.__VENUS_CATALOG_READY__ === true, null, {
      timeout: 120_000,
    })
    .catch((err) => {
      throw new Error(
        `catalog did not become ready.\n${pageErrors.join('\n') || String(err)}`,
      );
    });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  await page.getByTestId('venus-tree').waitFor({ timeout: 30_000 });
}

function collectPageSockets(page: Page) {
  const urls: string[] = [];
  page.on('websocket', (ws) => urls.push(ws.url()));
  return urls;
}

function transport(page: Page) {
  return page.evaluate(() => window.__VENUS_HUB_TRANSPORT__ ?? null);
}

function stats(page: Page) {
  return page.evaluate(async () => (await window.__VENUS_HUB_STATS__?.()) ?? null);
}

/** A in page A creates a folder + page and drops the page into the folder; B sees it. */
async function dropSyncs(pageA: Page, pageB: Page) {
  const created = await pageA.evaluate(async () => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    const folder = ops.createFolder(null, `sw-${Date.now()}`);
    const doc = await ops.createDoc('folder:spec');
    return { folderId: folder.id, docId: doc.id };
  });
  const dropped = await pageA.evaluate(
    ({ docId, folderId }) => window.__VENUS_CATALOG_OPS__!.drop(docId, folderId),
    created,
  );
  expect(dropped.ok).toBe(true);
  await expect
    .poll(
      () =>
        pageB.evaluate((id) => {
          const node = window.__VENUS_CATALOG_OPS__?.getNode(id);
          return node ? { parentId: node.parentId, gitPath: node.gitPath } : null;
        }, created.docId),
      { timeout: 15_000 },
    )
    .toEqual({ parentId: created.folderId, gitPath: dropped.gitPath });
  await expect(
    pageB.locator(`[data-catalog-id="${created.folderId}"]`),
    'the tree in B renders the new folder (not a blank tree)',
  ).toBeVisible();
  await pageA.evaluate(({ docId, folderId }) => {
    window.__VENUS_CATALOG_OPS__?.deleteNode(docId);
    window.__VENUS_CATALOG_OPS__?.deleteNode(folderId);
  }, created);
}

async function hasSharedWorker(context: BrowserContext) {
  const probe = await context.newPage();
  try {
    await probe.goto('about:blank');
    return await probe.evaluate(() => typeof SharedWorker === 'function');
  } finally {
    await probe.close();
  }
}

test.beforeAll(async () => {
  await assertHubOn3000();
});

test('Fallback: without SharedWorker each tab opens its own sockets and the drop still syncs', async ({
  page,
  context,
}) => {
  await forceTabSockets(context);
  const wsA = collectPageSockets(page);
  await waitForCatalog(page);
  const pageB = await context.newPage();
  const wsB = collectPageSockets(pageB);
  await waitForCatalog(pageB);

  for (const tab of [page, pageB]) {
    expect(await transport(tab)).toBe('tab');
    expect(await stats(tab)).toBeNull();
    await expect(tab.getByTestId('venus-tree-home')).toBeVisible();
  }
  expect(wsA).toContain(HUB_WS);
  expect(wsA).toContain(CATALOG_WS);
  expect(wsB).toContain(CATALOG_WS);

  await dropSyncs(page, pageB);
});

test('Share in one profile: two pages use the worker-held sockets; drop and typing sync', async ({
  page,
  context,
}) => {
  test.skip(
    !(await hasSharedWorker(context)),
    'this browser has no SharedWorker; Fallback covers the per-tab path',
  );
  const wsA = collectPageSockets(page);
  await waitForCatalog(page);
  const pageB = await context.newPage();
  const wsB = collectPageSockets(pageB);
  await waitForCatalog(pageB);

  for (const tab of [page, pageB]) {
    expect(await transport(tab)).toBe('shared-worker');
  }
  expect(
    [...wsA, ...wsB].filter((u) => u.startsWith(HUB_WS)),
    'tabs must not open their own hub sockets while the worker holds them',
  ).toEqual([]);

  const table = await stats(page);
  expect(table).not.toBeNull();
  expect(table!.ports).toBe(2);
  const urls = table!.sockets.map((s) => s.url);
  expect(new Set(urls).size, 'one socket per wire A URL').toBe(urls.length);
  expect(table!.sockets.find((s) => s.url === CATALOG_WS)).toMatchObject({
    readyState: 1,
    channels: 2,
    ports: 2,
  });
  expect(table!.sockets.find((s) => s.url === HUB_WS)).toMatchObject({
    readyState: 1,
    channels: 2,
    ports: 2,
  });

  await dropSyncs(page, pageB);

  // The hub does not echo a socket's own update; the worker relays it to B.
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  const typed = `sw-${Date.now()}`;
  await page.keyboard.type(typed);
  await expect(pageB.locator(NOTE).first()).toContainText(typed, {
    timeout: 10_000,
  });

  await pageB.close();
  await expect
    .poll(async () => (await stats(page))?.sockets.find((s) => s.url === CATALOG_WS)?.ports, {
      timeout: 10_000,
    })
    .toBe(1);
});

test('Two contexts do not share a worker', async ({ page, browser }) => {
  test.skip(
    !(await hasSharedWorker(page.context())),
    'this browser has no SharedWorker',
  );
  await waitForCatalog(page);
  const other = await browser.newContext({
    baseURL: test.info().project.use.baseURL,
  });
  try {
    const pageB = await other.newPage();
    await waitForCatalog(pageB);
    for (const tab of [page, pageB]) {
      const table = await stats(tab);
      expect(table?.ports).toBe(1);
      expect(table?.sockets.find((s) => s.url === CATALOG_WS)?.ports).toBe(1);
    }
    await dropSyncs(page, pageB);
  } finally {
    await other.close();
  }
});
