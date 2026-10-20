#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { AppWindow } from "../ui/demo.slint";

const appWindow = new AppWindow();
const api = appWindow.Api;
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

await appWindow.run();
clearInterval(timer);
