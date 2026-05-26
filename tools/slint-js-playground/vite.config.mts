// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig } from "vite";
import { resolve } from "node:path";

const PRINTER_DEMO_UI_DIR = resolve(__dirname, "../../demos/printerdemo/ui");
const SLIDE_PUZZLE_DIR = resolve(__dirname, "../../examples/slide_puzzle");
const MEMORY_DIR = resolve(__dirname, "../../examples/memory");
const TODO_UI_DIR = resolve(__dirname, "../../examples/todo/ui");

export default defineConfig({
    envPrefix: "PLAYGROUND_",
    // Exposed to the client as import.meta.env.PLAYGROUND_* — used by
    // src/demos.ts to build /@fs/ URLs and resolve .slint imports against
    // the real demo sources on disk.
    define: {
        "import.meta.env.PLAYGROUND_PRINTER_DEMO_UI_DIR": JSON.stringify(PRINTER_DEMO_UI_DIR),
        "import.meta.env.PLAYGROUND_SLIDE_PUZZLE_DIR": JSON.stringify(SLIDE_PUZZLE_DIR),
        "import.meta.env.PLAYGROUND_MEMORY_DIR": JSON.stringify(MEMORY_DIR),
        "import.meta.env.PLAYGROUND_TODO_UI_DIR": JSON.stringify(TODO_UI_DIR),
    },
    server: {
        fs: {
            // The playground reads the printer demo's .slint sources via
            // /@fs/, so allow serving anything under the workspace root.
            allow: [resolve(__dirname, "../..")],
        },
    },
    optimizeDeps: {
        // Same reason as in demos/printerdemo/browser: vite's dep optimizer
        // can't handle the dynamic wasm import.
        exclude: ["slint-ui-browser"],
    },
    base: "./",
});
