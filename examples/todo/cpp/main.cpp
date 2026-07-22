// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "app.h"

#ifdef __ANDROID__
extern "C" void slint_main()
#else
int main()
#endif
{
    auto state = create_ui();
    state.mainWindow->run();
}
