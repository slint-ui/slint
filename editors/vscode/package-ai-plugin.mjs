// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { cp, rm } from "node:fs/promises";
import { basename } from "node:path";

const source = new URL("../../ai-plugins/mcp/", import.meta.url);
const destination = new URL("./out/mcp/", import.meta.url);
const excluded = ["node_modules", "test", "build-checker.mjs", ".gitignore"];
await rm(destination, { recursive: true, force: true });

for (const entry of [
    ".claude-plugin/plugin.json",
    "icon.svg",
    "codex-skills/slint",
    "slint-preview",
]) {
    await cp(new URL(entry, source), new URL(entry, destination), {
        recursive: true,
        dereference: true,
        filter: (path) => !excluded.includes(basename(path)),
    });
}
