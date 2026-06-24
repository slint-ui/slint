// Copyright © Hyper Brew LLC
// SPDX-License-Identifier: MIT

import { defineConfig } from "vite";
import { figmaCodePlugin } from "vite-figma-plugin";

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [figmaCodePlugin()],
    build: {
        // cannot use oxc for minification as it is still buggy.
        minify: "esbuild",
        emptyOutDir: false,
        outDir: ".tmp",
        target: "chrome58",
        rollupOptions: {
            input: "./backend/code.ts",
            output: {
                entryFileNames: "code.js",
            },
        },
    },
});
