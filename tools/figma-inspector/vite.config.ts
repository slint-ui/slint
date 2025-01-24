// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { defineConfig } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import { figmaPlugin, figmaPluginInit, runAction } from "vite-figma-plugin";

import react from "@vitejs/plugin-react";

import { config } from "./figma.config";

const action = process.env.ACTION;
const mode = process.env.MODE;

if (action) {
    runAction(
        {},
        // config,
        action,
    );
}

figmaPluginInit();

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react(), viteSingleFile(), figmaPlugin(config, mode)],
    build: {
        assetsInlineLimit: Number.POSITIVE_INFINITY,
        emptyOutDir: false,
        outDir: ".tmp",
    },
});
