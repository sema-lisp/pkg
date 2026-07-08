import { test, expect } from './helpers/fixtures';

// The profile form uses <sema-input> web components. Playwright's fill() cannot
// target a custom-element host, so we reach the inner control by piercing the
// (open) shadow root with a chained `input` locator.

test.describe('Account — profile form', () => {
  test('readonly username shows the signed-in user and is not editable', async ({ authedPage }) => {
    await authedPage.goto('/account');

    const username = authedPage.getByTestId('profile-username');
    await expect(username).toBeVisible();

    const inner = username.locator('input');
    await expect(inner).toHaveValue(/.+/); // the seeded/registered username
    await expect(inner).toHaveJSProperty('readOnly', true);
  });

  test('editing email and saving shows a confirmation', async ({ authedPage }) => {
    await authedPage.goto('/account');

    const email = authedPage.getByTestId('profile-email').locator('input');
    const newEmail = `updated-${Date.now()}@test.com`;
    await email.fill(newEmail);

    await authedPage.getByTestId('profile-save').click();

    await expect(authedPage.getByTestId('profile-saved')).toBeVisible();

    // Persisted: reloading shows the new value.
    await authedPage.reload();
    await expect(authedPage.getByTestId('profile-email').locator('input')).toHaveValue(newEmail);
  });

  test('setting a homepage persists', async ({ authedPage }) => {
    await authedPage.goto('/account');

    const homepage = authedPage.getByTestId('profile-homepage').locator('input');
    const url = `https://example-${Date.now()}.dev`;
    await homepage.fill(url);
    await authedPage.getByTestId('profile-save').click();
    await expect(authedPage.getByTestId('profile-saved')).toBeVisible();

    await authedPage.reload();
    await expect(authedPage.getByTestId('profile-homepage').locator('input')).toHaveValue(url);
  });
});
