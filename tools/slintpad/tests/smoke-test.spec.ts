// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test("test", async ({ page }) => {
    await page.goto("http://localhost:3000/");
    await expect(page.getByRole("menubar")).toContainText("Project");
    await expect(page.locator("#tab-key-1-0")).toContainText("main.slint");
});
