// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { mkdir, writeFile } from "node:fs/promises";
import { defineConfig } from "vite";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL(".", import.meta.url));
export default defineConfig(({ mode }) => ({
    publicDir: false,
    plugins: [
        {
            name: "sandbox-notice-modules",
            async generateBundle() {
                await mkdir(resolve(root, ".cache"), { recursive: true });
                await writeFile(
                    resolve(root, ".cache/sandbox-modules.json"),
                    JSON.stringify([...this.getModuleIds()]),
                );
            },
        },
    ],
    build: {
        outDir: resolve(
            root,
            process.env.PLUGIN_OUTPUT_DIR ??
                (mode === "development" ? "dist-dev" : "dist"),
        ),
        emptyOutDir: mode !== "development",
        minify: false,
        target: "es2020",
        lib: {
            entry: resolve(root, "src/plugin/main.ts"),
            formats: ["iife"],
            name: "FigmaToSlint",
            fileName: () => "code.js",
        },
    },
}));
