// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Consumer-side assertion for the registry e2e test (__test__/registry/e2e.test.mts).
// Run with Node's TypeScript type stripping from inside a temp project that has
// slint-ui installed. Asserts which capabilities the loaded binary exposes:
// none for a plain install, system-testing/mcp when slint-ui-dev is present.
// Set WANT_DEV=1 to require the development features.

const slint = require("slint-ui") as typeof import("slint-ui");

const features = slint.private_api.buildFeatures();
const has = (feature: string) => features.includes(feature);
const wantDev = process.env.WANT_DEV === "1";

console.log("buildFeatures:", JSON.stringify(features));

if (wantDev) {
    if (!(has("system-testing") && has("mcp"))) {
        console.error(
            "FAIL: expected dev features, got",
            JSON.stringify(features),
        );
        process.exit(1);
    }
} else if (features.length !== 0) {
    console.error("FAIL: expected no features, got", JSON.stringify(features));
    process.exit(1);
}

console.log("OK");
