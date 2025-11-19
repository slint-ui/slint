// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { defineConfig } from "vitest/config";

export default defineConfig({
    test: {
        include: ["**/*.spec.mts"],
        globals: true, // Enable global test/expect/describe
        pool: "forks", // Use process forks (required for native modules that need main thread)
        teardownTimeout: 5000, // Force teardown after 5s to prevent hanging processes
        reporters: ["verbose"], // Show individual test names
    },
});
