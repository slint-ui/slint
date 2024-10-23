// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// ANCHOR: main
// main.js
import * as slint from "slint-ui";

const ui = slint.loadFile(new URL("./ui/app-window.slint", import.meta.url));
const mainWindow = new ui.MainWindow();
await mainWindow.run();

// ANCHOR_END: main
