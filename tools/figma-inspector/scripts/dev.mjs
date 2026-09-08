// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { spawn } from "node:child_process";
import { watch } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const projectRoot = fileURLToPath(new URL("..", import.meta.url));
let buildProcess;
let debounceTimer;

function startBuild() {
    if (buildProcess !== undefined) {
        buildProcess.kill();
    }
    buildProcess = spawn("node", [resolve(projectRoot, "scripts/build.mjs")], {
        cwd: projectRoot,
        env: { ...process.env, MODE: "dev" },
        stdio: "inherit",
    });
}

startBuild();
const watcher = watch(resolve(projectRoot, "src"), { recursive: true }, () => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(startBuild, 100);
});

function stop() {
    watcher.close();
    buildProcess?.kill();
}

process.on("SIGINT", stop);
process.on("SIGTERM", stop);
