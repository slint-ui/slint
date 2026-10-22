// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { execFileSync } from "node:child_process";
import {
    chmodSync,
    mkdirSync,
    mkdtempSync,
    rmSync,
    writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const hookPath = fileURLToPath(
    new URL("../check-slint-tool.mjs", import.meta.url),
);
const root = mkdtempSync(join(tmpdir(), "slint-tool-hook-"));
after(() => rmSync(root, { recursive: true, force: true }));

const binDir = join(root, "bin");
const slintProject = join(root, "slint-project");
const otherProject = join(root, "other-project");
mkdirSync(binDir);
mkdirSync(join(slintProject, "src", "ui"), { recursive: true });
writeFileSync(join(slintProject, "src", "ui", "app.slint"), "");
mkdirSync(join(otherProject, "node_modules", "pkg"), { recursive: true });
writeFileSync(join(otherProject, "node_modules", "pkg", "dep.slint"), "");

// cspell:ignore PATHEXT
const runHook = (tool, cwd) =>
    execFileSync(process.execPath, [hookPath, tool], {
        input: JSON.stringify({ hook_event_name: "SessionStart", cwd }),
        env: { PATH: binDir, PATHEXT: ".EXE" },
        encoding: "utf8",
    });

test("reports a missing tool in a Slint project", () => {
    const output = JSON.parse(runHook("slint-viewer", slintProject));
    assert.equal(output.hookSpecificOutput.hookEventName, "SessionStart");
    assert.equal(
        output.hookSpecificOutput.additionalContext,
        "slint-viewer isn't on PATH, so edits to .slint files get no compiler diagnostics.",
    );
});

test("stays silent outside Slint projects", () => {
    assert.equal(runHook("slint-viewer", otherProject), "");
});

test("stays silent when the tool is on PATH", () => {
    const name = process.platform === "win32" ? "slint-lsp.exe" : "slint-lsp";
    writeFileSync(join(binDir, name), "");
    chmodSync(join(binDir, name), 0o755);
    assert.equal(runHook("slint-lsp", slintProject), "");
});
