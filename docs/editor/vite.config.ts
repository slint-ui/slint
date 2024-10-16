// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineConfig } from "vite";

export default defineConfig({
    build: {
        emptyOutDir: false,
        rollupOptions: {
            input: "codemirror.js",
            
            output: {
                format: "iife",
                entryFileNames: "cm6.bundle.js",
            },
            external: [
                "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js",
            ],
        },
        outDir: "../../target/slintdocs/_static",
    },
});
