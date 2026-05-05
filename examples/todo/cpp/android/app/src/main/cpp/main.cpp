// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "app.h"

extern "C" void slint_main()
{
    auto state = create_ui();
    state.mainWindow->run();
}
