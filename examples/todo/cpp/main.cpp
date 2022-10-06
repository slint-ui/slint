// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include "todo.h"

int main()
{
    auto demo = MainWindow::create();

    auto todo_model = std::make_shared<slint::VectorModel<TodoItem>>(std::vector {
            TodoItem { true, "Implement the .slint file" }, TodoItem { false, "Do the Rust part" },
            TodoItem { true, "Make the C++ code" },
            TodoItem { false, "Write some JavaScript code" },
            TodoItem { false, "Test the application" }, TodoItem { false, "Ship to customer" },
            TodoItem { false, "???" }, TodoItem { false, "Profit" } });
    demo->set_todo_model(todo_model);

    demo->on_todo_added([todo_model](const slint::SharedString &s) {
        todo_model->push_back(TodoItem { false, s });
    });

    demo->on_remove_done([todo_model] {
        int offset = 0;
        int count = todo_model->row_count();
        for (int i = 0; i < count; ++i) {
            if (todo_model->row_data(i - offset)->checked) {
                todo_model->erase(i - offset);
                offset += 1;
            }
        }
    });

    demo->on_popup_confirmed(
            [demo = slint::ComponentWeakHandle(demo)] { (*demo.lock())->window().hide(); });

    demo->window().on_close_requested([todo_model, demo = slint::ComponentWeakHandle(demo)] {
        int count = todo_model->row_count();
        for (int i = 0; i < count; ++i) {
            if (!todo_model->row_data(i)->checked) {
                (*demo.lock())->invoke_show_confirm_popup();
                return slint::CloseRequestResponse::KeepWindowShown;
            }
        }
        return slint::CloseRequestResponse::HideWindow;
    });

    demo->set_is_sorting_enabled(true);

    demo->on_sort([todo_model, demo = slint::ComponentWeakHandle(demo)](bool sort_by_name,
                                                                        bool sort_by_done) {
        if (!sort_by_name && !sort_by_done) {
            (*demo.lock())->set_todo_model(todo_model);
            return;
        }

        if (sort_by_name) {
            (*demo.lock())
                    ->set_todo_model(std::make_shared<slint::SortModel<TodoItem>>(
                            todo_model, [](auto lhs, auto rhs) { return lhs.title < rhs.title; }));
        }

         if (sort_by_done) {
            (*demo.lock())
                    ->set_todo_model(std::make_shared<slint::SortModel<TodoItem>>(
                            todo_model, [](auto lhs, auto rhs) { return lhs.checked > rhs.checked; }));
        }
    });

    demo->run();
}
