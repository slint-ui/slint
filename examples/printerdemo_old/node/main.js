#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
let demo = slint.loadFile("../ui/printerdemo.slint");
let window = new demo.MainWindow();

window.ink_levels = [
    { color: "#00ffff", level: 0.30 },
    { color: "#ff00ff", level: 0.80 },
    { color: "#ffff00", level: 0.60 },
    { color: "#000000", level: 0.90 }];

window.fax_number_erase = function () {
    window.fax_number = window.fax_number.substring(0, window.fax_number.length - 1);
};
window.fax_send = function () {
    console.log("Send fax to " + window.fax_number);
    window.fax_number = "";
};

window.run();
