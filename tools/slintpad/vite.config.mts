// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore lumino

import { defineConfig, type UserConfig } from "vite";
import { resolve } from "node:path";

export default defineConfig(() => {
    const base_config: UserConfig = {
        server: {
            fs: {
                // Allow serving files from the project root
                allow: ["../../"],
            },
        },
        base: "./",
        build: {
            // We need to enable support for bigint
            target: "safari14",
            rollupOptions: {
                input: {
                    index: "./index.html",
                    preview: "./preview.html",
                },
            },
        },
        resolve: {
            alias: {
                "@lsp": resolve(__dirname, "../lsp/pkg"),
                "@interpreter": resolve(
                    __dirname,
                    "../../api/wasm-interpreter/pkg",
                ),
                "~@lumino": "node_modules/@lumino/", // work around strange defaults in @lumino
                path: "path-browserify", // To make path.sep available to monaco
            },
        },
    };

    return base_config as UserConfig;
});
