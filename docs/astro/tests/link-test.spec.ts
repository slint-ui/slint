// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";
import { linkMap } from "../src/utils/utils";

test("Test all links", async ({ page }) => {
    const baseUrl = "http://localhost:4321/master/docs/slint";

    for (const [_key, value] of Object.entries(linkMap)) {
        const fullUrl = `${baseUrl}${value.href}`;

        try {
            const response = await page.request.get(fullUrl);
            expect
                .soft(response.ok(), `${fullUrl} has no green status code`)
                .toBeTruthy();
        } catch {
            expect
                .soft(null, `${fullUrl} has no green status code`)
                .toBeTruthy();
        }
    }
});
