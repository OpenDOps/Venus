import { expect, test, type Page } from '@playwright/test';

const SEED_H1 = 'Why Venus';
const SEED_H2 = 'Empty host';
const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';

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
  await page.locator(OUTLINE_H1).waitFor({ timeout: 30_000 });
  return pageErrors;
}

test('smoke: outline lists H1 and typing works', async ({ page }) => {
  const pageErrors = await waitForEditor(page);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type('hello');
  await expect(page.locator(NOTE).first()).toContainText('hello');
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('refresh discards typed text and restores seed headings', async ({
  page,
}) => {
  await waitForEditor(page);
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type('hello');
  await expect(page.locator(NOTE).first()).toContainText('hello');

  await page.reload({ waitUntil: 'domcontentloaded' });
  await page.locator('affine-editor-container').waitFor({ timeout: 120_000 });
  await page.locator(NOTE).first().waitFor({ timeout: 30_000 });

  await expect(page.locator(NOTE).first()).not.toContainText('hello');
  await expect(
    page.locator('.affine-paragraph-rich-text-wrapper.h1'),
  ).toContainText(SEED_H1);
  await expect(
    page.locator('.affine-paragraph-rich-text-wrapper.h2'),
  ).toContainText(SEED_H2);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
});
