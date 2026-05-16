# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

from dataclasses import dataclass

import slint
from slint import DataTransfer, ListModel

TaskData = slint.loader.kanban.TaskData


# What we attach to each `DataTransfer` via `set_user_data`. A copy of the
# `TaskData` plus the row it came from, so `source-column-of` can answer the
# .slint side and `move-task` knows what to remove on a move.
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

    @slint.callback(global_name="Api", name="source-column-of")
    def source_column_of(self, data: DataTransfer) -> int:
        payload = data.user_data
        return payload.source_column if isinstance(payload, DragPayload) else -1

    @slint.callback(global_name="Api", name="add-task")
    def add_task(
        self, data: DataTransfer, target_column: int, target_index: int
    ) -> None:
        payload = data.user_data
        if not isinstance(payload, DragPayload):
            return
        if not 0 <= target_column < len(self._columns):
            return
        self._columns[target_column].insert(target_index, payload.task)

    @slint.callback(global_name="Api", name="move-task")
    def move_task(
        self, data: DataTransfer, target_column: int, target_index: int
    ) -> None:
        payload = data.user_data
        if not isinstance(payload, DragPayload):
            return
        if not 0 <= target_column < len(self._columns):
            return
        source = payload.source_column
        source_index = payload.source_index

        if source == target_column:
            # Same-column reorder. Drops at the source slot or immediately
            # after it are no-ops; otherwise remove the source first and
            # adjust the target index for the shift that the removal causes.
            if target_index == source_index or target_index == source_index + 1:
                return
            task = payload.task
            del self._columns[source][source_index]
            adjusted = target_index - 1 if target_index > source_index else target_index
            self._columns[target_column].insert(adjusted, task)
        else:
            # Cross-column move. Source and target are independent models, so
            # the order of operations doesn't affect index stability.
            del self._columns[source][source_index]
            self._columns[target_column].insert(target_index, payload.task)


main_window = MainWindow()
main_window.run()
