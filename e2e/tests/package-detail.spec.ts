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
