// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as fs from "node:fs";
import * as path from "node:path";
import { createRequire } from "node:module";
import { reexports } from "./slint-loader.mjs";

const require = createRequire(import.meta.url);
const slintEntry = require.resolve("slint-ui");
const slintUi = require(slintEntry);
// Node.js caches a native addon under its file name, so this is the one that slint-ui loaded.
const addon = Object.keys(require.cache).find((file) => file.endsWith(".node"));

/** Copy `from` to `to`, unless `to` is already a copy of it. */
function copyIfChanged(from, to) {
    const source = fs.statSync(from);
    const target = fs.statSync(to, { throwIfNoEntry: false });
    if (
        !target ||
        target.size !== source.size ||
        target.mtimeMs < source.mtimeMs
    ) {
        fs.copyFileSync(from, to);
    }
}

/** See `esbuild.d.mts`. */
export function slint({ runtimeDir = "." } = {}) {
    return {
        name: "slint",
        setup(build) {
            const options = build.initialOptions;
            const outdir = path.resolve(
                options.absWorkingDir ?? process.cwd(),
                options.outdir ?? path.dirname(options.outfile),
            );
            options.external = [...(options.external ?? []), "*.node"];
            // esbuild's ESM output throws on require() of a module left out of the bundle.
            options.banner = {
                ...options.banner,
                js: [
                    'import { createRequire as __slintCreateRequire } from "node:module";',
                    "const require = __slintCreateRequire(import.meta.url);",
                    options.banner?.js ?? "",
                ].join("\n"),
            };

            build.onLoad({ filter: /\.slint$/ }, (args) => {
                const url = path.posix.join(
                    runtimeDir,
                    path.relative(outdir, args.path).split(path.sep).join("/"),
                );
                return {
                    contents: [
                        `import { loadFile } from ${JSON.stringify(slintEntry)};`,
                        reexports(
                            slintUi.loadFile(args.path),
                            `loadFile(new URL(${JSON.stringify(url)}, import.meta.url))`,
                        ),
                    ].join("\n"),
                    loader: "js",
                };
            });

            build.onEnd((result) => {
                if (result.errors.length === 0) {
                    copyIfChanged(
                        addon,
                        path.join(outdir, path.basename(addon)),
                    );
                }
            });
        },
    };
}
