#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// import "slint";
let slint = require("slint-ui");
// import * as demo from "../ui/carousel_demo.slint";
let demo = require("../ui/carousel_demo.slint");
let app = new demo.MainWindow();

app.run();
