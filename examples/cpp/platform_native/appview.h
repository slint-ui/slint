// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#pragma once

#if defined(_WIN32) || defined(_WIN64)
#    include <windows.h>
typedef HWND WINDOW_HANDLE;
#endif

struct MyWindowAdapter;

class AppView
{
    MyWindowAdapter *myWindow = nullptr;

public:
    AppView();

    void attachToWindow(WINDOW_HANDLE winId);
    void setGeometry(int x, int y, int width, int height);
};
