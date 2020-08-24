#!/usr/bin/env node
/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */

// import "sixtyfps";
require("sixtyfps");
// import * as myModule from "../cpptest/hello.60";
let hello = require("../cpptest/hello.60");

let x = new hello.Hello({
    counter: 55,
    minus_clicked: (function () { console.log("Clicked!"); x.counter--; }),
});
x.show();
