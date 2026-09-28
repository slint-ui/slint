// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Consumer-side assertion for the registry e2e test (__test__/registry/e2e.test.mts).
// slint-ui-dev only provides the development binary and must not be imported
// directly: its entry point throws. Run from a temp project that has it installed.

try {
    require("slint-ui-dev");
    console.error("FAIL: importing slint-ui-dev did not throw");
    process.exit(1);
} catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    if (!/must not be imported directly/.test(message)) {
        console.error("FAIL: unexpected error:", message);
        process.exit(1);
    }
    console.log("guard OK");
}
