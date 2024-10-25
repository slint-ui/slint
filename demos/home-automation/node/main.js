#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

let ui = slint.loadFile("../ui/demo.slint");
let window = new ui.AppWindow();

await window.run();
