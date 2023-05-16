// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// TODO: Do build and package wasm-lsp separately. Right now vite does not
// support `exclude` in web workers!

// cSpell: ignore iife lumino

import { defineConfig, UserConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

export default defineConfig(({ command }) => {
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
        resolve: {},
        plugins: [
            VitePWA({
                registerType: "autoUpdate",
                injectRegister: "auto",
                injectManifest: {
                    injectionPoint: undefined,
                },
                srcDir: "src/service_worker",
                filename: "service_worker.ts",
                workbox: {
                    swDest: "sw.js",
                    maximumFileSizeToCacheInBytes: 10 * 1024 * 1024,
                },
                strategies: "injectManifest",
                devOptions: {
                    enabled: true,
                },
            }),
        ],
    };

    const global_aliases = {
        "@lsp/": "../../../lsp/pkg/",
        "~@lumino": "node_modules/@lumino/", // work around strange defaults in @lumino
        path: "path-browserify", // To make path.sep available to monaco
    };

    if (command === "serve") {
        // For development builds, serve the wasm interpreter straight out of the local file system.
        base_config.resolve = {
            alias: {
                "@preview/": "../../../api/wasm-interpreter/pkg/",
                ...global_aliases,
            },
        };
    } else {
        // For distribution builds,
        // assume deployment on the main website where the loading file (index.js) is in the assets/
        // sub-directory and the relative path to the interpreter is as below.
        if (base_config.build == null) {
            base_config.build = {};
        }
        base_config.build.rollupOptions = {
            makeAbsoluteExternalsRelative: true,
            external: [
                "../../../../wasm-interpreter/slint_wasm_interpreter.js",
            ],
            ...base_config.build.rollupOptions,
        };
        base_config.resolve = {
            alias: {
                "@preview/": "../../../../wasm-interpreter/",
                ...global_aliases,
            },
        };
    }

    return base_config as UserConfig;
});
