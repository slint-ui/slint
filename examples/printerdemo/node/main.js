#!/usr/bin/env node
/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

const path = require("path");
let sixtyfps = require("sixtyfps");

try {
    for (font_file of ["NotoSans-Regular.ttf", "NotoSans-Bold.ttf"]) {
        sixtyfps.register_font_from_path(path.resolve(__dirname, "../ui/fonts", font_file));
    }
} catch (load_exception) {
    console.error(load_exception);
}

let demo = require("../ui/printerdemo.60");
let window = new demo.MainWindow();

window.ink_levels = [
    { color: "#00ffff", level: 0.30 },
    { color: "#ff00ff", level: 0.80 },
    { color: "#ffff00", level: 0.60 },
    { color: "#000000", level: 0.90 }];

window.run();

