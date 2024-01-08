// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "fullscreen_toggle.h"
#include <random>

int main()
{
    auto app_window = AppWindow::create();
    bool fullscreen = false;
    app_window.on_fullscreen_toggle(
            [fullscreen, main_window_weak = slint::ComponentWeakHandle(main_window)] {
                auto main_window = *main_window_weak.lock();
                fullscreen = !fullscreen;
                main_window.window().set_fullscreen(fullscreen);
            });

    main_window->run();
}
