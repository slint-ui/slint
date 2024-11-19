// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test("smoke test", async ({ page }) => {
    await page.goto("http://localhost:4321/master/docs/slint");
    await expect(page.locator('[id="_top"]')).toContainText("Welcome");
    await page
        .locator("summary")
        .filter({ hasText: "Reference" })
        .first()
        .click();
    await page.getByRole("link", { name: "Image" }).click();
    await page.getByRole("link", { name: "colorize" }).click();
    await expect(page.locator("#colorize")).toContainText("colorize");
});
