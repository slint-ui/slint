#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";
const { DragAction } = slint.language;

const ui = slint.loadFile(new URL("kanban.slint", import.meta.url));
const appWindow = new ui.MainWindow();

// What we attach to each `DataTransfer` via the `userData` property. A copy of
// the `TaskData` plus the row it came from, so `can-drop` recognizes our own
// payloads and `dropped` knows what to remove on a move.
class DragPayload {
    constructor(task, sourceColumn, sourceIndex) {
        this.task = task;
        this.sourceColumn = sourceColumn;
        this.sourceIndex = sourceIndex;
    }
}

const todo = new slint.ArrayModel([
    { title: "Write release notes" },
    { title: "Reply to mailing list" },
    { title: "Triage open issues" },
]);
const doing = new slint.ArrayModel([
    { title: "Polish drag-and-drop example" },
    { title: "Review kanban PR" },
]);
const done = new slint.ArrayModel([{ title: "Set up project skeleton" }]);

appWindow.todo = todo;
appWindow.doing = doing;
appWindow.done = done;

const columns = [todo, doing, done];

appWindow.Api.make_data = (task, sourceColumn, sourceIndex) => {
    const transfer = new slint.DataTransfer();
    transfer.userData = new DragPayload(task, sourceColumn, sourceIndex);
    return transfer;
};

appWindow.Api.can_drop = (event, _targetColumn, _targetIndex) => {
    if (event.data.userData instanceof DragPayload) {
        // Our own card: accept whatever modifier the user is holding.
        return event.proposed_action;
    }
    if (event.data.hasPlainText) {
        // External plain text drop: always treated as a copy.
        return DragAction.Copy;
    }
    return DragAction.None;
};

appWindow.Api.dropped = (event, targetColumn, targetIndex) => {
    if (targetColumn < 0 || targetColumn >= columns.length) return;
    const payload = event.data.userData;

    if (payload instanceof DragPayload) {
        if (event.proposed_action !== DragAction.Move) {
            // Anything that isn't an explicit move is treated as a copy.
            columns[targetColumn].splice(targetIndex, 0, payload.task);
            return;
        }
        const source = payload.sourceColumn;
        const sourceIndex = payload.sourceIndex;

        if (source === targetColumn) {
            // Same-column reorder. Drops at the source slot or immediately
            // after it are no-ops; otherwise remove the source first, adjusting
            // the target index for the shift that the removal causes.
            if (targetIndex === sourceIndex || targetIndex === sourceIndex + 1)
                return;
            const task = payload.task;
            columns[source].splice(sourceIndex, 1);
            const adjusted =
                targetIndex > sourceIndex ? targetIndex - 1 : targetIndex;
            columns[targetColumn].splice(adjusted, 0, task);
        } else {
            // Cross-column move. Source and target are independent models, so
            // the order of operations doesn't affect index stability.
            columns[source].splice(sourceIndex, 1);
            columns[targetColumn].splice(targetIndex, 0, payload.task);
        }
    } else if (event.data.hasPlainText) {
        columns[targetColumn].splice(targetIndex, 0, {
            title: event.data.plainText,
        });
    }
};

await appWindow.run();
