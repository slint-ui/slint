// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "app.h"

int main()
{
    auto state = create_ui();
    state.mainWindow->run();
}
