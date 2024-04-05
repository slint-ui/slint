// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "usecases.h"

void init_virtual_keyboard(slint::ComponentHandle<usecases_ui::App> app)
{
    app->global<VirtualKeyboardHandler>().on_key_pressed([=](auto key) {
        app->window().dispatch_key_press_event(key);
        app->window().dispatch_key_release_event(key);
    });
}

int main()
{
    auto app = usecases_ui::App::create();

    init_virtual_keyboard(main_window);

    app->run();
}
