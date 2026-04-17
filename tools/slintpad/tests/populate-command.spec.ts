// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Regression test for https://github.com/slint-ui/slint/issues/11416:
// clicking the "Start with Hello World!" code lens on an empty buffer used to
// panic the wasm LSP because the command handler called tokio::task::spawn_local
// outside of a LocalSet.
import { test, expect } from "@playwright/test";

test("'Start with Hello World!' code lens populates the editor without panicking", async ({
    page,
    browserName,
}) => {
    // Headless Firefox on CI has no WebGL, so opening the SlintPad preview
    // panics in internal/renderers/femtovg/opengl.rs:134 with
    // "Cannot proceed without WebGL - aborting". That panic shows a modal
    // dialog that intercepts our code-lens click. Skip until the preview
    // either gets a software-WebGL fallback or stops panicking on init.
    test.skip(browserName === "firefox", "preview panics without WebGL");

    // A single-whitespace `snippet` makes SlintPad open a main.slint whose content
    // has no non-whitespace tokens, which is the condition under which the LSP
    // emits the "Start with Hello World!" code lens (see tools/lsp/language.rs).
    await page.goto("http://localhost:3000/?snippet=%20");
    await expect(page.locator("#tab-key-1-0")).toContainText("main.slint");

    // Wait for the LSP-provided code lens to show up and click it. This sends
    // `workspace/executeCommand slint/populate`, which is the path that used
    // to panic in the wasm LSP.
    const code_lens = page.getByRole("button", {
        name: "Start with Hello World!",
    });
    await expect(code_lens).toBeVisible({ timeout: 20_000 });
    await code_lens.click();

    // Wait until the LSP has responded — either the populate edit landed, or
    // a panic dialog popped up.
    const editor = page.locator(".monaco-editor").first();
    const panic_dialog = page.locator("dialog.panic_dialog");
    const main_window = editor.getByText("MainWindow");
    await expect(panic_dialog.or(main_window)).toBeVisible({ timeout: 15_000 });

    // The critical assertion: no panic dialog. On failure the page snapshot
    // in the error context will show the panic message from the LSP.
    await expect(panic_dialog).toHaveCount(0);
    // And the populate edit actually landed.
    await expect(editor).toContainText('"Hello World!"');
});
