#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: MIT

const path = require("path");
let slint = require("slint-ui");

let demo = require("../ui/carousel_demo.slint");
let app = new demo.MainWindow();

app.run();
