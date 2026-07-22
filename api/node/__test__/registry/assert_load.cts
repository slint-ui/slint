// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Consumer-side assertion for the registry e2e test: slint-ui must load (its
// native binary was installed automatically as an optional dependency). Run from
// a temp project that has slint-ui installed.

const slint = require("slint-ui") as typeof import("slint-ui");

if (typeof slint.loadFile !== "function") {
    console.error("FAIL: slint-ui did not load (loadFile missing)");
    process.exit(1);
}

console.log("OK");
