// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore reentrancy macrotask networkidle

// Regression test for the wasm LSP "recursive use of an object detected which
// would lead to unsafe aliasing in rust" panic when loading a demo that pulls
// in local imports (e.g. the gallery).
//
// Root cause: the exported async methods `trigger_file_watcher` and
// `close_document` on `SlintServer` (tools/lsp/wasm_main.rs) used to take
// `&mut self`. wasm-bindgen holds its borrow of the exported object for the
// whole lifetime of the returned future, so a `&mut self` method suspended at
// an `.await` kept an exclusive borrow while the JS event loop dispatched the
// next LSP message — and the next `SlintServer` call then hit wasm-bindgen's
// WasmRefCell borrow check and threw. Loading the gallery triggers it because
// registering the demo's imported files fires file-watcher notifications that
// interleave with the `open_document` calls for those same files.
import { test, expect } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Playwright runs from tools/slintpad; the repository root is two levels up.
const REPO_ROOT = path.resolve(process.cwd(), "..", "..");

const CONTENT_TYPES: Record<string, string> = {
    ".slint": "text/plain; charset=utf-8",
    ".png": "image/png",
    ".jpg": "image/jpeg",
    ".svg": "image/svg+xml",
};

test("loading the gallery demo does not panic the LSP", async ({
    page,
    browserName,
}) => {
    // Headless Firefox on CI has no WebGL, so the preview panics on init.
    test.skip(browserName === "firefox", "preview panics without WebGL");

    // The wasm-bindgen reentrancy guard throws this exact string; a Rust panic
    // (should the failure mode change) logs "panicked at". The failure surfaces
    // both as a worker console error and as an uncaught page error, so watch
    // both. Note the LSP worker panics independently of the preview, which keeps
    // rendering — so the console/page-error signal, not the preview state, is
    // what tells us the LSP broke.
    const panics: string[] = [];
    const is_panic = (text: string) =>
        text.includes("recursive use of an object") ||
        text.includes("panicked at");
    page.on("console", (msg) => {
        if (msg.type() === "error" && is_panic(msg.text())) {
            panics.push(msg.text());
        }
    });
    page.on("pageerror", (error) => {
        if (is_panic(error.message)) {
            panics.push(error.message);
        }
    });

    // Serve the demo and every file it imports from this checkout instead of
    // GitHub, so the test is hermetic and pinned to the current sources. SlintPad
    // resolves `load_demo` against raw.githubusercontent.com and the LSP fetches
    // each import from the same host, all on the main thread we intercept here.
    //
    // The delay matters: this is a race between the file-watcher future
    // suspending on an import load and the next LSP message being dispatched.
    // Instant local reads resolve within a microtask, so the futures never
    // overlap and the buggy `&mut self` borrow is never caught. A small
    // network-like latency keeps the load suspended across a macrotask, which is
    // what real GitHub fetches did when the panic was first observed. It does not
    // make the test flaky in the failing direction: the fixed `&self` code takes
    // reentrant-safe shared borrows and never panics regardless of timing.
    const NETWORK_LATENCY_MS = 150;
    await page.route(
        /raw\.githubusercontent\.com\/slint-ui\/slint\/[^/]+\/(.*)/,
        async (route) => {
            const rel = new URL(route.request().url()).pathname.replace(
                /^\/slint-ui\/slint\/[^/]+\//,
                "",
            );
            await new Promise((resolve) =>
                setTimeout(resolve, NETWORK_LATENCY_MS),
            );
            try {
                const body = await readFile(path.join(REPO_ROOT, rel));
                await route.fulfill({
                    status: 200,
                    contentType:
                        CONTENT_TYPES[path.extname(rel)] ?? "text/plain",
                    body,
                });
            } catch {
                await route.fulfill({ status: 404, body: "" });
            }
        },
    );

    await page.goto(
        "http://localhost:3000/?load_demo=examples/gallery/gallery.slint",
    );

    // Wait for the preview to render the gallery — the demo has loaded far
    // enough that its imports have been requested.
    const canvas = page.locator(".preview-container canvas");
    await expect(canvas).toBeVisible({ timeout: 40_000 });

    // Then let the LSP worker finish opening every imported document. The panic
    // fires while these imports are processed, so we must wait for that traffic
    // to settle before asserting it did not happen.
    await page.waitForLoadState("networkidle");

    // The critical assertion: the wasm LSP never hit the reentrancy guard. On a
    // regression `panics` holds the "recursive use of an object" message and its
    // stack, which the reporter prints.
    expect(panics, "wasm LSP panics during demo load").toHaveLength(0);
    // The panic also raises a user-visible dialog in real browsers; assert it is
    // absent too (it does not render in headless Chromium, but this guards the
    // interactive path).
    await expect(page.locator("dialog.panic_dialog")).toHaveCount(0);
});
