// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include "main_window.h"

void init_virtual_keyboard(slint::ComponentHandle<MainWindow> app)
{
    app->global<VirtualKeyboardHandler>().on_key_pressed(
            [=](auto key) { app->window().dispatch_key_press_event(key); });
}

int main()
{
    auto main_window = MainWindow::create();
    init_virtual_keyboard(main_window);
    main_window->run();
}
