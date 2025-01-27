// Copyright © Hyper Brew LLC
// SPDX-License-Identifier: MIT

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
            input: "./backend/code.ts",
        },
    },
});
