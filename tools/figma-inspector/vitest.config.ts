// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { playwright } from "@vitest/browser-playwright";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { defineConfig } from "vitest/config";
import type { ViteDevServer } from "vite";

const builtPreviewPlugin = {
    name: "serve-built-preview-without-transforming-it",
    configureServer(server: ViteDevServer) {
        for (const fileName of ["browser.html", "ui.html", "production.html"]) {
            server.middlewares.use(
                `/${fileName}`,
                async (_request, response, next) => {
                    try {
                        response.setHeader(
                            "content-type",
                            "text/html; charset=utf-8",
                        );
                        response.end(
                            await readFile(
                                resolve(
                                    fileName === "production.html"
                                        ? "dist"
                                        : "dist-dev",
                                    fileName === "production.html"
                                        ? "ui.html"
                                        : fileName,
                                ),
                                "utf8",
                            ),
                        );
                    } catch {
                        next();
                    }
                },
            );
        }
    },
};

export default defineConfig({
    plugins: [builtPreviewPlugin],
    publicDir: false,
    optimizeDeps: {
        include: ["jszip", "@noble/hashes/sha2.js", "@noble/hashes/utils.js"],
    },
    test: {
        projects: [
            {
                extends: true,
                test: {
                    name: "unit",
                    environment: "node",
                    include: ["tests/**/*.unit.spec.ts"],
                },
            },
            ...[1, 1.25, 2].map((density) => ({
                extends: true,
                test: {
                    name: density === 1 ? "browser" : `raster-${density}`,
                    include:
                        density === 1
                            ? ["tests/**/*.browser.spec.ts"]
                            : ["tests/raster.browser.spec.ts"],
                    fileParallelism: false,
                    testTimeout: 30000,
                    expect: { poll: { timeout: 10000 } },
                    browser: {
                        enabled: true,
                        headless: true,
                        provider: playwright({
                            // Match native and emulated density so Slint sizes the canvas backing buffer correctly.
                            launchOptions: {
                                args: [
                                    `--force-device-scale-factor=${density}`,
                                ],
                            },
                            contextOptions: { deviceScaleFactor: density },
                        }),
                        instances: [
                            {
                                browser: "chromium" as const,
                                viewport: { width: 800, height: 600 },
                            },
                        ],
                    },
                },
            })),
        ],
    },
});
