// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#include "todo.h"

int main()
{
    auto demo = MainWindow::create();

    auto todo_model = std::make_shared<sixtyfps::VectorModel<TodoItem>>(std::vector {
        TodoItem { true, "Implement the .60 file" },
        TodoItem { false, "Do the Rust part" },
        TodoItem { true, "Make the C++ code" },
        TodoItem { false, "Write some JavaScript code" },
        TodoItem { false, "Test the application" },
        TodoItem { false, "Ship to customer" },
        TodoItem { false, "???" },
        TodoItem { false, "Profit" }
    });
    demo->set_todo_model(todo_model);

    demo->on_todo_added([todo_model](const sixtyfps::SharedString &s) {
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

    demo->run();
}
