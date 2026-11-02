// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore githubusercontent Digitall

// Fonts that .slint files import reach the preview: the interpreter fetches
// them while compiling, as it can't read files in the browser.
import { test, expect, type Browser } from "@playwright/test";
import { serve_checkout } from "./serve_checkout";

const FONT_URL =
    "https://raw.githubusercontent.com/slint-ui/slint/master/examples/speedometer/01%20Digitall.ttf";

const WIDTH = /fonts-test-text-width (\d+(?:\.\d+)?)/;

// The text's width depends on the font that lays it out.
function snippet(import_font: boolean): string {
    return `${import_font ? `import "${FONT_URL}";` : ""}
export component Demo inherits Window {
    width: 300px;
    height: 80px;
    t := Text { text: "1234"; font-family: "01 Digitall"; font-size: 48px; }
    init => { debug("fonts-test-text-width " + (t.preferred-width / 1px)); }
}`;
}

async function text_width(
    browser: Browser,
    url: (code: string) => string,
    import_font: boolean,
): Promise<number> {
    const page = await browser.newPage();
    try {
        await serve_checkout(page);
        const message = page.waitForEvent("console", {
            predicate: (msg) => WIDTH.test(msg.text()),
            timeout: 40_000,
        });
        await page.goto(url(encodeURIComponent(snippet(import_font))));
        const width = WIDTH.exec((await message).text())?.[1];
        expect(width).toBeDefined();
        return Number(width);
    } finally {
        await page.close();
    }
}

async function expect_font_used(
    browser: Browser,
    url: (code: string) => string,
) {
    const with_font = await text_width(browser, url, true);
    const without = await text_width(browser, url, false);
    expect(with_font).not.toBe(without);
}

test("preview.html uses imported fonts", async ({ browser, browserName }) => {
    // Headless Firefox on CI has no WebGL, so the preview panics on init.
    test.skip(browserName === "firefox", "preview panics without WebGL");

    await expect_font_used(
        browser,
        (code) => `http://localhost:3000/preview.html?snippet=${code}`,
    );
});

test("the editor preview uses imported fonts", async ({
    browser,
    browserName,
}) => {
    // Headless Firefox on CI has no WebGL, so the preview panics on init.
    test.skip(browserName === "firefox", "preview panics without WebGL");
    test.setTimeout(120_000);

    await expect_font_used(
        browser,
        (code) => `http://localhost:3000/?snippet=${code}`,
    );
});
