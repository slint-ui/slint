// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import type { Plugin } from "esbuild";

/**
 * An esbuild plugin for applications that use slint-ui.
 * It bundles `.slint` imports as `loadFile()` calls relative to the bundle,
 * and copies the native addon next to the bundle.
 * Use it with `format: "esm"`.
 *
 * `runtimeDir` is where, relative to the bundle at run time,
 * the files next to it at build time are, such as `../Resources` in a macOS app bundle.
 * It defaults to the directory of the bundle.
 */
export function slint(options?: { runtimeDir?: string }): Plugin;
