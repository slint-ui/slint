// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "kanban.h"

#include <any>
#include <array>
#include <memory>
#include <vector>

// What we attach to each `DataTransfer` via `set_user_data`. A copy of the
// `TaskData` plus the row it came from, so `can-drop` recognizes our own
// payloads and `dropped` knows what to remove on a move.
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
            { "Write release notes" },
            { "Reply to mailing list" },
            { "Triage open issues" },
    });
    auto doing = std::make_shared<slint::VectorModel<TaskData>>(std::vector<TaskData> {
            { "Polish drag-and-drop example" },
            { "Review kanban PR" },
    });
    auto done = std::make_shared<slint::VectorModel<TaskData>>(std::vector<TaskData> {
            { "Set up project skeleton" },
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

    api.on_can_drop([](slint::language::DropEvent event, int /*target*/,
                       int /*target_index*/) -> slint::language::DragAction {
        auto user_data = event.data.user_data();
        if (std::any_cast<DragPayload>(&user_data)) {
            // Our own card: accept whatever modifier the user is holding.
            return event.proposed_action;
        }
        if (event.data.has_plain_text()) {
            // External plain text drop: always treated as a copy.
            return slint::language::DragAction::Copy;
        }
        return slint::language::DragAction::None;
    });

    api.on_dropped([columns](slint::language::DropEvent event, int target, int target_index) {
        if (target < 0 || target >= static_cast<int>(columns.size())) {
            return;
        }

        auto user_data = event.data.user_data();
        if (auto *payload = std::any_cast<DragPayload>(&user_data)) {
            if (event.proposed_action != slint::language::DragAction::Move) {
                // Anything that isn't an explicit move is treated as a copy.
                columns[target]->insert(static_cast<size_t>(target_index), payload->task);
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
                // Cross-column move. Source and target are independent models,
                // so the order of operations doesn't affect index stability.
                columns[source]->erase(source_index);
                columns[target]->insert(static_cast<size_t>(target_index), payload->task);
            }
        } else if (auto text = event.data.plain_text()) {
            columns[target]->insert(static_cast<size_t>(target_index), TaskData { *text });
        }
    });

    window->run();
}
