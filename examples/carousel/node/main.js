#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

let demo = slint.loadFile(new URL("../slint-project.json", import.meta.url));
let app = new demo.MainWindow();

app.run();
