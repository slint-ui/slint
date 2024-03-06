// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "todo.h"

int main()
{
    auto demo = todo_ui::MainWindow::create();
    using todo_ui::TodoItem;

    auto todo_model = std::make_shared<slint::VectorModel<TodoItem>>(std::vector {
            TodoItem { "Implement the .slint file", true },
            TodoItem { "Do the Rust part", false },
            TodoItem { "Make the C++ code", true },
            TodoItem { "Write some JavaScript code", false },
            TodoItem { "Test the application", false },
            TodoItem { "Ship to customer", false },
            TodoItem { "???", false },
            TodoItem { "Profit", false },
    });
    demo->set_todo_model(todo_model);

    demo->on_todo_added([todo_model](const slint::SharedString &s) {
        todo_model->push_back(TodoItem { s, false });
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

    demo->set_show_header(true);

    demo->on_apply_sorting_and_filtering([todo_model, demo = slint::ComponentWeakHandle(demo)] {
        auto demo_lock = demo.lock();
        (*demo_lock)->set_todo_model(todo_model);

        if ((*demo_lock)->get_hide_done_items()) {
            (*demo_lock)
                    ->set_todo_model(std::make_shared<slint::FilterModel<TodoItem>>(
                            (*demo_lock)->get_todo_model(), [](auto e) { return !e.checked; }));
        }

        if ((*demo_lock)->get_is_sort_by_name()) {
            (*demo_lock)
                    ->set_todo_model(std::make_shared<slint::SortModel<TodoItem>>(
                            (*demo_lock)->get_todo_model(),
                            [](auto lhs, auto rhs) { return lhs.title < rhs.title; }));
        }
    });

    demo->run();
}
