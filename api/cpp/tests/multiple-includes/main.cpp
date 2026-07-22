// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include "logic.h"
#include "app-window.h"
#include "another-window.h"

int main(int argc, char **argv)
{
    auto my_ui = App::create();
    setup_logic(my_ui->global<Logic>());

    auto my_ui2 = other::AnotherWindow::create();
    my_ui2->global<other::Logic>().on_decrement([](int x) { return x - 1; });
    my_ui2->show();

    my_ui->run();
}
