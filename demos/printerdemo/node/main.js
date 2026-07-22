#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

slint.initTranslations("printerdemo", new URL("../lang/", import.meta.url));

const demo = slint.loadFile(
    new URL("../ui/printerdemo.slint", import.meta.url),
);
const appWindow = new demo.MainWindow();

appWindow.PrinterState.ink_levels = [
    { color: "#00ffff", level: 0.3 },
    { color: "#ff00ff", level: 0.8 },
    { color: "#ffff00", level: 0.6 },
    { color: "#000000", level: 0.9 },
];

// Copy the default queue into a mutable ArrayModel.
const printerQueue = new slint.ArrayModel(
    Array.from(appWindow.PrinterQueue.printer_queue),
);
appWindow.PrinterQueue.printer_queue = printerQueue;

appWindow.PrinterQueue.start_job = function (title) {
    const now = new Date();
    const pad = (n) => String(n).padStart(2, "0");
    const date =
        `${pad(now.getHours())}:${pad(now.getMinutes())}:${pad(now.getSeconds())} ` +
        `${pad(now.getDate())}/${pad(now.getMonth() + 1)}/${now.getFullYear()}`;

    printerQueue.push({
        status: "waiting",
        progress: 0,
        title,
        owner: "user@example.com",
        pages: 1,
        size: "100kB",
        submission_date: date,
    });
};

appWindow.PrinterQueue.cancel_job = function (index) {
    printerQueue.remove(index, 1);
};

// Advance the first job's progress every second, like the Rust and C++ demos.
const progressTimer = setInterval(() => {
    if (printerQueue.length > 0) {
        const top = printerQueue.rowData(0);
        top.progress += 1;
        if (top.progress > 100) {
            printerQueue.remove(0, 1);
        } else {
            top.status = "printing";
            printerQueue.setRowData(0, top);
        }
    }
}, 1000);

await appWindow.run();
clearInterval(progressTimer);
