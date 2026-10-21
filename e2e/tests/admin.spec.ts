import { test, expect } from '@playwright/test';
import { execFileSync, execSync } from 'child_process';
import * as path from 'path';
import * as api from './helpers/api';

const BASE = 'http://localhost:3111';
const DB_PATH = path.resolve(__dirname, '..', 'e2e-test.db');

// ── DB helpers ──

function makeAdmin(username: string) {
  execSync(
    `sqlite3 "${DB_PATH}" "UPDATE users SET is_admin = 1 WHERE username = '${username}'"`,
  );
}

function getUserId(username: string): number {
  const result = execSync(
    `sqlite3 "${DB_PATH}" "SELECT id FROM users WHERE username = '${username}'"`,
    { encoding: 'utf-8' },
  );
  return parseInt(result.trim(), 10);
}

// ── Unique name helpers ──

let counter = 0;
function unique(prefix: string): string {
  counter++;
  return `${prefix}${counter}${Date.now()}`;
}

test.describe('Admin Panel', () => {
  // ────────────────────────────────────────────
  // 1. Admin page loads for admin user
  // ────────────────────────────────────────────
  test('admin page loads for admin user', async ({ page, request }) => {
    const username = unique('adm-load');
    const session = await api.register(request, username, `${username}@test.com`);
    makeAdmin(username);
    await api.setSession(page.context(), session);

    await page.goto('/admin');
    await expect(page).toHaveURL(/\/admin/);
    // Wait for stats grid to be visible — proves dashboard tab loaded with data
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });
    await expect(page.getByTestId('stats-grid')).toBeVisible();
  });

  // ────────────────────────────────────────────
  // 2. Admin page returns 403 for non-admin
  // ────────────────────────────────────────────
  test('admin page returns 403 for non-admin', async ({ page, request }) => {
    const username = unique('adm-nonadm');
    const session = await api.register(request, username, `${username}@test.com`);
    await api.setSession(page.context(), session);

    const response = await page.goto('/admin');
    expect(response?.status()).toBe(403);
    const body = await page.textContent('body');
    expect(body).toContain('Admin access required');
  });

  // ────────────────────────────────────────────
  // 3. Admin page redirects to login when unauthenticated
  // ────────────────────────────────────────────
  test('admin page redirects to login when unauthenticated', async ({
    page,
  }) => {
    await page.goto('/admin');
    await page.waitForURL(/\/login/, { timeout: 10_000 });
    expect(page.url()).toContain('/login');
  });

  // ────────────────────────────────────────────
  // 4. Dashboard shows stats
  // ────────────────────────────────────────────
  test('dashboard shows stats', async ({ page, request }) => {
    const adminName = unique('adm-stats');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Create some users and packages to show in stats
    const user1 = unique('statsuser1');
    await api.register(request, user1, `${user1}@test.com`);
    const user2 = unique('statsuser2');
    await api.register(request, user2, `${user2}@test.com`);

    const token = await api.createToken(request, adminSession, 'stats-tok');
    await api.publishPackage(request, token, unique('statspkg'), '0.1.0');

    await api.setSession(page.context(), adminSession);

    // Navigate and wait for stats to render
    const statsResponse = page.waitForResponse(res => new URL(res.url()).pathname === '/api/v1/admin/stats');
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Stat values should be visible and numeric
    const totalUsers = page.getByTestId('stat-users');
    await expect(totalUsers).toBeVisible({ timeout: 10000 });
    // Stats are cached for 30 seconds; verify the displayed response rather
    // than assuming the cache already includes this test's new records.
    const stats = await (await statsResponse).json();
    await expect(totalUsers).toHaveText(String(stats.total_users));
    await expect(page.getByTestId('stat-packages')).toHaveText(String(stats.total_packages));
  });

  // ────────────────────────────────────────────
  // 5. User search works
  // ────────────────────────────────────────────
  test('user search works', async ({ page, request }) => {
    const adminName = unique('adm-search');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    const alice = unique('searchable-alice');
    const bob = unique('searchable-bob');
    await api.register(request, alice, `${alice}@test.com`);
    await api.register(request, bob, `${bob}@test.com`);

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Switch to Users tab and wait for table rows to appear
    await page.getByTestId('nav-users').click();
    await page.getByTestId('admin-row').first().waitFor({ timeout: 10000 });

    // Type in search box
    const searchInput = page.getByTestId('users-search');
    await expect(searchInput).toBeVisible({ timeout: 10000 });

    await searchInput.fill(alice);
    // Wait for debounce + API response + re-render
    await page.waitForTimeout(1000);
    const usersTable = page.getByTestId('users-table');
    await usersTable
      .getByTestId('admin-row')
      .filter({ hasText: alice })
      .first()
      .waitFor({ timeout: 10000 });

    // Alice should be visible, bob should not
    const visibleRows = usersTable.getByTestId('admin-row');
    await expect(visibleRows.first()).toBeVisible();
    await expect(
      usersTable.getByTestId('admin-row').filter({ hasText: alice }).first(),
    ).toBeVisible();
    await expect(
      usersTable.getByTestId('admin-row').filter({ hasText: bob }),
    ).toHaveCount(0);
  });

  // ────────────────────────────────────────────
  // 6. Ban user flow
  // ────────────────────────────────────────────
  test('ban user flow', async ({ page, request }) => {
    const adminName = unique('adm-ban');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    const bannable = unique('bannable');
    await api.register(request, bannable, `${bannable}@test.com`);

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Navigate to Users tab and wait for table to load
    await page.getByTestId('nav-users').click();
    await page.getByTestId('admin-row').first().waitFor({ timeout: 10000 });

    // Search for the bannable user to narrow down the list
    const searchInput = page.getByTestId('users-search');
    await searchInput.fill(bannable);
    await page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: bannable })
      .first()
      .waitFor({ timeout: 10000 });

    // Find the row containing the bannable user and click Ban
    const userRow = page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: bannable });
    await expect(userRow).toBeVisible();

    // Click Ban — this opens the confirm dialog
    await userRow.getByTestId('action-ban').click();

    // Confirm dialog should appear
    await expect(page.getByTestId('confirm-dialog')).toBeVisible({ timeout: 10000 });
    await expect(page.getByTestId('confirm-title')).toHaveText('Ban User');

    const reason = 'Repeated spam uploads after a warning';
    await page.getByLabel('Reason (optional)').fill(reason);

    // Click the confirm button and wait for toast
    await page.getByTestId('confirm-accept').click();

    // Toast should show success
    await expect(page.getByTestId('toast-success')).toBeVisible({ timeout: 10000 });
    const toastText = await page.getByTestId('toast-success').textContent();
    expect(toastText).toContain('Banned');

    // After the action the user should show as Banned in the table
    const statusBadge = page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: bannable })
      .getByTestId('status-banned');
    await expect(statusBadge).toBeVisible({ timeout: 10000 });
    await page.getByTestId('nav-audit').click();
    await page.getByTestId('audit-search').fill(bannable);
    const entry = page.getByRole('list', { name: 'Audit events' }).getByTestId('audit-entry')
      .filter({ hasText: bannable })
      .filter({ has: page.getByText('Banned user', { exact: true }) });
    await expect(entry).toContainText(bannable);
    await expect(entry).toContainText(reason);
  });

  // ────────────────────────────────────────────
  // 7. Package management — yank all
  // ────────────────────────────────────────────
  test('package management — yank all', async ({ page, request }) => {
    const adminName = unique('adm-yank');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    const token = await api.createToken(request, adminSession, 'yank-tok');
    const pkgName = unique('yankpkg');
    await api.publishPackage(request, token, pkgName, '0.1.0');
    await api.publishPackage(request, token, pkgName, '0.2.0');

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Navigate to Packages tab and wait for table rows
    await page.getByTestId('nav-packages').click();
    await page.getByTestId('admin-row').first().waitFor({ timeout: 10000 });

    // Find the package row
    const pkgRow = page
      .getByTestId('packages-table')
      .getByTestId('admin-row')
      .filter({ hasText: pkgName });
    await expect(pkgRow).toBeVisible();

    // Click Yank All — opens confirm dialog
    await pkgRow.getByTestId('action-yank').click();

    await expect(page.getByTestId('confirm-dialog')).toBeVisible({ timeout: 10000 });
    await expect(page.getByTestId('confirm-title')).toHaveText(
      'Yank All Versions',
    );

    // Confirm and wait for toast
    await page.getByTestId('confirm-accept').click();

    // Toast should show success
    await expect(page.getByTestId('toast-success')).toBeVisible({ timeout: 10000 });
    const toastText = await page.getByTestId('toast-success').textContent();
    expect(toastText).toContain('Yanked');
  });

  // ────────────────────────────────────────────
  // 8. Package management — remove
  // ────────────────────────────────────────────
  test('package management — remove', async ({ page, request }) => {
    const adminName = unique('adm-remove');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    const token = await api.createToken(request, adminSession, 'rm-tok');
    const pkgName = unique('removepkg');
    await api.publishPackage(request, token, pkgName, '1.0.0');

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Navigate to Packages tab and wait for table rows
    await page.getByTestId('nav-packages').click();
    await page.getByTestId('admin-row').first().waitFor({ timeout: 10000 });

    // Find the package row
    const pkgRow = page
      .getByTestId('packages-table')
      .getByTestId('admin-row')
      .filter({ hasText: pkgName });
    await expect(pkgRow).toBeVisible();

    // Click Remove — opens confirm dialog
    await pkgRow.getByTestId('action-remove').click();

    await expect(page.getByTestId('confirm-dialog')).toBeVisible({ timeout: 10000 });
    await expect(page.getByTestId('confirm-title')).toHaveText('Remove Package');

    // Confirm and wait for toast
    await page.getByTestId('confirm-accept').click();

    // Toast should show success
    await expect(page.getByTestId('toast-success')).toBeVisible({ timeout: 10000 });
    const toastText = await page.getByTestId('toast-success').textContent();
    expect(toastText).toContain('Removed');

    // Package should no longer appear in the list after removal
    await expect(
      page
        .getByTestId('packages-table')
        .getByTestId('admin-row')
        .filter({ hasText: pkgName }),
    ).toHaveCount(0, { timeout: 10000 });
  });

  // ────────────────────────────────────────────
  // 9. Audit log displays entries
  // ────────────────────────────────────────────
  test('audit log displays entries', async ({ page, request }) => {
    const adminName = unique('adm-audit');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Publish a package so there is audit activity
    const token = await api.createToken(request, adminSession, 'audit-tok');
    const pkgName = unique('auditpkg');
    await api.publishPackage(request, token, pkgName, '1.0.0');

    // Ban a user to generate an audit entry
    const victim = unique('auditvictim');
    await api.register(request, victim, `${victim}@test.com`);
    const victimId = getUserId(victim);
    await request.post(`${BASE}/api/v1/admin/users/${victimId}/ban`, {
      data: { reason: 'Reason with <em>literal markup</em>' },
      headers: { cookie: `session=${adminSession}` },
    });

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    const recent = page.getByRole('list', { name: 'Recent activity' });
    await expect(recent).toContainText(pkgName);
    await expect(recent).toContainText('Version: 1.0.0');

    // Switch to Audit Log tab
    await page.getByTestId('nav-audit').click();

    // Audit entries should be visible — use the page title as proof tab switched
    await expect(
      page.getByTestId('page-title').filter({ hasText: 'Audit Log' }),
    ).toBeVisible({ timeout: 10000 });

    // Wait for at least one entry to render
    const entries = page.getByTestId('audit-entry').filter({ visible: true });
    await expect(entries.first()).toBeVisible({ timeout: 10000 });
    const count = await entries.count();
    expect(count).toBeGreaterThanOrEqual(1);

    const events = page.getByRole('list', { name: 'Audit events' });
    await expect(events).toContainText(victim);
    await expect(events).toContainText('Reason with <em>literal markup</em>');
    await expect(events.locator('em')).toHaveCount(0);
    await expect(events.locator('time').first()).toHaveAttribute('datetime', /Z$/);
    await expect(events.locator('time').first()).toContainText('UTC');
    await page.getByLabel('Event type').selectOption('ban_user');
    await expect(events).toContainText('Banned user');
    await expect(events).not.toContainText('Published');
    await page.getByLabel('Event type').selectOption('publish');
    await page.getByLabel('Search events').fill(pkgName);
    await expect(events.getByTestId('audit-entry')).toHaveCount(1);
    await expect(events).toContainText(pkgName);
    await expect(events).toContainText('Version: 1.0.0');
    await page.getByLabel('Search events').fill('nothing-matches-this-event');
    await expect(events).toContainText('No audit entries found.');

    // Audit filters must not hide the dashboard's recent activity.
    await page.getByTestId('nav-dashboard').click();
    await expect(recent).toContainText(victim);
    await expect(recent).toContainText(pkgName);
  });

  test('audit pages load on demand, paginate, and recover from errors', async ({ page, request }) => {
    const username = unique('adm-pages');
    const session = await api.register(request, username, `${username}@test.com`);
    makeAdmin(username);
    const fixture = unique('audit-pages');
    execFileSync('sqlite3', [DB_PATH, `
      WITH RECURSIVE seq(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM seq WHERE n<55)
      INSERT INTO audit_log(actor, action, target_type, target_name, detail, created_at)
      SELECT '${fixture}', 'publish', 'package', 'page-event-' || n, '1.0.0', '2026-01-01 12:00:00' FROM seq;
    `]);
    const auditRequests: string[] = [];
    page.on('request', req => {
      if (req.url().includes('/api/v1/admin/audit?')) auditRequests.push(req.url());
    });
    await api.setSession(page.context(), session);
    await page.goto('/admin');
    await expect(page.getByRole('list', { name: 'Recent activity' }).getByTestId('audit-entry')).toHaveCount(8);
    expect(auditRequests).toHaveLength(1);
    expect(new URL(auditRequests[0]).searchParams.get('per_page')).toBe('8');
    const count = await page.getByTestId('nav-audit').locator('.sidebar-count').textContent();

    await page.getByTestId('nav-audit').click();
    await page.getByLabel('Search events').fill(fixture);
    const events = page.getByRole('list', { name: 'Audit events' });
    await expect(events).toContainText('page-event-55');
    await expect(events.getByTestId('audit-entry')).toHaveCount(50);
    await expect(page.getByTestId('nav-audit').locator('.sidebar-count')).toHaveText(count!);
    const pages = page.getByRole('navigation', { name: 'Audit pages' });
    await expect(pages).toContainText('1 / 2');
    await pages.getByRole('button', { name: 'Next' }).click();
    await expect(events.getByTestId('audit-entry')).toHaveCount(5);
    await expect(events).toContainText('page-event-1');
    await expect(pages).toContainText('2 / 2');
    await expect(pages.getByRole('button', { name: 'Next' })).toBeDisabled();
    await pages.getByRole('button', { name: 'Prev' }).click();
    await expect(events.getByTestId('audit-entry')).toHaveCount(50);
    await expect(events).toContainText('page-event-55');

    let fail = true;
    await page.route('**/api/v1/admin/audit?*', route => {
      if (fail) {
        fail = false;
        return route.fulfill({ status: 500, contentType: 'application/json', body: '{"error":"test failure"}' });
      }
      return route.continue();
    });
    await page.getByLabel('Search events').fill('no-match-after-retry');
    await expect(page.getByRole('status')).toHaveText('Could not load audit events. Try again.');
    await expect(events).toBeHidden();
    await page.getByRole('button', { name: 'Try again' }).click();
    await expect(events).toContainText('No audit entries found.');
  });

  test('audit filters match stored actions and sparse entries render safely on mobile', async ({ page, request }) => {
    const username = unique('adm-sparse');
    const session = await api.register(request, username, `${username}@test.com`);
    makeAdmin(username);
    const fixture = unique('audit-fixture');
    const longName = 'long-package-name-'.repeat(12);
    execFileSync('sqlite3', [DB_PATH, `
      INSERT INTO audit_log (actor, action, target_type, target_name, detail, created_at) VALUES
      ('${fixture}', 'webhook_sync', 'package', '${longName}', 'Repository: owner/repo · Tag: v1.2.3 · Version: 1.2.3', '2026-10-20 12:00:00'),
      ('${fixture}', 'unban_user', 'user', '${username}', NULL, '2026-10-20 12:00:00'),
      ('${fixture}', 'future_action', NULL, NULL, NULL, '2026-10-20 12:00:00');
    `]);
    await api.setSession(page.context(), session);
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto('/admin');
    await page.getByTestId('nav-audit').click();
    await page.getByLabel('Search events').fill(fixture);
    const events = page.getByRole('list', { name: 'Audit events' });
    await expect(events.getByTestId('audit-entry')).toHaveCount(3);
    await expect(events).toContainText('future action');
    await expect(events).not.toContainText('undefined');
    await expect(events).not.toContainText('null');
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBeLessThanOrEqual(390);

    await page.getByLabel('Event type').selectOption('webhook_sync');
    await expect(events.getByTestId('audit-entry')).toHaveCount(1);
    await expect(events).toContainText(longName);
    await expect(events).toContainText('Repository: owner/repo · Tag: v1.2.3 · Version: 1.2.3');
    await page.getByLabel('Event type').selectOption('unban_user');
    await expect(events.getByTestId('audit-entry')).toHaveCount(1);
    await expect(events).toContainText(username);
    await expect(events).toContainText('Unbanned user');
    await expect(page.getByLabel('Event type').locator('option[value="admin"], option[value="ban"], option[value="webhook"]')).toHaveCount(0);
  });

  // ────────────────────────────────────────────
  // 10. Reports — submit and view
  // ────────────────────────────────────────────
  test('reports — submit and view', async ({ page, request }) => {
    const adminName = unique('adm-report');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Regular user submits a report
    const reporter = unique('reporter');
    const reporterSession = await api.register(
      request,
      reporter,
      `${reporter}@test.com`,
    );

    const targetPkg = unique('reportedpkg');
    const token = await api.createToken(request, reporterSession);
    await api.publishPackage(request, token, targetPkg, '1.0.0');
    const reportRes = await request.post(`${BASE}/api/v1/reports`, {
      data: {
        target_type: 'package',
        target_name: targetPkg,
        report_type: 'spam',
        reason: 'This package is spam',
      },
      headers: { cookie: `session=${reporterSession}` },
    });
    expect(reportRes.ok()).toBeTruthy();

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Switch to Reports tab and wait for report content to appear
    await page.getByTestId('nav-reports').click();

    // The report should be visible
    const reportsSection = page.getByTestId('reports-section');
    await expect(
      reportsSection.getByTestId('report-card').filter({ hasText: targetPkg }),
    ).toBeVisible({ timeout: 10000 });
    const sectionText = await reportsSection.textContent();
    expect(sectionText).toContain('This package is spam');
    expect(sectionText).toContain(reporter);

    await reportsSection.getByTestId('report-card').filter({ hasText: targetPkg }).getByTestId('action-act').click();
    await page.getByTestId('confirm-accept').click();
    await expect(page.getByTestId('toast-success')).toContainText(`Removed ${targetPkg}`);
    await expect(reportsSection.getByTestId('report-card').filter({ hasText: targetPkg })).toHaveCount(0);
    await page.getByTestId('nav-audit').click();
    await page.getByLabel('Search events').fill(targetPkg);
    const events = page.getByRole('list', { name: 'Audit events' });
    await expect(events).toContainText('Removed 1 version');
    await expect(events).toContainText('Actioned report');
    await expect(events).toContainText('This package is spam');
  });

  // ────────────────────────────────────────────
  // 11. Reports — dismiss
  // ────────────────────────────────────────────
  test('reports — dismiss', async ({ page, request }) => {
    const adminName = unique('adm-dismiss');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Submit a report
    const reporter = unique('dismissrep');
    const reporterSession = await api.register(
      request,
      reporter,
      `${reporter}@test.com`,
    );

    const targetPkg = unique('dismissedpkg');
    await request.post(`${BASE}/api/v1/reports`, {
      data: {
        target_type: 'package',
        target_name: targetPkg,
        report_type: 'abuse',
        reason: 'Abusive content in readme',
      },
      headers: { cookie: `session=${reporterSession}` },
    });

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Switch to Reports tab and wait for report content to appear
    await page.getByTestId('nav-reports').click();

    // Find the report and wait for it to be visible
    const reportsSection = page.getByTestId('reports-section');
    await expect(
      reportsSection.getByTestId('report-card').filter({ hasText: targetPkg }),
    ).toBeVisible({ timeout: 10000 });

    // The Dismiss button is next to the report
    await page
      .getByTestId('reports-section')
      .getByTestId('action-dismiss')
      .first()
      .click();

    // Toast should show success
    await expect(page.getByTestId('toast-success')).toBeVisible({ timeout: 10000 });
    const toastText = await page.getByTestId('toast-success').textContent();
    expect(toastText).toContain('Dismissed');

    // After dismiss, report should be gone from open list
    // Wait a beat for Alpine to re-render
    await page.waitForTimeout(500);
    const remainingText = await page
      .getByTestId('reports-section')
      .textContent();
    expect(remainingText).not.toContain(targetPkg);
  });

  // ────────────────────────────────────────────
  // 12. Admin nav link visible only for admins
  // ────────────────────────────────────────────
  test('admin nav link visible only for admins', async ({ page, request }) => {
    // Non-admin user should not see the Admin link
    const normalUser = unique('navnormal');
    const normalSession = await api.register(
      request,
      normalUser,
      `${normalUser}@test.com`,
    );
    await api.setSession(page.context(), normalSession);
    await page.goto('/');
    await expect(page.getByTestId('nav-admin')).toHaveCount(0);

    // Admin user should see the Admin link
    const adminUser = unique('navadmin');
    const adminSession = await api.register(
      request,
      adminUser,
      `${adminUser}@test.com`,
    );
    makeAdmin(adminUser);

    // Need a new context to set the admin session
    const adminContext = await page.context().browser()!.newContext();
    await api.setSession(adminContext, adminSession);
    const adminPage = await adminContext.newPage();
    await adminPage.goto('/');
    await expect(adminPage.getByTestId('nav-admin')).toBeVisible();
    await adminContext.close();
  });

  // ────────────────────────────────────────────
  // 13. Unban user flow
  // ────────────────────────────────────────────
  test('unban user flow', async ({ page, request }) => {
    const adminName = unique('adm-unban');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Register target user and ban them via API
    const target = unique('unbannable');
    await api.register(request, target, `${target}@test.com`);
    const targetId = getUserId(target);
    await request.post(`${BASE}/api/v1/admin/users/${targetId}/ban`, {
      headers: { cookie: `session=${adminSession}` },
    });

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Navigate to Users tab
    await page.getByTestId('nav-users').click();
    await page.getByTestId('admin-row').first().waitFor({ timeout: 10000 });

    // Search for the target user
    const searchInput = page.getByTestId('users-search');
    await searchInput.fill(target);
    await page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: target })
      .first()
      .waitFor({ timeout: 10000 });

    // Verify the user shows "Banned" badge
    const userRow = page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: target });
    await expect(userRow).toBeVisible();
    await expect(userRow.getByTestId('status-banned')).toBeVisible({ timeout: 10000 });

    // Click "Unban" — opens confirm dialog
    await userRow.getByTestId('action-unban').click();

    // Confirm dialog should appear
    await expect(page.getByTestId('confirm-dialog')).toBeVisible({ timeout: 10000 });

    // Click the confirm button inside the dialog
    const confirmBtn = page.getByTestId('confirm-accept');
    await confirmBtn.click();

    // Toast should show success
    await expect(page.getByTestId('toast-success')).toBeVisible({ timeout: 10000 });

    // User should no longer show "Banned" badge
    await page.waitForTimeout(500);
    const updatedRow = page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: target });
    await expect(updatedRow.getByTestId('status-banned')).toHaveCount(0, { timeout: 10000 });
  });

  // ────────────────────────────────────────────
  // 14. Transfer ownership flow
  // ────────────────────────────────────────────
  test('transfer ownership flow', async ({ page, request }) => {
    const adminName = unique('adm-transfer');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Register owner and new owner
    const owner = unique('xferowner');
    const ownerSession = await api.register(
      request,
      owner,
      `${owner}@test.com`,
    );
    const newowner = unique('xfernew');
    await api.register(request, newowner, `${newowner}@test.com`);

    // Owner publishes a package
    const ownerToken = await api.createToken(request, ownerSession, 'xfer-tok');
    const pkgName = unique('xferpkg');
    await api.publishPackage(request, ownerToken, pkgName, '1.0.0');

    // Transfer ownership via admin API
    const transferRes = await request.post(
      `${BASE}/api/v1/admin/packages/${pkgName}/transfer`,
      {
        data: { to_username: newowner },
        headers: { cookie: `session=${adminSession}` },
      },
    );
    expect(transferRes.ok()).toBeTruthy();

    // Verify the transfer in the browser on the package detail page
    await api.setSession(page.context(), adminSession);
    await page.goto(`/packages/${pkgName}`);

    // The package detail page should show the new owner
    await expect(page.locator(`text=${newowner}`)).toBeVisible({ timeout: 10000 });
  });

  // ────────────────────────────────────────────
  // 15. Revoke tokens from user drawer
  // ────────────────────────────────────────────
  test('revoke tokens from user drawer', async ({ page, request }) => {
    const adminName = unique('adm-revtok');
    const adminSession = await api.register(
      request,
      adminName,
      `${adminName}@test.com`,
    );
    makeAdmin(adminName);

    // Register target user and create 2 tokens
    const target = unique('tokentarget');
    const targetSession = await api.register(
      request,
      target,
      `${target}@test.com`,
    );
    await api.createToken(request, targetSession, 'tok-a');
    await api.createToken(request, targetSession, 'tok-b');

    await api.setSession(page.context(), adminSession);
    await page.goto('/admin');
    await expect(page.getByTestId('stats-grid')).toBeVisible({ timeout: 15000 });

    // Navigate to Users tab
    await page.getByTestId('nav-users').click();
    await page.getByTestId('admin-row').first().waitFor({ timeout: 10000 });

    // Search for the target user
    const searchInput = page.getByTestId('users-search');
    await searchInput.fill(target);
    await page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: target })
      .first()
      .waitFor({ timeout: 10000 });

    // Click "View" on the target user to open the drawer
    const userRow = page
      .getByTestId('users-table')
      .getByTestId('admin-row')
      .filter({ hasText: target });
    await expect(userRow).toBeVisible();
    await userRow.getByTestId('action-view').click();

    // Wait for drawer to open and click "Revoke All Tokens"
    await page.getByTestId('revoke-all-tokens').click();

    // Toast should show success
    await expect(page.getByTestId('toast-success')).toBeVisible({ timeout: 10000 });
  });
});

