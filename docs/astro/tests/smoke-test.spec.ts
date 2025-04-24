// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test("smoke test", async ({ page }) => {
    await page.goto("");
    await expect(page.locator('[id="_top"]')).toContainText("Welcome to Slint");
    await page
        .getByLabel("Main")
        .getByRole("link", { name: "Reference" })
        .click();
    await page.getByText("Visual Elements").click();
    await page.getByRole("link", { name: "Image" }).click();
    await page.getByRole("link", { name: "colorize", exact: true }).click();
    await page.getByRole("link", { name: "brush" }).click();
});
