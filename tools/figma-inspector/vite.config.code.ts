// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { defineConfig } from "vite";
import { figmaCodePlugin } from "vite-figma-plugin";

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [figmaCodePlugin()],
    build: {
        emptyOutDir: false,
        outDir: ".tmp",
        target: "chrome58",
        rollupOptions: {
            output: {
                manualChunks: {},
                entryFileNames: "code.js",
            },
            input: "./src-code/code.ts",
        },
    },
});
