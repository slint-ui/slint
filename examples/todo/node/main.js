#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

// import "sixtyfps";
let slint = require("slint");
// import * as demo from "../ui/todo.60";
let demo = require("../ui/todo.60");
let app = new demo.MainWindow();

let model = new slint.ArrayModel([
    {
        title: "Implement the .60 file",
        checked: true
    },
    {
        title: "Do the Rust part",
        checked: false
    },
    {
        title: "Make the C++ code",
        checked: false
    },
    {
        title: "Write some JavaScript code",
        checked: true
    },
    {
        title: "Test the application",
        checked: false
    },
    {
        title: "Ship to customer",
        checked: false
    },
    {
        title: "???",
        checked: false
    },
    {
        title: "Profit",
        checked: false
    },
]);
app.todo_model = model;

app.todo_added.setHandler(function (text) {
    model.push({ title: text, checked: false })
})

app.remove_done.setHandler(function () {
    let offset = 0;
    const length = model.length;
    for (let i = 0; i < length; ++i) {
        if (model.rowData(i - offset).checked) {
            model.remove(i - offset, 1);
            offset++;
        }
    }
})

app.run();
