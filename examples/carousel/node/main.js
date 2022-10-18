#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

const path = require("path");
let slint = require("slint-ui");

let demo = require("../ui/carousel_demo.slint");
let app = new demo.MainWindow();

app.run();
