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
        const heading = await main
            .getByRole("heading", {
                name: "Slint + Material Design",
                exact: true,
            })
            .boundingBox();
        const hero = await image.boundingBox();
        expect(heading).not.toBeNull();
        expect(hero).not.toBeNull();
        expect(hero!.y).toBeGreaterThan(heading!.y + heading!.height);
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
        const menu = page.getByRole("button", {
            name: "Toggle Menu",
            exact: true,
        });
        await menu.click();
        await expect(
            page.getByRole("link", { name: "Demo", exact: true }),
        ).toHaveAttribute("href", new URL("wasm/index.html", base).pathname);
        await menu.click();
        await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
            "href",
            `https://material.slint.dev${new URL(base).pathname}`,
        );
        await main
            .getByRole("link", { name: "Get Started", exact: true })
            .press("Enter");
        await expect(page).toHaveURL(new URL("getting-started/", base).href);
        await expect(page.locator('[id="_top"]')).toContainText(
            "Getting Started",
        );
    });
});

test("public assets and sitemap use the deployment base", async ({
    page,
    request,
}) => {
    await page.goto("./");
    const base = new URL(page.url());
    const robots = await request.get(new URL("robots.txt", base).href);
    expect(robots.ok()).toBeTruthy();
    expect(await robots.text()).toContain(
        `Sitemap: https://material.slint.dev${base.pathname}sitemap-index.xml`,
    );
    const sitemap = await request.get(new URL("sitemap-index.xml", base).href);
    expect(sitemap.ok()).toBeTruthy();
    expect(await sitemap.text()).toContain(
        `https://material.slint.dev${base.pathname}sitemap-0.xml`,
    );
    const favicon = page.locator('link[rel="icon"][sizes="32x32"]');
    await expect(favicon).toHaveAttribute(
        "href",
        `${base.pathname}favicon-32x32.png`,
    );
    expect(
        (await request.get(new URL("favicon-32x32.png", base).href)).ok(),
    ).toBeTruthy();
    await expect(page.locator('meta[property="og:image"]')).toHaveAttribute(
        "content",
        new RegExp(`^https://material\\.slint\\.dev${base.pathname}_astro/`),
    );
});

test("theme selection persists from the homepage to documentation", async ({
    page,
}) => {
    await page.goto("./");
    const html = page.locator("html");
    const initial = await html.getAttribute("data-theme");
    await page
        .getByRole("button", { name: "Toggle color theme", exact: true })
        .click();
    const selected = initial === "light" ? "dark" : "light";
    await expect(html).toHaveAttribute("data-theme", selected);
    await page.getByRole("link", { name: "Get Started", exact: true }).click();
    await expect(html).toHaveAttribute("data-theme", selected);
});

test("smoke test", async ({ page }) => {
    await page.goto("./getting-started/");
    await expect(page.locator('[id="_top"]')).toContainText("Getting Started");
    await expect(page.getByRole("main")).toContainText(
        "Material 3 Design System",
    );
    const base = new URL("../", page.url());
    await expect(
        page.getByRole("link", {
            name: "material component source",
            exact: true,
        }),
    ).toHaveAttribute("href", `${base.pathname}zip/material-1.0.1.zip`);
    await page
        .getByLabel("Main")
        .getByRole("link", { name: "FilledButton" })
        .click();
    await expect(page).toHaveURL(/filled_button/);
    await expect(page.locator('[id="_top"]')).toContainText("FilledButton");
    await expect(page.getByRole("main")).toContainText("Properties");
});

test("search opens the generated reference", async ({ page }) => {
    await page.goto("./getting-started/");
    await page.getByRole("button", { name: "Search", exact: true }).click();
    const dialog = page.getByRole("dialog", { name: "Search", exact: true });
    await dialog
        .getByRole("textbox", { name: "Search", exact: true })
        .fill("FilledButton");
    await dialog
        .getByRole("link", { name: "FilledButton", exact: true })
        .first()
        .click();
    await expect(page).toHaveURL(/components\/buttons\/filled_button\//);
    await expect(page.locator('[id="_top"]')).toContainText("FilledButton");
});

test("component cross-links use the deployment base", async ({ page }) => {
    await page.goto("./components/checkboxes/check_box_tile/");
    const base = new URL("../../../", page.url());
    const main = page.getByRole("main");
    await expect(
        main.getByRole("link", { name: "ListTile", exact: true }),
    ).toHaveAttribute("href", `${base.pathname}components/list_tile/`);
    await main.getByRole("link", { name: "CheckBox", exact: true }).click();
    await expect(page).toHaveURL(
        new URL("components/checkboxes/check_box/", base).href,
    );
    await expect(page.locator('[id="_top"]')).toContainText("CheckBox");
});

test("homepage retains the original desktop layout", async ({ page }) => {
    await page.goto("./");
    await expect(
        page.getByRole("navigation", { name: "Main navigation", exact: true }),
    ).toBeVisible();
    const heading = await page
        .getByRole("heading", { name: "Slint + Material Design", exact: true })
        .boundingBox();
    const image = await page
        .getByRole("img", {
            name: "Material Components Hero Image",
            exact: true,
        })
        .boundingBox();
    expect(heading).not.toBeNull();
    expect(image).not.toBeNull();
    expect(image!.y).toBeGreaterThan(heading!.y + heading!.height);
    expect(image!.width).toBeGreaterThanOrEqual(1000);
    await expect(
        page.getByRole("link", { name: "Get Started", exact: true }),
    ).toBeVisible();
});
