// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore networkidle githubusercontent

// Loading a demo that pulls in imports (the gallery) must render its preview
// without panicking the wasm LSP or the preview. Regression of #11258, where
// exported async SlintServer methods took `&mut self` and tripped wasm-bindgen's
// borrow check when file-watcher and open-document notifications interleaved.
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Playwright runs from tools/slintpad; the repository root is two levels up.
const REPO_ROOT = path.resolve(process.cwd(), "..", "..");

// Any Rust panic or wasm trap, not just the specific error we regressed.
const PANIC =
    /panic|recursive use of an object|unsafe aliasing|RuntimeError|unreachable/i;

test("loading the gallery demo does not panic the LSP", async ({
    page,
    browserName,
}) => {
    // Headless Firefox on CI has no WebGL, so the preview panics on init.
    test.skip(browserName === "firefox", "preview panics without WebGL");

    // The LSP worker panics independently of the preview, so watch its console
    // and page errors rather than the preview state.
    const panics: string[] = [];
    page.on("console", (msg) => {
        if (msg.type() === "error" && PANIC.test(msg.text())) {
            panics.push(msg.text());
        }
    });
    page.on("pageerror", (error) => {
        if (PANIC.test(error.message)) {
            panics.push(error.message);
        }
    });

    // Serve the demo and its imports from this checkout so the test is hermetic
    // and pinned to the current sources. The latency is deliberate: the panic is
    // a race between a file-watcher future suspending on an import load and the
    // next LSP message being dispatched, which instant local reads never expose.
    await page.route(
        /raw\.githubusercontent\.com\/slint-ui\/slint\/[^/]+\/(.*)/,
        async (route) => {
            const rel = new URL(route.request().url()).pathname.replace(
                /^\/slint-ui\/slint\/[^/]+\//,
                "",
            );
            await new Promise((resolve) => setTimeout(resolve, 150));
            try {
                await route.fulfill({
                    status: 200,
                    body: await readFile(path.join(REPO_ROOT, rel)),
                });
            } catch {
                await route.fulfill({ status: 404, body: "" });
            }
        },
    );

    await page.goto(
        "http://localhost:3000/?load_demo=examples/gallery/gallery.slint",
    );

    // The example loads: its live preview renders.
    await expect(page.locator(".preview-container canvas")).toBeVisible({
        timeout: 40_000,
    });
    // Let the LSP finish processing the imports, where the panic would occur.
    await page.waitForLoadState("networkidle");

    expect(panics, "the wasm LSP or preview panicked").toHaveLength(0);
    await expect(page.locator("dialog.panic_dialog")).toHaveCount(0);
});
