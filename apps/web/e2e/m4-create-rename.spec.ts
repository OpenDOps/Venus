import { readFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import { PAGE_DOC_ID } from '../src/host/ids.js';
import { assertHubOn3000, assertSidecarOn3002 } from './hub-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
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

async function wikiExists(rel: string) {
  const text = await readWiki(rel);
  return text.length > 0;
}

test.beforeAll(async () => {
  await assertHubOn3000();
  await assertSidecarOn3002();
});

test('create Flush rename Flush delete Flush writes git mv and git rm', async ({
  page,
}) => {
  const pageErrors = await waitForCatalog(page);
  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  expect(created.id).toMatch(
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i,
  );
  expect(created.gitPath).toBe(`spec/${created.id}.md`);
  expect(created.docId).toBe(created.id);

  await page.evaluate((id) => window.__VENUS_OPEN_DOC__?.(id), created.id);
  try {
    await expectOpenDoc(page, created.id);
  } catch {
    await page.waitForTimeout(2000);
    await page.evaluate((id) => window.__VENUS_OPEN_DOC__?.(id), created.id);
    await expectOpenDoc(page, created.id);
  }
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  const word = `m4-cr-${Date.now()}`;
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type(word);
  await expect(page.locator(NOTE).first()).toContainText(word);
  await page.waitForTimeout(2000);

  const uuidMd = `spec/${created.id}.md`;
  await flushAndWait(page, async () => {
    const md = await readWiki(uuidMd);
    const yaml = await readWiki('.venus/pages.yaml');
    return (
      md.includes(word) &&
      yaml.includes(`${uuidMd}:`) &&
      yaml.includes(`uuid: ${created.id}`) &&
      yaml.includes('folder:spec')
    );
  });
  expect(await wikiExists(`.venus/ids/${created.id}.json`)).toBe(true);
  expect(await wikiExists('spec/home.md')).toBe(true);
  const uuidBlob = await readWiki(uuidMd);
  expect(uuidBlob).toContain(word);

  const protocolName = `protocol-${created.id.slice(0, 8)}`;
  const renamed = await page.evaluate(
    ({ id, name }) => {
      const ops = window.__VENUS_CATALOG_OPS__;
      if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
      return ops.rename(id, name);
    },
    { id: created.id, name: protocolName },
  );
  expect(renamed.name).toBe(protocolName);
  expect(renamed.gitPath).toBe(`spec/${protocolName}.md`);
  await page.waitForTimeout(3000);

  const protocolMd = `spec/${protocolName}.md`;
  await flushAndWait(page, async () => {
    const yaml = await readWiki('.venus/pages.yaml');
    const protocol = await readWiki(protocolMd);
    const old = await readWiki(uuidMd);
    return (
      protocol.length > 0 &&
      old.length === 0 &&
      yaml.includes(`${protocolMd}:`) &&
      yaml.includes(`name: ${protocolName}`) &&
      yaml.includes(`uuid: ${created.id}`) &&
      !yaml.includes(`${uuidMd}:`)
    );
  });
  expect(await readWiki(protocolMd)).toBe(uuidBlob);

  await page.evaluate((id) => {
    window.__VENUS_CATALOG_OPS__?.deleteNode(id);
  }, created.id);
  await expectOpenDoc(page, PAGE_DOC_ID);
  await page.waitForTimeout(3000);
  await flushAndWait(page, async () => {
    const yaml = await readWiki('.venus/pages.yaml');
    const gone = (await readWiki(protocolMd)).length === 0;
    return gone && !yaml.includes(created.id);
  });
  expect(await wikiExists('spec/home.md')).toBe(true);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});
