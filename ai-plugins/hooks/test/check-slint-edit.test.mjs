// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { execFileSync } from "node:child_process";
import {
    chmodSync,
    mkdirSync,
    mkdtempSync,
    rmSync,
    utimesSync,
    writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

const hookPath = fileURLToPath(
    new URL("../check-slint-edit.mjs", import.meta.url),
);
const root = mkdtempSync(join(tmpdir(), "slint-edit-hook-"));
after(() => rmSync(root, { recursive: true, force: true }));

const binDir = join(root, "bin");
const project = join(root, "project");
mkdirSync(binDir);
mkdirSync(join(project, "ui"), { recursive: true });
for (const name of [
    "ok.slint",
    "broken.slint",
    "hang.slint",
    "slow-1.slint",
    "slow-2.slint",
    "stale-broken.slint",
    "notes.txt",
]) {
    writeFileSync(join(project, "ui", name), "");
}
const anHourAgo = new Date(Date.now() - 3_600_000);
utimesSync(join(project, "ui", "stale-broken.slint"), anHourAgo, anHourAgo);

// cspell:ignore esac merror Rectangel
// Stands in for slint-viewer: fails on files named *broken.slint, dies on
// hang.slint, and takes a second on slow-*.slint.
writeFileSync(
    join(binDir, "slint-viewer"),
    `#!/bin/sh
case "$2" in
*broken.slint) printf '\\033[1m\\033[91merror\\033[0m: Unknown element '"'"'Rectangel'"'"'\\n' >&2; echo "  --> $2:1:1" >&2; exit 1 ;;
*hang.slint) kill -9 $$ ;;
*slow-*.slint) sleep 1 ;;
esac
`,
);
chmodSync(join(binDir, "slint-viewer"), 0o755);

const runHook = (
    toolName,
    command,
    path = `${binDir}${delimiter}${process.env.PATH}`,
) =>
    execFileSync(process.execPath, [hookPath], {
        input: JSON.stringify({
            hook_event_name: "PostToolUse",
            cwd: project,
            tool_name: toolName,
            tool_input: { command },
        }),
        env: { PATH: path },
        encoding: "utf8",
    });

const patch = (...headers) =>
    [
        "*** Begin Patch",
        ...headers.map((h) => `*** ${h}\n+x`),
        "*** End Patch",
    ].join("\n");

const skip =
    process.platform === "win32" && "the fake slint-viewer is a shell script";

test("reports diagnostics for .slint files in an apply_patch", { skip }, () => {
    const output = JSON.parse(
        runHook(
            "apply_patch",
            patch(
                "Update File: ui/ok.slint",
                "Add File: ui/broken.slint",
                "Update File: ui/notes.txt",
            ),
        ),
    );
    assert.equal(output.hookSpecificOutput.hookEventName, "PostToolUse");
    const context = output.hookSpecificOutput.additionalContext;
    assert.match(context, /Unknown element 'Rectangel'/);
    assert.match(context, /ui[/\\]broken\.slint:1:1/);
    assert.doesNotMatch(context, /ok\.slint|notes\.txt|\x1b/);
});

test("follows a move in an apply_patch", { skip }, () => {
    const output = runHook(
        "apply_patch",
        patch("Update File: ui/gone.slint\n*** Move to: ui/broken.slint"),
    );
    assert.match(output, /broken\.slint/);
});

test("checks .slint files a shell command changed", { skip }, () => {
    const output = runHook(
        "Bash",
        "cat > ui/broken.slint <<'EOF'\nexport component App {}\nEOF",
    );
    assert.match(output, /Unknown element/);
});

test("skips .slint files a shell command didn't change", { skip }, () => {
    assert.equal(runHook("Bash", "cat ui/stale-broken.slint"), "");
});

test("reports a check that doesn't finish and goes on", { skip }, () => {
    const context = JSON.parse(
        runHook(
            "apply_patch",
            patch("Update File: ui/hang.slint", "Update File: ui/broken.slint"),
        ),
    ).hookSpecificOutput.additionalContext;
    assert.match(context, /hang\.slint didn't finish: SIGKILL/);
    assert.match(context, /Unknown element/);
});

test("checks the files concurrently", { skip }, () => {
    const start = Date.now();
    runHook(
        "apply_patch",
        patch("Update File: ui/slow-1.slint", "Update File: ui/slow-2.slint"),
    );
    assert.ok(Date.now() - start < 1900, `took ${Date.now() - start}ms`);
});

test("stays silent for clean files", { skip }, () => {
    assert.equal(runHook("apply_patch", patch("Update File: ui/ok.slint")), "");
});

test("stays silent when slint-viewer is missing", () => {
    assert.equal(
        runHook("apply_patch", patch("Update File: ui/broken.slint"), root),
        "",
    );
});
