// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Registration entry point for the Slint module loader hook.
//
// Usage: node --import slint-ui/register app.mjs
//
// This enables `import { MainWindow } from "./main.slint"` in JavaScript
// and TypeScript files, without calling loadFile() explicitly.

import { register } from "node:module";

register("./slint-loader.mjs", import.meta.url);
