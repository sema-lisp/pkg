import { readFile } from 'node:fs/promises';
import { test, expect } from './helpers/fixtures';

test('published API reference, source, Markdown, offline print, and mobile layout', async ({ page, request, apiToken }) => {
  const name = `api-reference-${Date.now()}`;
  const release = `/packages/${name}/0.1.0`;
  const archive = await readFile(`${__dirname}/../../tests/fixtures/sema-retry-0.1.0.tar.gz`);
  const publish = await request.put(`/api/v1${release}`, {
    headers: { authorization: `Bearer ${apiToken}` },
    multipart: {
      metadata: JSON.stringify({ description: 'Retry with exponential backoff, circuit breaker, and rate limiting' }),
      tarball: { name: 'sema-retry-0.1.0.tar.gz', mimeType: 'application/gzip', buffer: archive },
    },
  });
  expect(publish.status()).toBe(201);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(release);
  const packageHeader = await page.locator('.site-header').innerText();
  await page.getByRole('link', { name: 'API reference', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`${release}/api$`));
  expect(await page.locator('.site-header').innerText()).toBe(packageHeader);
  await expect(page.locator('[data-api-entry]')).toHaveCount(9);
  await expect(page.getByRole('navigation', { name: 'Breadcrumb' })).toContainText('0.1.0');

  const search = page.getByRole('searchbox', { name: 'Find an export' });
  await search.fill('zzzz-no-export');
  await expect(page.locator('.api-no-results')).toBeVisible();
  await expect(page.locator('[data-api-entry]:visible')).toHaveCount(0);
  await search.fill('retry/with-timeout');
  await expect(page.locator('[data-api-entry]:visible')).toHaveCount(1);
  const entry = page.locator('[data-api-entry]:visible');
  await expect(entry.locator('.api-signature var')).toHaveText(['ms', 'thunk']);
  await entry.getByText('View source definition', { exact: true }).click();
  await expect(entry.locator('.api-definition pre')).toBeVisible();
  expect(await entry.locator('.api-definition span[style]').count()).toBeGreaterThan(5);
  await entry.getByRole('link', { name: 'Source :217' }).click();
  await expect(page).toHaveURL(/source\/package.sema#L217$/);
  await expect(page.locator('#L217')).toContainText('(define (retry/with-timeout ms thunk)');

  const markdown = await request.get(`${release}/api.md`);
  expect(markdown.headers()['content-type']).toContain('text/markdown');
  expect(await markdown.text()).toContain('(retry/with-timeout ms thunk)');
  const external: string[] = [];
  page.on('request', req => { if (req.url() !== `http://localhost:3111${release}/api/print`) external.push(req.url()); });
  await page.goto(`${release}/api/print`);
  await expect(page.locator('article')).toHaveCount(9);
  expect(external).toEqual([]);
  await page.screenshot({ path: '../.impeccable/review/api-print.png' });
  await page.goto(`${release}/api`);
  await page.locator('.api-definition summary').first().click();
  await page.screenshot({ path: '../.impeccable/review/api-desktop.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: '../.impeccable/review/api-mobile.png' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await expect(search).toBeVisible();
  await search.fill('retry/with-timeout');
  await expect(page.locator('[data-api-entry]:visible')).toHaveCount(1);
  await page.screenshot({ path: '../.impeccable/review/api-mobile-filtered.png', fullPage: true });
});

test('API content and source disclosures work without JavaScript', async ({ browser, request, apiToken }) => {
  const name = `api-no-js-${Date.now()}`;
  const release = `/packages/${name}/0.1.0`;
  const archive = await readFile(`${__dirname}/../../tests/fixtures/sema-retry-0.1.0.tar.gz`);
  const published = await request.put(`/api/v1${release}`, {
    headers: { authorization: `Bearer ${apiToken}` },
    multipart: {
      metadata: '{}',
      tarball: { name: 'pkg.tar.gz', mimeType: 'application/gzip', buffer: archive },
    },
  });
  expect(published.status()).toBe(201);
  const context = await browser.newContext({ javaScriptEnabled: false });
  try {
    const page = await context.newPage();
    await page.goto(`http://localhost:3111${release}/api`);
    await expect(page.locator('[data-api-entry]')).toHaveCount(9);
    await expect(page.locator('.api-filter')).toBeHidden();
    await page.locator('.api-definition summary').first().click();
    await expect(page.locator('.api-definition pre').first()).toBeVisible();
    await expect(page.getByRole('link', { name: 'Markdown', exact: true })).toBeVisible();
  } finally {
    await context.close();
  }
});