test('sidebar badges stay hidden for empty counts with lazy audit loading', async ({ page, request }) => {
  const username = unique('badges');
  const session = await api.register(request, username, `${username}@test.com`);
  makeAdmin(username);
  await api.setSession(page.context(), session);
  let count: number | null | undefined = 0;
  await page.route('**/api/v1/admin/stats', route => route.fulfill({ json: {
    total_users: count, total_packages: count, open_reports: count, banned_users: 0,
  } }));
  await page.route('**/api/v1/admin/audit?*', route => route.fulfill({ json: { entries: [], total: count } }));
  const errors: string[] = [];
  page.on('pageerror', error => errors.push(error.message));
  for (const value of [0, null, undefined, 1234, 0]) {
    count = value;
    await Promise.all([
      page.waitForResponse(response => response.url().includes('/api/v1/admin/audit?')),
      page.goto('/admin'),
    ]);
    for (const tab of ['audit', 'users', 'packages', 'reports']) {
      const badge = page.getByTestId(`nav-${tab}`).locator('.sidebar-count');
      if (value) await expect(badge).toBeVisible();
      else await expect(badge).toBeHidden();
    }
    if (value) await expect(page.getByTestId('nav-audit')).toContainText('1,234');
  }
  expect(errors).toEqual([]);
});
