// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

const production = process.argv.includes("--production");

const commonBuildOptions = {
    minify: production,
    sourcemap: !production,
    sourcesContent: false,
};

const wasmPlugin = {
    name: "wasm",
    setup(build) {
        const path = require("node:path");
        const fs = require("node:fs");

        // Resolve ".wasm" files to a path with a namespace
        build.onResolve({ filter: /\.wasm$/ }, (args) => {
            return {
                path: path.isAbsolute(args.path)
                    ? args.path
                    : path.join(args.resolveDir, args.path),
                namespace: "wasm-binary",
            };
        });

        // Virtual modules in the "wasm-binary" namespace contain the
        // actual bytes of the WebAssembly file. This uses esbuild's
        // built-in "binary" loader instead of manually embedding the
        // binary data inside JavaScript code ourselves.
        build.onLoad(
            { filter: /.*/, namespace: "wasm-binary" },
            async (args) => {
                return {
                    contents: await fs.promises.readFile(args.path),
                    loader: "binary",
                };
            },
        );
    },
};

const esbuild = require("esbuild");
esbuild
    .build({
        entryPoints: ["src/browser.ts"],
        bundle: true,
        external: ["vscode"],
        outfile: "out/browser.js",
        format: "cjs",
        platform: "browser",
        ...commonBuildOptions,
    })
    .catch(() => process.exit(1));

esbuild
    .build({
        entryPoints: ["src/browser-lsp-worker.ts"],
        bundle: true,
        outfile: "out/browserServerMain.js",
        format: "iife",
        platform: "browser",
        plugins: [wasmPlugin],
        ...commonBuildOptions,
    })
    .catch(() => process.exit(1));

esbuild
    .build({
        entryPoints: ["src/extension.ts"],
        bundle: true,
        external: ["vscode"],
        outfile: "out/extension.js",
        platform: "node",
        format: "cjs",
        ...commonBuildOptions,
    })
    .catch(() => process.exit(1));
