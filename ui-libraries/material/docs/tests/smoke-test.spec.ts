// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { test, expect } from "@playwright/test";

test.describe("homepage", () => {
    test.use({
        javaScriptEnabled: false,
        viewport: { width: 390, height: 844 },
    });

    test("content and actions work without JavaScript", async ({ page }) => {
        await page.goto("./");
        const base = new URL(page.url());
        expect(
            await page.evaluate(() => document.documentElement.scrollWidth),
        ).toBeLessThanOrEqual(390);
        await expect(page).toHaveTitle("Slint + Material Design");
        const main = page.getByRole("main");
        await expect(
            main.getByRole("heading", {
                name: "Slint + Material Design",
                exact: true,
            }),
        ).toBeVisible();
        await expect(
            main.getByText("Make your product shine", { exact: true }),
        ).toBeVisible();
        await expect(
            main.getByText("Customizable Themes", { exact: true }),
        ).toBeVisible();
        await expect(main.getByText("Tooltips", { exact: true })).toBeVisible();
        const image = main.getByRole("img", {
            name: "Material Components Hero Image",
        });
        await expect(image).toBeVisible();
        await expect
            .poll(() =>
                image.evaluate((img: HTMLImageElement) => img.naturalWidth),
            )
            .toBeGreaterThan(0);
        await expect(
            main.getByRole("link", { name: "Download APK", exact: true }),
        ).toHaveAttribute(
            "href",
            "https://material.slint.dev/apk/slint_material.apk",
        );
        await expect(
            main.getByRole("link", { name: "Web Gallery", exact: true }),
        ).toHaveAttribute("href", "https://material.slint.dev/wasm/");
        await expect(
            main.getByRole("link", { name: "Demo", exact: true }),
        ).toHaveAttribute("href", new URL("wasm/index.html", base).pathname);
        await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
            "href",
            `https://material.slint.dev${new URL(base).pathname}`,
        );
        await main
            .getByRole("link", { name: "Get Started", exact: true })
            .click();
        await expect(page).toHaveURL(new URL("getting-started/", base).href);
        await expect(page.locator('[id="_top"]')).toContainText(
            "Getting Started",
        );
    });
});

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
