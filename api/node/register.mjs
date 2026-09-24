// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Entry point that enables `import { MainWindow } from "./main.slint"`.
//
//   node --import slint-ui/register app.mjs
//   deno run --preload npm:slint-ui/register app.ts
//   bun --preload slint-ui/register app.ts

// A namespace import, because Bun's node:module has no registerHooks and a
// named import of a missing export is a link-time error there.
import * as nodeModule from "node:module";
import { pathToFileURL } from "node:url";
import { load, moduleSource, resolve } from "./slint-loader.mjs";

if (typeof Bun !== "undefined") {
    // Bun has no node:module hooks, but its own plugin API does the same job.
    Bun.plugin({
        name: "slint",
        setup(build) {
            build.onLoad({ filter: /\.slint$/ }, (args) => ({
                contents: moduleSource(pathToFileURL(args.path).href),
                loader: "js",
            }));
        },
    });
} else if (typeof nodeModule.registerHooks === "function") {
    nodeModule.registerHooks({ resolve, load });
} else {
    throw new Error(
        "Importing .slint files needs Node.js 22.18 or newer. " +
            "On an older version, call slint.loadFile() instead.",
    );
}
