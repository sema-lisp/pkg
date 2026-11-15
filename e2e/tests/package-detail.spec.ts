import { test, expect } from './helpers/fixtures';
import { publishPackage, yankVersion } from './helpers/api';

test.describe('Package detail page', () => {
  test('readme tab is active by default', async ({ authedPage, request, apiToken }) => {
    const name = `detail-tabs-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await expect(authedPage.getByTestId('tab-readme')).toHaveAttribute('aria-selected', 'true');
    await expect(authedPage.getByTestId('tab-versions')).toHaveAttribute('aria-selected', 'false');
  });

  test('switching to the versions tab reveals the versions table', async ({ authedPage, request, apiToken }) => {
    const name = `detail-versions-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await authedPage.getByTestId('tab-versions').click();

    await expect(authedPage.getByTestId('tab-versions')).toHaveAttribute('aria-selected', 'true');
    await expect(authedPage.getByTestId('versions-table')).toBeVisible();
  });

  test('switching to dependencies tab works', async ({ authedPage, request, apiToken }) => {
    const name = `detail-deps-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await authedPage.getByTestId('tab-deps').click();

    await expect(authedPage.getByTestId('tab-deps')).toHaveAttribute('aria-selected', 'true');
    await expect(authedPage.locator('sema-tab-panel[value="deps"]')).toBeVisible();
    await expect(authedPage.getByTestId('versions-table')).toBeHidden();
  });

  test('install command shows correct package name', async ({ authedPage, request, apiToken }) => {
    const name = `detail-install-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await expect(authedPage.getByTestId('install-cmd')).toContainText(name);
  });

  test('sidebar shows owners', async ({ authedPage, request, apiToken }) => {
    const name = `detail-owners-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    const ownersList = authedPage.getByTestId('owners-list');
    await expect(ownersList).toBeVisible();
    // Should have at least one owner (the publisher)
    await expect(ownersList.getByTestId('owner-entry')).toHaveCount(1);
  });

  test('multiple versions sort newest first', async ({ authedPage, request, apiToken }) => {
    const name = `detail-sort-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');
    await publishPackage(request, apiToken, name, '2.0.0');

    await authedPage.goto(`/packages/${name}`);
    await authedPage.getByTestId('tab-versions').click();
    const table = authedPage.getByTestId('versions-table');
    await expect(table).toBeVisible();

    // Get all version cells and verify v2.0.0 appears before v1.0.0
    const tableText = await table.textContent();
    const idx200 = tableText!.indexOf('2.0.0');
    const idx100 = tableText!.indexOf('1.0.0');
    expect(idx200).toBeGreaterThanOrEqual(0);
    expect(idx100).toBeGreaterThanOrEqual(0);
    expect(idx200).toBeLessThan(idx100);
  });

  test('yanked versions show visual indicator', async ({ authedPage, request, apiToken }) => {
    const name = `detail-yanked-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');
    await yankVersion(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await authedPage.getByTestId('tab-versions').click();
    const table = authedPage.getByTestId('versions-table');
    await expect(table).toContainText('yanked');
  });
});

test.describe('Package detail — report form', () => {
  // The report form uses <sema-select> + <sema-textarea>; the textarea's inner
  // control is reached by piercing the open shadow root.
  test('submitting a report shows a confirmation', async ({ authedPage, request, apiToken }) => {
    const name = `detail-report-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await authedPage.getByTestId('report-toggle').click();
    await expect(authedPage.getByTestId('report-form')).toBeVisible();

    // report type is a <sema-select> defaulting to "spam" (valid); fill the reason.
    await expect(authedPage.getByTestId('report-type')).toBeVisible();
    await authedPage
      .getByTestId('report-reason')
      .locator('textarea')
      .fill('This package ships a crypto miner.');

    await authedPage.getByTestId('report-submit').click();
    await expect(authedPage.getByTestId('report-submitted')).toBeVisible();
  });

  test('reason textarea enforces the 2000-char maxlength', async ({ authedPage, request, apiToken }) => {
    const name = `detail-report-max-${Date.now()}`;
    await publishPackage(request, apiToken, name, '1.0.0');

    await authedPage.goto(`/packages/${name}`);
    await authedPage.getByTestId('report-toggle').click();
    await expect(
      authedPage.getByTestId('report-reason').locator('textarea'),
    ).toHaveAttribute('maxlength', '2000');
  });
});

test('large release histories stay paginated on desktop and mobile', async ({ authedPage, request, apiToken }) => {
  const { execFileSync } = await import('node:child_process');
  const path = await import('node:path');
  const name = `detail-pages-${Date.now()}`;
  await publishPackage(request, apiToken, name, '2.0.0');
  const db = path.resolve(__dirname, '..', 'e2e-test.db');
  execFileSync('sqlite3', [db, `
    WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<122)
    INSERT INTO package_versions (package_id, version, checksum_sha256, blob_key, size_bytes, published_at)
    SELECT (SELECT id FROM packages WHERE name='${name}'), '1.'||i||'.0', 'test', 'test', 1, '2026-10-21 00:00:00' FROM n;
  `]);
  for (const width of [1440, 390]) {
    await authedPage.setViewportSize({ width, height: 900 });
    await authedPage.goto(`/packages/${name}/1.7.0#versions`);
    await expect(authedPage.getByTestId('versions-table')).toBeVisible();
    await expect(authedPage.getByTestId('versions-table').locator('tbody tr')).toHaveCount(50);
    await expect(authedPage.getByTestId('tab-versions')).toContainText('123');
    await expect(authedPage.locator('.detail-sidebar .data-table tr')).toHaveCount(5);
    const pages = authedPage.getByRole('navigation', { name: 'Version pages' });
    await pages.getByRole('link', { name: 'Next', exact: true }).click();
    await expect(authedPage).toHaveURL(new RegExp(`/1\\.7\\.0\\?versions_page=2#versions$`));
    await expect(pages).toContainText('Page 2 of 3');
    await expect(authedPage.getByTestId('install-cmd')).toContainText(`${name}@1.7.0`);
    await pages.getByRole('link', { name: 'Next', exact: true }).click();
    await expect(authedPage.getByTestId('versions-table').locator('tbody tr')).toHaveCount(23);
    await expect(pages.getByRole('link', { name: 'Next', exact: true })).toHaveCount(0);
    await pages.getByRole('link', { name: 'Previous' }).click();
    await expect(pages).toContainText('Page 2 of 3');
    expect(await authedPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
});
