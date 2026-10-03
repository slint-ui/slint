#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { main as pack } from "./pack";

const [command, ...args] = process.argv.slice(2);
if (command === "pack") {
    void pack(args);
} else {
    console.error(
        "Usage: slint-ui pack [options]\nRun 'slint-ui pack --help' for the options.",
    );
    process.exit(1);
}
