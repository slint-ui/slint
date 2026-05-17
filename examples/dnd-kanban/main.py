# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

from dataclasses import dataclass

import slint
from slint import DataTransfer, ListModel

TaskData = slint.loader.kanban.TaskData


# What we attach to each `DataTransfer` via `set_user_data`. A copy of the
# `TaskData` plus where it came from, so the drop handler can move the row
# without searching the source column by id.
@dataclass
class DragPayload:
    task: TaskData
    source_column: int
    source_index: int


class MainWindow(slint.loader.kanban.MainWindow):
    def __init__(self) -> None:
        super().__init__()
        self.todo = ListModel(
            [
                TaskData(id=1, title="Write release notes"),
                TaskData(id=2, title="Reply to mailing list"),
                TaskData(id=3, title="Triage open issues"),
            ]
        )
        self.doing = ListModel(
            [
                TaskData(id=4, title="Polish drag-and-drop example"),
                TaskData(id=5, title="Review kanban PR"),
            ]
        )
        self.done = ListModel(
            [
                TaskData(id=6, title="Set up project skeleton"),
            ]
        )
        self._columns = [self.todo, self.doing, self.done]

    @slint.callback(global_name="Api", name="make-data")
    def make_data(
        self, task: TaskData, source_column: int, source_index: int
    ) -> DataTransfer:
        transfer = DataTransfer()
        transfer.user_data = DragPayload(task, source_column, source_index)
        return transfer

    @slint.callback(global_name="Api", name="can-drop")
    def can_drop(self, data: DataTransfer, target_column: int) -> bool:
        payload = data.user_data
        return (
            isinstance(payload, DragPayload) and payload.source_column != target_column
        )

    @slint.callback(global_name="Api", name="drop-task")
    def drop_task(self, data: DataTransfer, target_column: int) -> None:
        payload = data.user_data
        if not isinstance(payload, DragPayload):
            return
        if not 0 <= target_column < len(self._columns):
            return
        if payload.source_column == target_column:
            return
        # The copy we put in `user_data` is what we move; we just delete the source row.
        del self._columns[payload.source_column][payload.source_index]
        self._columns[target_column].append(payload.task)


main_window = MainWindow()
main_window.run()
