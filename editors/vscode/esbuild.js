// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

let wasmPlugin = {
    name: "wasm",
    setup(build) {
        let path = require("path");
        let fs = require("fs");

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

let esbuild = require("esbuild");
esbuild
    .build({
        entryPoints: ["src/browser.ts"],
        bundle: true,
        external: ["vscode"],
        outfile: "out/browser.js",
        format: "cjs",
        platform: "browser",
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
    })
    .catch(() => process.exit(1));

esbuild
    .build({
        entryPoints: ["src/extension.ts"],
        bundle: true,
        external: [
            "vscode",
            "vscode-languageclient",
            "vscode-languageclient/node",
            "path",
            "fs",
        ],
        outfile: "out/extension.js",
        platform: "node",
        format: "cjs",
    })
    .catch(() => process.exit(1));
