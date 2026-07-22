// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#pragma once

#include "slint.h"
#include "todo.h"
#include <memory>

struct AppState
{
    slint::ComponentHandle<todo_ui::MainWindow> mainWindow;
    std::shared_ptr<slint::VectorModel<todo_ui::TodoItem>> todo_model;
};

AppState create_ui();