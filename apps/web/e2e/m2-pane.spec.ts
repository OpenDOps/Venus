import { expect, test, type Page } from '@playwright/test';

const SEED_H1 = 'Why Venus';
const NOTE = 'affine-note affine-paragraph rich-text';
const OUTLINE_H1 = '[data-testid="outline-block-preview-h1"]';
const MD_PANE = '[data-testid="venus-md-pane"]';

async function waitForEditor(page: Page) {
  await page.addInitScript(() => {
    window.__VENUS_E2E__ = true;
  });
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

test('pane visible: seed H1 in innerText; not contenteditable', async ({
  page,
}) => {
  const pageErrors = await waitForEditor(page);
  const pane = page.locator(MD_PANE);
  await pane.waitFor({ timeout: 30_000 });
  await expect(pane).toContainText(SEED_H1);
  await expect(pane).not.toHaveAttribute('contenteditable', 'true');
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('source highlighted: hljs span present; ATX heading stays in innerText', async ({
  page,
}) => {
  await waitForEditor(page);
  const pane = page.locator(MD_PANE);
  await pane.waitFor({ timeout: 30_000 });
  await expect(pane.locator('span[class*="hljs-"]').first()).toBeVisible({
    timeout: 15_000,
  });
  await expect(pane).toContainText('# Why Venus');
});

test('e2e-pane: typing in the note updates pane innerText and re-highlights', async ({
  page,
}) => {
  const pageErrors = await waitForEditor(page);
  const pane = page.locator(MD_PANE);
  await pane.waitFor({ timeout: 30_000 });
  await expect(pane).toContainText('# Why Venus');

  await page.locator(NOTE).first().click();
  await expect(page.locator('affine-page-root')).toBeFocused();
  await page.keyboard.type('m2-hello');
  await expect(page.locator(NOTE).first()).toContainText('m2-hello');

  await expect(pane).toContainText('m2-hello', { timeout: 5_000 });
  await expect(pane).not.toHaveAttribute('contenteditable', 'true');
  await expect(pane.locator('span[class*="hljs-"]').first()).toBeVisible();
  expect(pageErrors, pageErrors.join('\n')).toEqual([]);
});

test('pane equals helper: innerText is fromDoc markdown (highlight does not drop source)', async ({
  page,
}) => {
  await waitForEditor(page);
  const pane = page.locator(MD_PANE);
  await pane.waitFor({ timeout: 30_000 });
  await expect(pane).toContainText('# Why Venus');

  const markdown = await page.evaluate(async () => {
    const run = window.__VENUS_FROM_DOC__;
    if (!run) {
      throw new Error('window.__VENUS_FROM_DOC__ missing (e2e flag not set)');
    }
    const { markdown: file } = await run();
    return file;
  });
  const paneText = await pane.innerText();

  expect(markdown).toContain('# Why Venus');
  expect(markdown).toContain('## Empty host');
  expect(paneText.replace(/\r\n/g, '\n')).toBe(markdown.replace(/\r\n/g, '\n'));
});
