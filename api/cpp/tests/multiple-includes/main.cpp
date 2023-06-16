// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#include "logic.h"
#include "appwindow.h"

int main(int argc, char **argv)
{
    auto my_ui = App::create();
    setup_logic(my_ui->global<Logic>());
    my_ui->run();
}
