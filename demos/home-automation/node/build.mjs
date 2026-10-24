// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Bundles the demo to package it as a single executable.
// Pass `../Resources` for a macOS app bundle, which keeps files apart from the executable.

import * as esbuild from "esbuild";
import { slint } from "slint-ui/esbuild";

await esbuild.build({
    entryPoints: ["main.js"],
    bundle: true,
    platform: "node",
    format: "esm",
    outfile: "../main.bundle.mjs",
    plugins: [slint({ runtimeDir: process.argv[2] })],
});
