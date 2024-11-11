// Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test("smoke test", async ({ page }) => {
    await page.goto("http://localhost:4321/tng");
    await expect(page.locator('[id="_top"]')).toContainText("Welcome");
    await expect(page.getByRole("main")).toContainText(
        "Get started building your product with Slint.",
    );
    await page
        .locator("summary")
        .filter({ hasText: "Reference" })
        .first()
        .click();
    await page.getByRole("link", { name: "Rectangle" }).click();
    await page.getByRole("link", { name: "border-color" }).click();
    await expect(
        page.getByLabel("On this page", { exact: true }),
    ).toContainText("border-color");
});
