// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// This file checks if a binary package was installed (through architecture dependencies), and
// builds slint if no binary was found.

import { Worker } from "node:worker_threads";
import { spawn } from "child_process";

const worker = new Worker("./rust-module.cjs")
// Define dummy error handler to prevent node from aborting on errors
worker.on('error', (error) => { console.log(`Error loading rust-module.cjs: {error}`) })
worker.on('exit', (code) => {
    if (code != 0) {
        console.log("slint-ui: loading rust-module.cjs failed, building now")
        spawn("npm", ["run", "build"], {
            stdio: 'inherit'
        })
    }
})

