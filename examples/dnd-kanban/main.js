#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile(new URL("kanban.slint", import.meta.url));
const appWindow = new ui.MainWindow();

// What we attach to each `DataTransfer` via the `userData` property. A copy of
// the `TaskData` plus where it came from, so the drop handler can move the row
// without searching the source column by id.
class DragPayload {
    constructor(task, sourceColumn, sourceIndex) {
        this.task = task;
        this.sourceColumn = sourceColumn;
        this.sourceIndex = sourceIndex;
    }
}

const todo = new slint.ArrayModel([
    { id: 1, title: "Write release notes" },
    { id: 2, title: "Reply to mailing list" },
    { id: 3, title: "Triage open issues" },
]);
const doing = new slint.ArrayModel([
    { id: 4, title: "Polish drag-and-drop example" },
    { id: 5, title: "Review kanban PR" },
]);
const done = new slint.ArrayModel([
    { id: 6, title: "Set up project skeleton" },
]);

appWindow.todo = todo;
appWindow.doing = doing;
appWindow.done = done;

const columns = [todo, doing, done];

appWindow.Api.make_data = (task, sourceColumn, sourceIndex) => {
    const transfer = new slint.DataTransfer();
    transfer.userData = new DragPayload(task, sourceColumn, sourceIndex);
    return transfer;
};

appWindow.Api.can_drop = (data, targetColumn) => {
    const payload = data.userData;
    return payload instanceof DragPayload && payload.sourceColumn !== targetColumn;
};

appWindow.Api.drop_task = (data, targetColumn) => {
    const payload = data.userData;
    if (!(payload instanceof DragPayload)) return;
    if (targetColumn < 0 || targetColumn >= columns.length) return;
    if (payload.sourceColumn === targetColumn) return;
    // The copy we put in `userData` is what we move; we just delete the source row.
    columns[payload.sourceColumn].remove(payload.sourceIndex, 1);
    columns[targetColumn].push(payload.task);
};

await appWindow.run();
