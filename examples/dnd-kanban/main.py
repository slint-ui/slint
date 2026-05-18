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
                TaskData(title="Write release notes"),
                TaskData(title="Reply to mailing list"),
                TaskData(title="Triage open issues"),
            ]
        )
        self.doing = ListModel(
            [
                TaskData(title="Polish drag-and-drop example"),
                TaskData(title="Review kanban PR"),
            ]
        )
        self.done = ListModel(
            [
                TaskData(title="Set up project skeleton"),
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

    @slint.callback(global_name="Api", name="has-plaintext")
    def has_plaintext(self, data: DataTransfer) -> bool:
        return data.has_plaintext

    @slint.callback(global_name="Api", name="add-task")
    def add_task(
        self, data: DataTransfer, target_column: int, target_index: int
    ) -> None:
        if not 0 <= target_column < len(self._columns):
            return
        payload = data.user_data
        if isinstance(payload, DragPayload):
            self._columns[target_column].insert(target_index, payload.task)
            return
        text = data.fetch_plaintext()
        if text is not None:
            self._columns[target_column].insert(target_index, TaskData(title=text))

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
