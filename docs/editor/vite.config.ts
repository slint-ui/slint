// Import Vite libraries
import { defineConfig } from "vite";

// Import resolve from Rollup (you may not need this since Vite has built-in module resolution)
import resolve from "@rollup/plugin-node-resolve";

// Export Vite configuration
export default defineConfig({
    build: {
        rollupOptions: {
            input: "codemirror.js",
            output: {
                // dir: "../../target/slintdocs/_static/cm6.bundle.js",
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