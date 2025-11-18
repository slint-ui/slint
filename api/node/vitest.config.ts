// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { defineConfig } from "vitest/config";

export default defineConfig({
    test: {
        include: [
            "**/window.spec.mts",
            "**/types.spec.mts",
            "**/models.spec.mts",
            "**/globals.spec.mts",
            "**/compiler.spec.mts",
        ],
        globals: true, // Enable global test/expect/describe
        isolate: true, // Use separate processes for isolation (matching ava's workerThreads: false)
        reporters: ["verbose"], // Show individual test names (similar to Ava output)
    },
});
