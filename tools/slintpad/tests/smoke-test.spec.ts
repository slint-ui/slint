// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
import { test, expect } from "@playwright/test";

test("smoke test", async ({ page, browserName }) => {
    // Headless Firefox on CI has no WebGL, so the preview panics on init.
    test.skip(browserName === "firefox", "preview panics without WebGL");

    // Collect console.error messages. The wasm panic hook always logs to
    // console.error before showing the dialog, so this catches panics from
    // both the preview (main thread) and the LSP worker reliably.
    const panics: string[] = [];
    page.on("console", (msg) => {
        if (msg.type() === "error" && msg.text().includes("panicked at")) {
            panics.push(msg.text());
        }
    });

    await page.goto("http://localhost:3000/");
    await expect(page.locator("#tab-key-1-0")).toContainText("main.slint");

    // Wait for the preview to compile and render the default snippet.
    const canvas = page.locator(".preview-container canvas");
    await expect(canvas).toBeVisible({ timeout: 30_000 });

    expect(panics, "wasm panics during load").toHaveLength(0);
});
