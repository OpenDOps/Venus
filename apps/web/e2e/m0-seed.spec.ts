import { expect, test, type Page } from '@playwright/test';

const SEED_TITLE = 'Venus';
const SEED_H1 = 'Why Venus';
const SEED_H2 = 'Empty host';

const NOTE = 'affine-note affine-paragraph rich-text';

async function waitForEditor(page: Page) {
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
      await page.locator('affine-editor-container').waitFor({
        timeout: 120_000,
      });
    })(),
    failOnError,
  ]).catch((err) => {
    throw new Error(
      `affine-editor-container did not mount.\n${pageErrors.join('\n') || String(err)}`,
    );
  });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });
  return pageErrors;
}

test('fresh load shows title, H1, and H2', async ({ page }) => {
  const pageErrors = await waitForEditor(page);
  await expect(page.locator('doc-title')).toContainText(SEED_TITLE);
  await expect(
    page.locator('.affine-paragraph-rich-text-wrapper.h1'),
  ).toContainText(SEED_H1);
  await expect(
    page.locator('.affine-paragraph-rich-text-wrapper.h2'),
  ).toContainText(SEED_H2);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('one undo does not delete the seeded page tree', async ({ page }) => {
  await waitForEditor(page);
  await page.keyboard.press('ControlOrMeta+z');
  await expect(page.locator('doc-title')).toContainText(SEED_TITLE);
  await expect(
    page.locator('.affine-paragraph-rich-text-wrapper.h1'),
  ).toContainText(SEED_H1);
  await expect(
    page.locator('.affine-paragraph-rich-text-wrapper.h2'),
  ).toContainText(SEED_H2);
});
