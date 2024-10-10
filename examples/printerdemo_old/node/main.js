#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
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

window.fax_number_erase = function () {
    window.fax_number = window.fax_number.substring(
        0,
        window.fax_number.length - 1,
    );
};
window.fax_send = function () {
    console.log("Send fax to " + window.fax_number);
    window.fax_number = "";
};

window.run();
