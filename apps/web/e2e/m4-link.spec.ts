import { execFile } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
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

/** `rel` is a blob in the wiki's HEAD commit (not just the working tree). */
async function committed(rel: string) {
  try {
    await promisify(execFile)('git', [
      '-c',
      `safe.directory=${wikiRoot}`,
      '-C',
      wikiRoot,
      'cat-file',
      '-e',
      `HEAD:${rel}`,
    ]);
    return true;
  } catch {
    return false;
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

async function cardView(page: Page, pageId: string) {
  return page.locator(CARD).evaluateAll((els, id) => {
    for (const el of els) {
      const host = el as {
        model?: {
          pageId?: string;
          style?: string;
          props?: { pageId?: string; style?: string };
        };
        shadowRoot?: ShadowRoot | null;
      };
      const model = host.model;
      const pid = model?.pageId ?? model?.props?.pageId ?? '';
      if (pid !== id) continue;
      const root = host.shadowRoot ?? (el as unknown as ParentNode);
      const frame = root.querySelector?.('.affine-embed-linked-doc-block');
      const title =
        root
          .querySelector?.('.affine-embed-linked-doc-content-title-text')
          ?.textContent?.trim() ?? '';
      return {
        style: model?.props?.style ?? model?.style ?? '',
        deleted: Boolean(frame?.classList.contains('deleted')),
        loading: Boolean(frame?.classList.contains('loading')),
        title,
      };
    }
    return null;
  }, pageId);
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
  expect(await committed(created.gitPath), created.gitPath).toBe(true);

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
  expect(await committed(`spec/${protocolName}.md`)).toBe(true);

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
  expect(await committed(moved.gitPath), moved.gitPath).toBe(true);
  expect(await committed(`spec/${protocolName}.md`)).toBe(false);

  await page.reload({ waitUntil: 'domcontentloaded' });
  await waitForCatalog(page);
  await page.evaluate((id) => window.__VENUS_OPEN_DOC__?.(id), PAGE_DOC_ID);
  await expectOpenDoc(page, PAGE_DOC_ID);
  await waitForCard(page, created.id);
  expect(await cardPageId(page, created.id)).toBe(created.id);
  await expect
    .poll(() => cardView(page, created.id), { timeout: 30_000 })
    .toEqual({
      style: 'horizontal',
      deleted: false,
      loading: false,
      title: protocolName,
    });
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('two tabs leave the linked-doc card untitled-free and do not rewrite home', async ({
  page,
  context,
}) => {
  const pageA = page;
  const errorsA = await waitForCatalog(pageA);
  const pageB = await context.newPage();
  const errorsB = await waitForCatalog(pageB);

  const created = await pageA.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  const protocolName = `protocol-${created.id.slice(0, 8)}`;
  await pageA.evaluate(
    ({ id, name }) => {
      const ops = window.__VENUS_CATALOG_OPS__;
      if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
      return ops.rename(id, name);
    },
    { id: created.id, name: protocolName },
  );
  await pageA.evaluate((pageId) => {
    const insert = window.__VENUS_INSERT_LINKED_DOC__;
    if (!insert) throw new Error('missing __VENUS_INSERT_LINKED_DOC__');
    insert(pageId);
  }, created.id);
  await waitForCard(pageA, created.id);
  await waitForCard(pageB, created.id);

  await expect
    .poll(() => cardView(pageB, created.id), { timeout: 30_000 })
    .toEqual({
      style: 'horizontal',
      deleted: false,
      loading: false,
      title: protocolName,
    });

  const vectorA = await pageA.evaluate(
    () => window.__VENUS_OPEN_VECTOR__?.() ?? '',
  );
  const vectorB = await pageB.evaluate(
    () => window.__VENUS_OPEN_VECTOR__?.() ?? '',
  );
  expect(vectorA).not.toBe('');
  expect(vectorB).not.toBe('');
  await pageA.waitForTimeout(2000);
  expect(await pageA.evaluate(() => window.__VENUS_OPEN_VECTOR__?.() ?? '')).toBe(
    vectorA,
  );
  expect(await pageB.evaluate(() => window.__VENUS_OPEN_VECTOR__?.() ?? '')).toBe(
    vectorB,
  );
  expect(errorsA, errorsA.join('\n')).toEqual([]);
  expect(errorsB, errorsB.join('\n')).toEqual([]);
});
