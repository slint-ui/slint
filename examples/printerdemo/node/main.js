#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

const path = require("path");
let slint = require("slint-ui");

let demo = require("../ui/printerdemo.slint");
let window = new demo.MainWindow();

window.ink_levels = [
    { color: "#00ffff", level: 0.30 },
    { color: "#ff00ff", level: 0.80 },
    { color: "#ffff00", level: 0.60 },
    { color: "#000000", level: 0.90 }];

window.run();
