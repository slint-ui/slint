// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test("smoke test", async ({ page }) => {
    await page.goto("/getting-started/");
    await expect(page.locator('[id="_top"]')).toContainText("Getting Started");
    await expect(page.getByRole("main")).toContainText(
        "Material 3 Design System",
    );
    await page
        .getByLabel("Main")
        .getByRole("link", { name: "FilledButton" })
        .click();
    await expect(page).toHaveURL(/filled_button/);
    await expect(page.locator('[id="_top"]')).toContainText("FilledButton");
    await expect(page.getByRole("main")).toContainText("Properties");
});
