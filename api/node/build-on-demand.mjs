// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// This file checks if a binary package was installed (through architecture dependencies), and
// builds slint if no binary was found.

import { Worker } from "node:worker_threads";
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";

const worker = new Worker("./rust-module.cjs");
// Define dummy error handler to prevent node from aborting on errors
worker.on("error", (error) => {
    //console.log(`Error loading rust-module.cjs: {error}`);
});
worker.on("exit", (code) => {
    if (code !== 0) {
        // HACK: npm package removes .npmignore. If the file is present, then it means that we're in the Slint git repo,
        // and we don't want to automatically build (see https://github.com/slint-ui/slint/pull/6780).
        if (!existsSync("./.npmignore")) {
            console.log(
                "slint-ui: loading rust-module.cjs failed, building now",
            );
            spawn("npm", ["run", "build"], {
                stdio: "inherit",
            });
        }
    }
});
