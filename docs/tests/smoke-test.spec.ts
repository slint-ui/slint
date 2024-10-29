import { test, expect } from '@playwright/test';

test('smoke test', async ({ page }) => {
  await page.goto('http://localhost:4321/tng');
  await expect(page.getByRole('banner')).toContainText('Slint Language Docs');
  await page.getByRole('link', { name: 'Language Docs', exact: true }).click();
  await expect(page.locator('sl-sidebar-state-persist')).toContainText('Getting started');
});