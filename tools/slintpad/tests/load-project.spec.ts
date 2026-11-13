// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect, type Page } from "@playwright/test";

// Headless Firefox on CI has no WebGL, so the preview panics on init.
test.skip(
    ({ browserName }) => browserName === "firefox",
    "preview panics without WebGL",
);

const PROJECT = "https://example.com/project/slint-project.json";

// `Label` only resolves through the project's include path.
const FILES: { [url: string]: string } = {
    [PROJECT]: `{
        // Comments are allowed.
        "edition": "1.19",
        "main": "ui/main.slint",
        "include-paths": ["lib"]
    }`,
    "https://example.com/project/ui/main.slint": `
import { Label } from "label.slint";
export component Main inherits Window {
    width: 100px; height: 100px;
    Label { }
}`,
    "https://example.com/project/lib/label.slint":
        'export component Label inherits Text { text: "from the include path"; }',
};

/// Serves `FILES` and returns the URLs that the page requested.
async function serve_project(page: Page): Promise<Set<string>> {
    const requested = new Set<string>();
    await page.route(/^https:\/\/example\.com\//, async (route) => {
        const url = route.request().url();
        requested.add(url);
        const body = FILES[url];
        await route.fulfill(
            body === undefined ? { status: 404 } : { status: 200, body },
        );
    });
    return requested;
}

test("load_url opens a project file in the editor", async ({ page }) => {
    const requested = await serve_project(page);
    const dialogs: string[] = [];
    page.on("dialog", (dialog) => {
        dialogs.push(dialog.message());
        void dialog.dismiss();
    });

    await page.goto(`http://localhost:3000/?load_url=${PROJECT}`);

    await expect(page.locator("#tab-key-1-0")).toContainText("main.slint");
    await expect(page.locator(".preview-container canvas")).toBeVisible({
        timeout: 30_000,
    });
    await expect
        .poll(() => [...requested], { timeout: 30_000 })
        .toEqual(expect.arrayContaining(Object.keys(FILES)));
    expect(dialogs).toEqual([]);
});

test("load_url opens a project file in the preview", async ({ page }) => {
    await serve_project(page);

    await page.goto(`http://localhost:3000/preview.html?load_url=${PROJECT}`);

    await expect(page.locator("#preview canvas")).toBeVisible({
        timeout: 30_000,
    });
    await expect(page.locator("#preview pre")).toHaveCount(0);
});
