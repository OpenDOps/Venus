import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { PAGE_DOC_ID, WORKSPACE_ID } from '../src/host/ids.js';
import { assertHubOn3000, assertSidecarOn3002 } from './hub-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const CARD = 'affine-embed-linked-doc-block';
const FLUSH = '[data-testid="venus-flush"]';
const here = dirname(fileURLToPath(import.meta.url));
const wikiRoot = join(here, '../../../wiki');

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
  return pageErrors;
}

async function expectOpenDoc(page: Page, docId: string) {
  await expect
    .poll(() => page.evaluate(() => window.__VENUS_OPEN_DOC_ID__ ?? ''), {
      timeout: 30_000,
    })
    .toBe(docId);
}

async function flushAndWait(page: Page, probe: () => Promise<boolean>) {
  const flush = page.locator(FLUSH);
  await expect(flush).toBeVisible();
  await flush.click();
  const sidecar =
    process.env.VITE_SIDECAR_URL?.replace(/\/$/, '') ??
    'http://127.0.0.1:3002';
  await page.request.post(`${sidecar}/flush`);
  await expect
    .poll(probe, { timeout: 60_000, intervals: [250, 500, 1000] })
    .toBe(true);
}

async function readWiki(rel: string) {
  try {
    return await readFile(join(wikiRoot, rel), 'utf8');
  } catch {
    return '';
  }
}

async function waitForCard(page: Page, pageId: string) {
  await expect
    .poll(
      () =>
        page.locator(CARD).evaluateAll((els, id) => {
          return els.some((el) => {
            const model = (
              el as {
                model?: { pageId?: string; props?: { pageId?: string } };
              }
            ).model;
            return (model?.pageId ?? model?.props?.pageId) === id;
          });
        }, pageId),
      { timeout: 30_000 },
    )
    .toBe(true);
}

async function cardPageId(page: Page, pageId: string) {
  return page.locator(CARD).evaluateAll((els, id) => {
    for (const el of els) {
      const model = (
        el as {
          model?: { pageId?: string; props?: { pageId?: string } };
        }
      ).model;
      const pid = model?.pageId ?? model?.props?.pageId ?? '';
      if (pid === id) return pid;
    }
    return '';
  }, pageId);
}

test.beforeAll(async () => {
  await assertHubOn3000();
  await assertSidecarOn3002();
});

test('embed card survives rename+move; git hrefs follow catalog', async ({
  page,
}) => {
  const pageErrors = await waitForCatalog(page);
  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  await page.evaluate((id) => window.__VENUS_OPEN_DOC__?.(id), PAGE_DOC_ID);
  await expectOpenDoc(page, PAGE_DOC_ID);
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });

  await page.evaluate((pageId) => {
    const insert = window.__VENUS_INSERT_LINKED_DOC__;
    if (!insert) throw new Error('missing __VENUS_INSERT_LINKED_DOC__');
    insert(pageId);
  }, created.id);
  await waitForCard(page, created.id);
  await page.waitForTimeout(2000);

  await flushAndWait(page, async () => {
    const home = await readWiki('spec/home.md');
    const links = await readWiki('.venus/links.json');
    return (
      home.includes(`<!-- venus:doc:${created.id} -->`) &&
      !home.includes(`./workspace/${WORKSPACE_ID}/${created.id}`) &&
      links.includes(created.id) &&
      links.includes(PAGE_DOC_ID)
    );
  });
  const homeAfterCreate = await readWiki('spec/home.md');
  expect(homeAfterCreate).toContain(`<!-- venus:doc:${created.id} -->`);
  expect(homeAfterCreate).not.toContain(
    `./workspace/${WORKSPACE_ID}/${created.id}`,
  );

  const protocolName = `protocol-${created.id.slice(0, 8)}`;
  await page.evaluate(
    ({ id, name }) => {
      const ops = window.__VENUS_CATALOG_OPS__;
      if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
      return ops.rename(id, name);
    },
    { id: created.id, name: protocolName },
  );
  await page.waitForTimeout(3000);
  await flushAndWait(page, async () => {
    const home = await readWiki('spec/home.md');
    return (
      home.includes(`[${protocolName}]`) && home.includes(`${protocolName}.md`)
    );
  });

  const designName = `design-${created.id.slice(0, 8)}`;
  const design = await page.evaluate((name) => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createFolder(null, name);
  }, designName);
  const moved = await page.evaluate(
    ({ id, parentId }) => {
      const ops = window.__VENUS_CATALOG_OPS__;
      if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
      return ops.reparent(id, parentId);
    },
    { id: created.id, parentId: design.id },
  );
  expect(moved.gitPath).toBe(`${designName}/${protocolName}.md`);
  await page.waitForTimeout(3000);
  await flushAndWait(page, async () => {
    const home = await readWiki('spec/home.md');
    return home.includes(`../${designName}/${protocolName}.md`);
  });
  const homeAfterMove = await readWiki('spec/home.md');
  expect(homeAfterMove).toContain(`<!-- venus:doc:${created.id} -->`);
  expect(homeAfterMove).toContain(
    `[${protocolName}](../${designName}/${protocolName}.md)`,
  );

  await page.reload({ waitUntil: 'domcontentloaded' });
  await waitForCatalog(page);
  await page.evaluate((id) => window.__VENUS_OPEN_DOC__?.(id), PAGE_DOC_ID);
  await expectOpenDoc(page, PAGE_DOC_ID);
  await waitForCard(page, created.id);
  expect(await cardPageId(page, created.id)).toBe(created.id);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});
