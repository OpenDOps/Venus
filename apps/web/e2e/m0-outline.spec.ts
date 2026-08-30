import { expect, test, type Page } from '@playwright/test';

const SEED_H1 = 'Why Venus';
const SEED_H1_BODY =
  'A thin host around BlockSuite: one workspace, one page.';
const SEED_H2 = 'Empty host';

const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE = 'affine-outline-panel';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';
const OUTLINE_H2 = '[data-testid="outline-block-preview-h2"]';

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
  await page.locator(OUTLINE).waitFor({ timeout: 30_000 });
  return pageErrors;
}

test('outline lists seeded headings', async ({ page }) => {
  const pageErrors = await waitForEditor(page);
  await expect(page.locator(OUTLINE_H1)).toContainText(SEED_H1);
  await expect(page.locator(OUTLINE_H2)).toContainText(SEED_H2);
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('outline label tracks H1 edits', async ({ page }) => {
  await waitForEditor(page);
  await page.locator('.affine-paragraph-rich-text-wrapper.h1').click();
  await page.keyboard.press('ControlOrMeta+a');
  await page.keyboard.type('Renamed heading');
  await expect(page.locator(OUTLINE_H1)).toContainText('Renamed heading');
});

test('clicking H2 in the outline scrolls the editor to it', async ({ page }) => {
  await waitForEditor(page);
  const h2 = page.locator('.affine-paragraph-rich-text-wrapper.h2');
  await expect(h2).not.toBeInViewport();
  await page.locator(OUTLINE_H2).click();
  await expect(h2).toBeInViewport({ timeout: 15_000 });
});

test('outline is in-page headings only, not a wiki folder tree', async ({
  page,
}) => {
  await waitForEditor(page);
  const outline = page.locator(OUTLINE);
  await expect(outline).toContainText(SEED_H1);
  await expect(outline).toContainText(SEED_H2);
  await expect(outline).not.toContainText(SEED_H1_BODY);
  await expect(page.locator(`${OUTLINE} [data-testid*="folder"]`)).toHaveCount(
    0,
  );
});
