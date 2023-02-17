// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#include "logic.h"
#include "appwindow.h"

void setup_logic(const Logic &logic)
{
    logic.on_increment([](int x) { return x + 1; });
}