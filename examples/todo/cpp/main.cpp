/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#include "todo.h"

int main()
{
    static MainWindow demo;

    auto todo_model = std::make_shared<sixtyfps::VectorModel<TodoItem>>(std::vector {
        TodoItem { true, "Implement the .60 file" },
        TodoItem { false, "Do the rust part" },
        TodoItem { true, "Make the C++ code" },
        TodoItem { false, "???" },
        TodoItem { false, "Profit" }
    });
    demo.set_todo_model(todo_model);

    demo.on_todo_added([todo_model](const sixtyfps::SharedString &s) {
         todo_model->push_back(TodoItem { false, s} );
    });

    demo.on_remove_done([todo_model] {
        int offset = 0;
        int count = todo_model->row_count();
        for (int i = 0; i < count; ++i) {
            if (todo_model->row_data(i - offset).checked) {
                todo_model->erase(i - offset);
                offset += 1;
            }
        }
    });

    demo.run();
}
