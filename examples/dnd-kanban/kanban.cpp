// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "kanban.h"

#include <any>
#include <array>
#include <memory>
#include <vector>

// What we attach to each `DataTransfer` via `set_user_data`. A copy of the
// `TaskData` plus where it came from, so the drop handler can move the row
// without searching the source column by id.
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

    api.on_can_drop([](slint::DataTransfer data, int target) {
        auto user_data = data.user_data();
        auto *payload = std::any_cast<DragPayload>(&user_data);
        return payload && payload->source_column != target;
    });

    api.on_drop_task([columns](slint::DataTransfer data, int target) {
        auto user_data = data.user_data();
        auto *payload = std::any_cast<DragPayload>(&user_data);
        if (!payload) {
            return;
        }
        if (target < 0 || target >= static_cast<int>(columns.size())
            || payload->source_column == target) {
            return;
        }
        // The copy we put in `user_data` is what we move; we just delete
        // the source row.
        columns[payload->source_column]->erase(payload->source_index);
        columns[target]->push_back(payload->task);
    });

    window->run();
}
