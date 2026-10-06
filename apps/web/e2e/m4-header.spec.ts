import { expect, test, type Page } from '@playwright/test';
import { assertHubOn3000 } from './hub-ws';

const NOTE = 'affine-note affine-paragraph rich-text';
const SEED_H1 = 'Why Venus';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';

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
  await page.getByTestId('venus-header').waitFor({ timeout: 30_000 });
  return pageErrors;
}

async function focusNote(page: Page) {
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
}

test.beforeAll(async () => {
  await assertHubOn3000();
});

test('header undo/redo binds the open Store via canUndo$', async ({ page }) => {
  const pageErrors = await waitForCatalog(page);
  const word = `m4-undo-${Date.now()}`;
  await focusNote(page);
  await page.keyboard.type(word);
  const paragraph = page.locator(NOTE).first();
  await expect(paragraph).toContainText(word);

  const undo = page.getByTestId('venus-undo');
  const redo = page.getByTestId('venus-redo');
  await expect(undo).toBeEnabled();
  for (let i = 0; i < 20; i++) {
    const text = (await paragraph.textContent()) ?? '';
    if (!text.includes(word)) break;
    await undo.click();
  }
  await expect(paragraph).not.toContainText(word);
  await expect(undo).toBeDisabled();
  await expect(redo).toBeEnabled();
  for (let i = 0; i < 20; i++) {
    const text = (await paragraph.textContent()) ?? '';
    if (text.includes(word)) break;
    await redo.click();
  }
  await expect(paragraph).toContainText(word);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('venus-page-title is catalog name of the open node', async ({ page }) => {
  await waitForCatalog(page);
  await expect(page.getByTestId('venus-page-title')).toHaveValue('home');

  const created = await page.evaluate(() => {
    const ops = window.__VENUS_CATALOG_OPS__;
    if (!ops) throw new Error('missing __VENUS_CATALOG_OPS__');
    return ops.createDoc('folder:spec');
  });
  await page.evaluate((id) => {
    window.__VENUS_OPEN_DOC__?.(id);
  }, created.id);
  await expect
    .poll(() => page.evaluate(() => window.__VENUS_OPEN_DOC_ID__ ?? ''), {
      timeout: 30_000,
    })
    .toBe(created.id);
  await expect(page.getByTestId('venus-page-title')).toHaveValue(created.id);
  await expect(page.getByTestId('venus-page-title')).not.toHaveValue('Venus');
  await page.evaluate((id) => {
    window.__VENUS_OPEN_DOC__?.('doc:home');
    window.__VENUS_CATALOG_OPS__?.deleteNode(id);
  }, created.id);
});

test('layout: header and tree slot; outline stays headings', async ({
  page,
}) => {
  await waitForCatalog(page);
  await expect(page.getByTestId('venus-header')).toBeVisible();
  await expect(page.getByTestId('venus-tree')).toBeVisible();
  await expect(page.getByTestId('venus-tree-home')).toBeVisible();
  await expect(page.getByTestId('venus-tree')).toContainText('spec');
  await expect(page.getByTestId('venus-tree')).not.toContainText(SEED_H1);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
});
