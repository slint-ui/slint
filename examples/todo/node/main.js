#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const demo = slint.loadFile(new URL("../ui/todo.slint", import.meta.url));
const app = new demo.MainWindow();

const model = new slint.ArrayModel([
    {
        title: "Implement the .slint file",
        checked: true,
    },
    {
        title: "Do the Rust part",
        checked: false,
    },
    {
        title: "Make the C++ code",
        checked: false,
    },
    {
        title: "Write some JavaScript code",
        checked: true,
    },
    {
        title: "Test the application",
        checked: false,
    },
    {
        title: "Ship to customer",
        checked: false,
    },
    {
        title: "???",
        checked: false,
    },
    {
        title: "Profit",
        checked: false,
    },
]);
app.todo_model = model;

app.todo_added = function (text) {
    model.push({ title: text, checked: false });
};

app.remove_done = function () {
    let offset = 0;
    const length = model.length;
    for (let i = 0; i < length; ++i) {
        if (model.rowData(i - offset).checked) {
            model.remove(i - offset, 1);
            offset++;
        }
    }
};

app.run();
