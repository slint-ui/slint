// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore lumino

import { defineConfig, UserConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

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
                "@lsp/": "../../../lsp/pkg/",
                "@preview/": "../../../api/wasm-interpreter/pkg/",
                "~@lumino": "node_modules/@lumino/", // work around strange defaults in @lumino
                path: "path-browserify", // To make path.sep available to monaco
            },
        },
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

    return base_config as UserConfig;
});
