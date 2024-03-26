// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// ANCHOR: main
// src/main.cpp

#include "appwindow.h" // generated header from memory.slint

int main(int argc, char **argv)
{
    auto main_window = MainWindow::create();
    main_window->run();
}
// ANCHOR_END: main
