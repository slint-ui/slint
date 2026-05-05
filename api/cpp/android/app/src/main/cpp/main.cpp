// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include "main.h" // Generated from main.slint

// Entry point for Slint applications on Android.
// The Slint runtime takes care of Android platform initialization;
// this function is called once the platform is ready.
extern "C" void slint_main()
{
    auto window = MainWindow::create();
    window->run();
}
