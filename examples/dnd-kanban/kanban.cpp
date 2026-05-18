// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "kanban.h"

#include <any>
#include <array>
#include <memory>
#include <vector>

// What we attach to each `DataTransfer` via `set_user_data`. A copy of the
// `TaskData` plus the row it came from, so `source-column-of` can answer the
// .slint side and `move-task` knows what to remove on a move.
struct DragPayload
{
    TaskData task;
    int source_column;
    int source_index;
};

int main()
{
    auto window = MainWindow::create();

    auto todo = std::make_shared<slint::VectorModel<TaskData>>(std::vector<TaskData> {
            { 1, "Write release notes" },
            { 2, "Reply to mailing list" },
            { 3, "Triage open issues" },
    });
    auto doing = std::make_shared<slint::VectorModel<TaskData>>(std::vector<TaskData> {
            { 4, "Polish drag-and-drop example" },
            { 5, "Review kanban PR" },
    });
    auto done = std::make_shared<slint::VectorModel<TaskData>>(std::vector<TaskData> {
            { 6, "Set up project skeleton" },
    });

    window->set_todo(todo);
    window->set_doing(doing);
    window->set_done(done);

    std::array<std::shared_ptr<slint::VectorModel<TaskData>>, 3> columns = { todo, doing, done };

    const auto &api = window->global<Api>();

    api.on_make_data([](TaskData task, int source_column, int source_index) {
        slint::DataTransfer transfer;
        transfer.set_user_data(DragPayload { task, source_column, source_index });
        return transfer;
    });

    api.on_source_column_of([](slint::DataTransfer data) {
        auto user_data = data.user_data();
        auto *payload = std::any_cast<DragPayload>(&user_data);
        return payload ? payload->source_column : -1;
    });

    api.on_add_task([columns](slint::DataTransfer data, int target, int target_index) {
        auto user_data = data.user_data();
        auto *payload = std::any_cast<DragPayload>(&user_data);
        if (!payload) {
            return;
        }
        if (target < 0 || target >= static_cast<int>(columns.size())) {
            return;
        }
        columns[target]->insert(static_cast<size_t>(target_index), payload->task);
    });

    api.on_move_task([columns](slint::DataTransfer data, int target, int target_index) {
        auto user_data = data.user_data();
        auto *payload = std::any_cast<DragPayload>(&user_data);
        if (!payload) {
            return;
        }
        if (target < 0 || target >= static_cast<int>(columns.size())) {
            return;
        }
        int source = payload->source_column;
        int source_index = payload->source_index;

        if (source == target) {
            // Same-column reorder. Drops at the source slot or immediately
            // after it are no-ops; otherwise remove the source first and
            // adjust the target index for the shift that the removal causes.
            if (target_index == source_index || target_index == source_index + 1) {
                return;
            }
            TaskData task = payload->task;
            columns[source]->erase(source_index);
            int adjusted = target_index > source_index ? target_index - 1 : target_index;
            columns[target]->insert(static_cast<size_t>(adjusted), task);
        } else {
            // Cross-column move. Source and target are independent models, so
            // the order of operations doesn't affect index stability.
            columns[source]->erase(source_index);
            columns[target]->insert(static_cast<size_t>(target_index), payload->task);
        }
    });

    window->run();
}
