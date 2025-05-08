#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile("../ui/demo.slint");
const window = new ui.AppWindow();
const api = window.Api;
const date = api.current_date;
const time = api.current_time;

const timer = setInterval(() => {
    const now = new Date();
    date.year = now.getFullYear();
    date.month = now.getMonth() + 1;
    date.day = now.getDate();
    api.current_date = date;
    time.hour = now.getHours();
    time.minute = now.getMinutes();
    time.second = now.getSeconds();
    api.current_time = time;
}, 1000);

await window.run();
clearInterval(timer);
