// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { defineConfig } from "vitest/config";

export default defineConfig({
    test: {
        include: ["**/*.spec.mts"],
        globals: true, // Enable global test/expect/describe
        pool: "forks", // Use process forks (required for native modules that need main thread)
        testTimeout: 30000, // Showing the first window of a process took up to 9s on the Windows CI runner
        teardownTimeout: 5000, // Force teardown after 5s to prevent hanging processes
        reporters: ["verbose"], // Show individual test names
        execArgv: ["--expose-gc"], // Enable global.gc() for GC tests
    },
});
