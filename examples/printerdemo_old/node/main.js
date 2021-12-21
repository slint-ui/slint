#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

// import "sixtyfps";
require("sixtyfps");
// import * as demo from "../ui/printerdemo.60";
let demo = require("../ui/printerdemo.60");
let window = new demo.MainWindow();

window.ink_levels = [
    { color: "#00ffff", level: 0.30 },
    { color: "#ff00ff", level: 0.80 },
    { color: "#ffff00", level: 0.60 },
    { color: "#000000", level: 0.90 }];

window.fax_number_erase.setHandler(function () {
    window.fax_number = window.fax_number.substring(0, window.fax_number.length - 1);
})
window.fax_send.setHandler(function () {
    console.log("Send fax to " + window.fax_number);
    window.fax_number = "";
})

window.run();
