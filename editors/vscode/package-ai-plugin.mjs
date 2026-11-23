// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { cp, rm } from "node:fs/promises";

const source = new URL("../../ai-plugins/mcp/", import.meta.url);
const destination = new URL("./out/mcp/", import.meta.url);
await rm(destination, { recursive: true, force: true });

for (const file of [
    ".claude-plugin/plugin.json",
    "icon.svg",
    "codex-skills/slint",
    "slint-preview/assets",
    "slint-preview/channel.json",
    "slint-preview/checker.mjs",
    "slint-preview/checker/package.json",
    "slint-preview/checker/slint_wasm_interpreter.js",
    "slint-preview/checker/slint_wasm_interpreter_bg.wasm",
    "slint-preview/server.mjs",
    "slint-preview/view.html.in",
]) {
    await cp(new URL(file, source), new URL(file, destination), {
        recursive: true,
        dereference: true,
    });
}
