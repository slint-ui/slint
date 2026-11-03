#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// SessionStart hook: `check-slint-tool.mjs <tool>` tells the agent when the
// tool that reports post-edit diagnostics for .slint files isn't on PATH.
// That's slint-lsp in Claude Code and slint-viewer (see check-slint-edit.mjs)
// in Codex.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { delimiter, join } from "node:path";

const tool = process.argv[2];
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

if (!onPath(tool) && hasSlintFiles(cwd)) {
    process.stdout.write(
        JSON.stringify({
            hookSpecificOutput: {
                hookEventName: "SessionStart",
                additionalContext: `${tool} isn't on PATH, so edits to .slint files get no compiler diagnostics.`,
            },
        }),
    );
}
