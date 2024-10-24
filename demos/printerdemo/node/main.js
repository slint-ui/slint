#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

slint.initTranslations("printerdemo", new URL("../lang/", import.meta.url));

const demo = slint.loadFile(
    new URL("../ui/printerdemo.slint", import.meta.url),
);
const window = new demo.MainWindow();

window.ink_levels = [
    { color: "#00ffff", level: 0.3 },
    { color: "#ff00ff", level: 0.8 },
    { color: "#ffff00", level: 0.6 },
    { color: "#000000", level: 0.9 },
];

window.run();
