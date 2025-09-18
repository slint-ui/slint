// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import resolve from "@rollup/plugin-node-resolve";

export default {
    input: "codemirror.js", // Adjust to your entry file
    output: {
        file: "../../target/slintdocs/_static/cm6.bundle.js",
        format: "iife", // Use IIFE format for browser compatibility
        name: "cm6", // Name for the global variable
    },
    plugins: [
        resolve(), // Helps Rollup find external modules
    ],
    external: [
        "https://snapshots.slint.dev/master/wasm-interpreter/slint_wasm_interpreter.js", // Mark as external
    ],
};
