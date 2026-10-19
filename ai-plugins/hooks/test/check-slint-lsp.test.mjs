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
    new URL("../check-slint-lsp.mjs", import.meta.url),
);
const root = mkdtempSync(join(tmpdir(), "slint-lsp-hook-"));
after(() => rmSync(root, { recursive: true, force: true }));

const binDir = join(root, "bin");
const slintProject = join(root, "slint-project");
const otherProject = join(root, "other-project");
mkdirSync(binDir);
mkdirSync(join(slintProject, "src", "ui"), { recursive: true });
writeFileSync(join(slintProject, "src", "ui", "app.slint"), "");
mkdirSync(otherProject);

// cspell:ignore PATHEXT
const runHook = (cwd) =>
    execFileSync(process.execPath, [hookPath], {
        input: JSON.stringify({ hook_event_name: "SessionStart", cwd }),
        env: { PATH: binDir, PATHEXT: ".EXE" },
        encoding: "utf8",
    });

test("reports a missing slint-lsp in a Slint project", () => {
    const output = JSON.parse(runHook(slintProject));
    assert.equal(output.hookSpecificOutput.hookEventName, "SessionStart");
    assert.match(
        output.hookSpecificOutput.additionalContext,
        /tools-install\.md/,
    );
});

test("stays silent outside Slint projects", () => {
    assert.equal(runHook(otherProject), "");
});

test("stays silent when slint-lsp is on PATH", () => {
    const name = process.platform === "win32" ? "slint-lsp.exe" : "slint-lsp";
    writeFileSync(join(binDir, name), "");
    chmodSync(join(binDir, name), 0o755);
    assert.equal(runHook(slintProject), "");
});
