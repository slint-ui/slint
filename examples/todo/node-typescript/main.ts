#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// This example demonstrates using Slint with TypeScript.
// At runtime, `--import slint-ui/register` enables importing .slint files directly.
// For IDE autocomplete, run `npm run generate` to create the .d.ts file.

import * as slint from "slint-ui";
import { MainWindow, TodoItem } from "./todo.slint";

const app = new MainWindow();

const model = new slint.ArrayModel<TodoItem>([
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
        title: "Write some TypeScript code",
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

app.todo_added = function (text: string) {
    model.push({ title: text, checked: false });
};

app.remove_done = function () {
    let offset = 0;
    const length = model.length;
    for (let i = 0; i < length; ++i) {
        if (model.rowData(i - offset)!.checked) {
            model.remove(i - offset, 1);
            offset++;
        }
    }
};

app.run();
