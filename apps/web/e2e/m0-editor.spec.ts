import { expect, test, type Page } from '@playwright/test';

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

async function focusParagraph(page: Page) {
  // BlockSuite moves document focus to affine-page-root; the inline editor
  // stays inactive. Click still sets TextSelection, which slash/type need.
  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
}

test('type into the note', async ({ page }) => {
  const pageErrors = await waitForEditor(page);
  await focusParagraph(page);
  await page.keyboard.type('hello');
  await expect(page.locator(NOTE).first()).toContainText('hello');
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('slash menu opens', async ({ page }) => {
  await waitForEditor(page);
  await focusParagraph(page);
  // Slash widget reads TextSelection on keydown and then waits for
  // inlineRangeSync. Give the click's selection a frame to land.
  await page.waitForTimeout(200);
  await page.keyboard.type('/');
  await expect(page.locator('affine-slash-menu .slash-menu')).toBeVisible({
    timeout: 15_000,
  });
});

test('undo reverts typed text', async ({ page }) => {
  await waitForEditor(page);
  await focusParagraph(page);
  await page.keyboard.type('hello');
  const paragraph = page.locator(NOTE).first();
  await expect(paragraph).toContainText('hello');
  for (let i = 0; i < 10; i++) {
    const text = (await paragraph.textContent()) ?? '';
    if (!text.includes('hello')) break;
    await page.keyboard.press('ControlOrMeta+z');
  }
  await expect(paragraph).not.toContainText('hello');
});
