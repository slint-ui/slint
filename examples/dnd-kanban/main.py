# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

from dataclasses import dataclass

import slint
from slint import DataTransfer, ListModel
from slint.language import DragAction, DropEvent

TaskData = slint.loader.kanban.TaskData


# What we attach to each `DataTransfer` via `set_user_data`. A copy of the
# `TaskData` plus the row it came from, so `can-drop` recognizes our own
# payloads and `dropped` knows what to remove on a move.
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

    @slint.callback(global_name="Api", name="can-drop")
    def can_drop(
        self, event: DropEvent, target_column: int, target_index: int
    ) -> DragAction:
        if isinstance(event.data.user_data, DragPayload):
            # Our own card: accept whatever modifier the user is holding.
            return event.proposed_action
        if event.data.has_plain_text:
            # External plain text drop: always treated as a copy.
            return DragAction.copy
        return DragAction.none

    @slint.callback(global_name="Api", name="dropped")
    def dropped(self, event: DropEvent, target_column: int, target_index: int) -> None:
        if not 0 <= target_column < len(self._columns):
            return
        payload = event.data.user_data

        if isinstance(payload, DragPayload):
            if event.proposed_action != DragAction.move:
                # Anything that isn't an explicit move is treated as a copy.
                self._columns[target_column].insert(target_index, payload.task)
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
                adjusted = (
                    target_index - 1 if target_index > source_index else target_index
                )
                self._columns[target_column].insert(adjusted, task)
            else:
                # Cross-column move. Source and target are independent models,
                # so the order of operations doesn't affect index stability.
                del self._columns[source][source_index]
                self._columns[target_column].insert(target_index, payload.task)
        else:
            text = event.data.plain_text
            if text is not None:
                self._columns[target_column].insert(target_index, TaskData(title=text))


main_window = MainWindow()
main_window.run()
