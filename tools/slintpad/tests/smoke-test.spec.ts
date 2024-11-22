// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
import { test, expect } from "@playwright/test";

test("smoke test", async ({ page }) => {
    const logs: string[] = [];
    page.on("console", (msg) => logs.push(msg.text()));
    await page.goto("http://localhost:3000/");
    await expect(page.getByRole("menubar")).toContainText("Project");
    await expect(page.locator("#tab-key-1-0")).toContainText("main.slint");
    console.log(logs);
    expect(logs).toContain("UI should be up!");
});
