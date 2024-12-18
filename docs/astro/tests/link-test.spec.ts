// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";
import { linkMap } from "../src/utils/utils";

test("Test all links", async ({ page }) => {
    for (const [key, value] of Object.entries(linkMap)) {
        const href = value.href.replace(/^\//, "");

        // Skip testing anchor links (internal page references)
        if (href.includes("#")) {
            // Optionally test if the base page exists
            const basePath = href.split("#")[0];
            if (basePath) {
                const response = await page.goto(basePath);
                expect(
                    response?.status(),
                    `Link ${key} (${basePath}) returned ${response?.status()}`,
                ).toBe(200);
            }
            continue;
        }

        const response = await page.goto(href);
        expect(
            response?.status(),
            `Link ${key} (${href}) returned ${response?.status()}`,
        ).toBe(200);

        // Optionally verify we didn't get to an error page
        const title = await page.title();
        expect(title, `Page ${href} has error title: ${title}`).not.toContain(
            "404",
        );
    }
});
