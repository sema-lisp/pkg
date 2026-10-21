import { readFile } from 'node:fs/promises';
import { execFileSync } from 'node:child_process';
import { test, expect } from './helpers/fixtures';

test('published API reference, source, Markdown, offline print, and mobile layout', async ({ page, request, apiToken }) => {
  const name = `api-reference-${Date.now()}`;
  const release = `/packages/${name}/0.1.0`;
  const archive = await readFile(`${__dirname}/../../tests/fixtures/sema-retry-0.1.0.tar.gz`);
  const publish = await request.put(`/api/v1${release}`, {
    headers: { authorization: `Bearer ${apiToken}` },
    multipart: {
      metadata: JSON.stringify({ description: 'Retry with exponential backoff, circuit breaker, and rate limiting', repository_url: 'https://github.com/sema-lisp/packages' }),
      tarball: { name: 'sema-retry-0.1.0.tar.gz', mimeType: 'application/gzip', buffer: archive },
    },
  });
  expect(publish.status()).toBe(201);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(release);
  const packageHeader = await page.locator('.site-header').innerText();
  await expect(page.locator('.detail-sidebar').getByRole('link', { name: 'API reference', exact: true })).toBeVisible();
  const packageWidth = await page.locator('.page-content').evaluate(element => element.getBoundingClientRect().width);
  for (const width of [831, 390]) {
    await page.setViewportSize({ width, height: 1000 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    expect(await page.locator('.detail-sidebar').evaluate(element => element.getBoundingClientRect().right)).toBeLessThanOrEqual(width);
    await page.screenshot({ path: `../.impeccable/review/package-${width}.png` });
  }
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.locator('.detail-sidebar').getByRole('link', { name: 'API reference', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`${release}/api$`));
  expect(await page.locator('.site-header').innerText()).toBe(packageHeader);
  await expect(page.locator('[data-api-entry]')).toHaveCount(9);
  expect(await page.locator('.page-content').evaluate(element => element.getBoundingClientRect().width)).toBe(packageWidth);
  await expect(page.locator('.api-source-file')).toHaveText('package.sema');
  await expect(page.getByRole('link', { name: 'Print', exact: true })).toBeVisible();
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
  expect(await entry.locator('.api-definition code').textContent()).toMatch(/^\(define \(retry\/with-timeout/);
  await entry.getByRole('link', { name: 'Source :217' }).click();
  await expect(page).toHaveURL(/source\/package.sema#L217$/);
  await expect(page.locator('#L217')).toContainText('(define (retry/with-timeout ms thunk)');
  expect(await page.locator('.page-content').evaluate(element => element.getBoundingClientRect().width)).toBe(packageWidth);
  await expect(page.locator('.api-source-file')).toHaveAttribute('aria-current', 'page');
  await expect(page.locator('[data-api-index]')).toHaveCount(9);
  const sourceExportLinks = await page.locator('[data-api-index] a').evaluateAll(elements => elements.map(element => element.getAttribute('href')));
  await page.goto(`${release}/source/package.sema`);
  await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
  const hashCopy = page.getByRole('button', { name: 'Copy full source SHA-256' });
  const digest = await hashCopy.getAttribute('data-copy-value');
  expect(digest).toMatch(/^[a-f0-9]{64}$/);
  await expect(hashCopy.locator('code')).toHaveText(digest!.slice(0, 12));
  await hashCopy.click();
  await expect(page.locator('.api-copy-status')).toHaveText('Full source SHA-256 copied.');
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(digest);
  await expect(hashCopy.locator('code')).toHaveText(digest!.slice(0, 12));
  await page.screenshot({ path: '../.impeccable/review/refined-source-desktop.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: '../.impeccable/review/refined-source-mobile.png' });
  await page.getByText('Browse exports', { exact: false }).click();
  await page.getByRole('searchbox', { name: 'Find an export' }).fill('retry/with-timeout');
  await expect(page.locator('[data-api-index]:visible')).toHaveCount(1);
  await page.locator('[data-api-index]:visible a').click();
  await expect(page).toHaveURL(new RegExp(`${release}/api#fn-`));
  await expect(page.locator('.api-entry:target')).toContainText('retry/with-timeout');
  expect(await page.locator('[data-api-index] a').evaluateAll(elements => elements.map(element => element.getAttribute('href')))).toEqual(sourceExportLinks);
  await page.setViewportSize({ width: 1440, height: 1000 });

  const markdown = await request.get(`${release}/api.md`);
  expect(markdown.headers()['content-type']).toContain('text/markdown');
  expect(await markdown.text()).toContain('(retry/with-timeout ms thunk)');
  const external: string[] = [];
  page.on('request', req => { if (req.url() !== `http://localhost:3111${release}/api/print`) external.push(req.url()); });
  await page.goto(`${release}/api/print`);
  await expect(page.locator('article')).toHaveCount(9);
  expect(external).toEqual([]);
  await page.screenshot({ path: '../.impeccable/review/refined-print.png' });
  await page.goto(`${release}/api`);
  await page.locator('.api-definition summary').first().click();
  await page.screenshot({ path: '../.impeccable/review/refined-api-desktop.png' });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: '../.impeccable/review/refined-api-mobile.png' });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.getByText('Browse exports', { exact: false }).click();
  await expect(search).toBeVisible();
  await search.fill('retry/with-timeout');
  await expect(page.locator('[data-api-entry]:visible')).toHaveCount(1);
  await page.screenshot({ path: '../.impeccable/review/refined-api-mobile-filtered.png', fullPage: true });
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
    await page.locator('.api-source-file').click();
    await expect(page.locator('.api-hash-static')).toBeVisible();
    await expect(page.locator('.api-hash-copy')).toBeHidden();
    await page.getByText('File details', { exact: true }).click();
    await expect(page.locator('.api-file-details code')).toHaveText(/^[a-f0-9]{64}$/);
  } finally {
    await context.close();
  }
});

test('public page shells share a maximum width', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  for (const route of ['/', '/search', '/login', '/packages/missing-width-audit', '/missing-width-audit']) {
    await page.goto(route);
    const shell = page.locator('.page-content');
    expect(await shell.evaluate(element => element.getBoundingClientRect().width), route).toBe(1024);
  }
});

test('nested entrypoint, helper import, and tests remain a source-backed reference', async ({ page, request, apiToken }) => {
  const name = `nested-api-${Date.now()}`;
  const release = `/packages/${name}/0.1.0`;
  const archive = execFileSync('tar', ['-czf', '-', '-C', `${__dirname}/../../tests/fixtures/nested-api-demo`, '.'], {
    env: { ...process.env, COPYFILE_DISABLE: '1' },
  });
  const published = await request.put(`/api/v1${release}`, {
    headers: { authorization: `Bearer ${apiToken}` },
    multipart: {
      metadata: JSON.stringify({ description: 'Local preview fixture with nested source, an imported alias, and a test suite' }),
      tarball: { name: 'nested-api-demo.tar.gz', mimeType: 'application/gzip', buffer: archive },
    },
  });
  expect(published.status()).toBe(201);
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto(`${release}/api`);
  await expect(page.locator('[data-api-entry]')).toHaveCount(3);
  await expect(page.locator('.api-source-file')).toHaveText('src/api.sema');
  await expect(page.locator('[data-api-index]').filter({ hasText: 'demo/triple' })).toContainText('unresolved');
  const alias = page.locator('[data-api-entry]').filter({ has: page.getByRole('heading', { name: 'demo/triple' }) });
  await expect(alias.locator('.api-signature')).toHaveCount(0);
  await expect(alias).toContainText('no single direct function declaration');
  await page.screenshot({ path: '../.impeccable/review/nested-api-desktop.png', fullPage: true });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: '../.impeccable/review/nested-api-mobile.png', fullPage: true });
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.locator('.api-source-file').click();
  await expect(page).toHaveURL(/source\/src\/api\.sema$/);
  await expect(page.locator('.api-source-code')).toContainText('(import "./internal/scale.sema")');
  await page.screenshot({ path: '../.impeccable/review/nested-source-desktop.png', fullPage: true });
  await page.getByRole('navigation', { name: 'Exports' }).getByRole('link', { name: 'demo/clamp', exact: true }).click();
  await expect(page).toHaveURL(new RegExp(`${release}/api#fn-`));
  const markdown = await request.get(`${release}/api.md`);
  expect(await markdown.text()).toContain('/source/src/api.sema#L');
  await page.goto(`${release}/api/print`);
  await expect(page.locator('article')).toHaveCount(3);
});
