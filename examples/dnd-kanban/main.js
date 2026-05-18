#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile(new URL("kanban.slint", import.meta.url));
const appWindow = new ui.MainWindow();

// What we attach to each `DataTransfer` via the `userData` property. A copy of
// the `TaskData` plus the row it came from, so `source-column-of` can answer
// the .slint side and `move-task` knows what to remove on a move.
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

appWindow.Api.source_column_of = (data) => {
    const payload = data.userData;
    return payload instanceof DragPayload ? payload.sourceColumn : -1;
};

appWindow.Api.add_task = (data, targetColumn, targetIndex) => {
    const payload = data.userData;
    if (!(payload instanceof DragPayload)) return;
    if (targetColumn < 0 || targetColumn >= columns.length) return;
    columns[targetColumn].insert(targetIndex, payload.task);
};

appWindow.Api.move_task = (data, targetColumn, targetIndex) => {
    const payload = data.userData;
    if (!(payload instanceof DragPayload)) return;
    if (targetColumn < 0 || targetColumn >= columns.length) return;
    const source = payload.sourceColumn;
    const sourceIndex = payload.sourceIndex;

    if (source === targetColumn) {
        // Same-column reorder. Drops at the source slot or immediately after
        // it are no-ops; otherwise remove the source first, adjusting the
        // target index for the shift that the removal causes.
        if (targetIndex === sourceIndex || targetIndex === sourceIndex + 1) return;
        const task = payload.task;
        columns[source].remove(sourceIndex, 1);
        const adjusted = targetIndex > sourceIndex ? targetIndex - 1 : targetIndex;
        columns[targetColumn].insert(adjusted, task);
    } else {
        // Cross-column move. Source and target are independent models, so the
        // order of operations doesn't affect index stability.
        columns[source].remove(sourceIndex, 1);
        columns[targetColumn].insert(targetIndex, payload.task);
    }
};

await appWindow.run();
