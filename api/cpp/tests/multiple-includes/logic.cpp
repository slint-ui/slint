// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include "logic.h"
#include "app-window.h"
// Test that it's ok to include twice
#include "app-window.h"

void setup_logic(const Logic &logic)
{
    logic.on_increment([](int x) { return x + 1; });
}