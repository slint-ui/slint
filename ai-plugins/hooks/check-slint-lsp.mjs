#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Claude Code SessionStart hook: Claude Code only pushes post-edit diagnostics
// for .slint files when the plugin's slint-lsp server is on PATH.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { delimiter, join } from "node:path";

const { cwd = process.cwd() } = JSON.parse(readFileSync(0, "utf8") || "{}");

// cspell:ignore PATHEXT
const extensions =
    process.platform === "win32"
        ? (process.env.PATHEXT ?? ".EXE").split(";")
        : [""];

const onPath = (name) =>
    (process.env.PATH ?? "")
        .split(delimiter)
        .filter(Boolean)
        .some((dir) =>
            extensions.some((ext) => {
                try {
                    return statSync(join(dir, name + ext)).isFile();
                } catch {
                    return false;
                }
            }),
        );

const skippedDirs = new Set(["node_modules", "target"]);
const maxDirs = 2000;

function hasSlintFiles(root) {
    const queue = [root];
    for (let i = 0; i < queue.length && i < maxDirs; i++) {
        let entries;
        try {
            entries = readdirSync(queue[i], { withFileTypes: true });
        } catch {
            continue;
        }
        for (const entry of entries) {
            if (entry.isFile() && entry.name.endsWith(".slint")) {
                return true;
            }
            if (
                entry.isDirectory() &&
                !entry.name.startsWith(".") &&
                !skippedDirs.has(entry.name)
            ) {
                queue.push(join(queue[i], entry.name));
            }
        }
    }
    return false;
}

if (!onPath("slint-lsp") && hasSlintFiles(cwd)) {
    process.stdout.write(
        JSON.stringify({
            hookSpecificOutput: {
                hookEventName: "SessionStart",
                additionalContext:
                    "slint-lsp isn't on PATH, so edits to .slint files get no compiler diagnostics. " +
                    "Offer to install it as described in the slint skill's tools-install.md.",
            },
        }),
    );
}
