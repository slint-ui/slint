// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig({
    server: {
        fs: {
            // Allow serving files outside the demo root: the .slint sources
            // live in ../ui, the wasm bundle in ../../../api/js/browser/pkg.
            allow: [resolve(__dirname, "../../..")],
        },
    },
    optimizeDeps: {
        // Vite's dep optimizer chokes on the dynamic wasm import emitted by
        // wasm-pack output; skip pre-bundling so the .wasm file resolves
        // relative to the .js loader.
        exclude: ["slint-ui-browser"],
    },
    base: "./",
});
