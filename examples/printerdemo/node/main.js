#!/usr/bin/env node
/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */

// import "sixtyfps";
require("sixtyfps");
// import * as demo from "../ui/printerdemo.60";
let demo = require("../ui/printerdemo.60");
let window = new demo.MainWindow();
window.show();

