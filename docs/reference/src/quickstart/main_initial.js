// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// ANCHOR: main
// main.js
import * as slint from "slint-ui";

let ui = slint.loadFile("./ui/appwindow.slint");
let mainWindow = new ui.MainWindow();
await mainWindow.run();

// ANCHOR_END: main
