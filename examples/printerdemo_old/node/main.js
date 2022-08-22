#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// import "slint";
require("slint-ui");
// import * as demo from "../ui/printerdemo.slint";
let demo = require("../ui/printerdemo.slint");
let window = new demo.MainWindow();

window.ink_levels = [
    { color: "#00ffff", level: 0.3 },
    { color: "#ff00ff", level: 0.8 },
    { color: "#ffff00", level: 0.6 },
    { color: "#000000", level: 0.9 },
];

window.fax_number_erase.setHandler(function () {
    window.fax_number = window.fax_number.substring(
        0,
        window.fax_number.length - 1
    );
});
window.fax_send.setHandler(function () {
    console.log("Send fax to " + window.fax_number);
    window.fax_number = "";
});

window.run();
