// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { defineConfig, type Plugin } from "vite";
import { viteSingleFile } from "vite-plugin-singlefile";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readFileSync } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import { dependencyNotices } from "./scripts/dependency-notices.mjs";
import manifest from "./manifest.json";

const root = fileURLToPath(new URL(".", import.meta.url));
const { version } = JSON.parse(
    readFileSync(resolve(root, "package.json"), "utf8"),
);

export default defineConfig(({ mode }) => {
    const development = mode === "development";
    const outDir = resolve(root, development ? "dist-dev" : "dist");
    const modules = new Set<string>();
    const collectModules = (): Plugin => ({
        name: "collect-notice-modules",
        generateBundle() {
            for (const id of this.getModuleIds()) modules.add(id);
        },
    });
    return {
        base: "./",
        publicDir: false,
        resolve: {
            alias: {
                "@interpreter": resolve(root, "../../api/wasm-interpreter/pkg"),
            },
        },
        define: { DEVELOPMENT: JSON.stringify(development) },
        worker: { format: "iife", plugins: () => [collectModules()] },
        plugins: [
            collectModules(),
            {
                name: "figma-ui",
                enforce: "pre",
                // Bytes are explicitly supplied by the controller. Disable the unused
                // wasm-bindgen URL fallback so Vite does not embed the WASM twice.
                transform(code, id) {
                    if (id.endsWith("/slint_wasm_interpreter.js"))
                        return code.replace(
                            /new URL\('slint_wasm_interpreter_bg.wasm', import.meta.url\)/g,
                            "undefined",
                        );
                },
                transformIndexHtml: {
                    order: "pre",
                    handler: (html) =>
                        development
                            ? html
                            : html.replace(
                                  /<!-- development:start -->[\s\S]*?<!-- development:end -->/g,
                                  "",
                              ),
                },
                async writeBundle() {
                    const sandboxModules = JSON.parse(
                        await readFile(
                            resolve(root, ".cache/sandbox-modules.json"),
                            "utf8",
                        ),
                    );
                    const notices = await dependencyNotices([
                        ...modules,
                        ...sandboxModules,
                    ]);
                    await writeFile(
                        resolve(outDir, "THIRD_PARTY_NOTICES.txt"),
                        notices.text,
                    );
                    await writeFile(
                        resolve(outDir, "dependencies.json"),
                        JSON.stringify(notices.inventory, null, 4) + "\n",
                    );
                    await writeFile(
                        resolve(outDir, "manifest.json"),
                        JSON.stringify(manifest, null, 4) + "\n",
                    );
                    await writeFile(
                        resolve(outDir, "provenance.json"),
                        JSON.stringify(
                            {
                                channel: "development",
                                repository:
                                    "https://github.com/slint-ui/slint.git",
                                manifest: "api/wasm-interpreter/Cargo.toml",
                                revision: execFileSync(
                                    "git",
                                    ["rev-parse", "HEAD"],
                                    { cwd: root, encoding: "utf8" },
                                ).trim(),
                                dirty:
                                    execFileSync(
                                        "git",
                                        ["status", "--porcelain"],
                                        { cwd: root, encoding: "utf8" },
                                    ).trim() !== "",
                                version,
                            },
                            null,
                            4,
                        ) + "\n",
                    );
                },
            },
            viteSingleFile(),
        ],
        build: {
            target: "es2020",
            outDir,
            emptyOutDir: false,
            minify: false,
            rollupOptions: { input: resolve(root, "ui.html") },
        },
    };
});
