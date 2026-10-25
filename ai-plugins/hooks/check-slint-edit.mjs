#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Codex PostToolUse hook for apply_patch and Bash, since Codex has no LSP
// support: compiles the .slint files they touched with `slint-viewer --check`
// and hands the diagnostics to the agent. Both pass their patch text or shell
// command as tool_input.command.

import { spawn } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";

// Codex doesn't pass a shell command's duration, so a .slint file the command
// mentions counts as touched when it changed within this window.
const recentMs = 60_000;

const {
    cwd,
    tool_name: toolName,
    tool_input: toolInput,
} = JSON.parse(readFileSync(0, "utf8"));
const command = String(toolInput?.command ?? "");
const isPatch = toolName === "apply_patch";

const paths = new Set();
for (const match of command.matchAll(
    /^\*\*\* (?:Add File|Update File|Move to): (.+)$/gm,
)) {
    paths.add(match[1].trim());
}
if (!isPatch) {
    for (const match of command.matchAll(/[^\s'"`<>|;&()=]+\.slint\b/g)) {
        paths.add(match[0]);
    }
}

const touched = (file) => {
    try {
        const { mtimeMs } = statSync(file);
        return isPatch || mtimeMs >= Date.now() - recentMs;
    } catch {
        return false;
    }
};

const check = (file) =>
    new Promise((settle) => {
        const child = spawn("slint-viewer", ["--check", file], {
            cwd,
            env: { ...process.env, NO_COLOR: "1" },
            timeout: 30_000,
        });
        let output = "";
        child.stdout.setEncoding("utf8").on("data", (data) => {
            output += data;
        });
        child.stderr.setEncoding("utf8").on("data", (data) => {
            output += data;
        });
        child.on("error", (error) => settle({ file, error }));
        child.on("close", (_code, signal) => settle({ file, output, signal }));
    });

const files = [...paths]
    .map((path) => resolve(cwd, path))
    .filter((file) => file.endsWith(".slint") && touched(file));
const results = await Promise.all(files.map(check));

const reports = [];
for (const { file, error, output, signal } of results) {
    if (error?.code === "ENOENT") {
        // check-slint-tool.mjs reports the missing slint-viewer.
        break;
    }
    if (error || signal) {
        reports.push(
            `slint-viewer --check ${file} didn't finish: ${error?.message ?? signal}`,
        );
        continue;
    }
    // slint-viewer up to 1.18 ignores NO_COLOR and always colors its diagnostics.
    const report = output.replace(/\x1b\[[0-9;]*m/g, "").trim();
    if (report) {
        reports.push(report);
    }
}

if (reports.length > 0) {
    process.stdout.write(
        JSON.stringify({
            hookSpecificOutput: {
                hookEventName: "PostToolUse",
                additionalContext: `slint-viewer --check reports:\n\n${reports.join("\n\n")}`,
            },
        }),
    );
}
