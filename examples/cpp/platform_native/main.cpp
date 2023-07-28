// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include <windows.h>
#include "appview.h"
#include <memory>

#define THE_BUTTON_ID 101

static std::unique_ptr<AppView> app;

extern "C" static LRESULT WindowProc(HWND h, UINT msg, WPARAM wp, LPARAM lp)
{
    switch (msg) {
    /* Add a win32 push button and do something when it's clicked.  */
    case WM_CREATE: {
        HWND hbutton = CreateWindow(
                "BUTTON", "Hey There", /* class and title */
                WS_TABSTOP | WS_VISIBLE | WS_CHILD | BS_DEFPUSHBUTTON, /* style */
                0, 0, 100, 30, /* position */
                h, /* parent */
                (HMENU)THE_BUTTON_ID, /* unique (within the application) integer identifier */
                GetModuleHandle(0), 0 /* GetModuleHandle(0) gets the hinst */
        );
        app = std::make_unique<AppView>();
        app->attachToWindow(h);
    } break;

    case WM_SIZE: {
        UINT width = LOWORD(lp);
        UINT height = HIWORD(lp);
        if (app)
            app->setGeometry(0, 40, width, height - 40);
    } break;

    case WM_COMMAND: {
        switch (LOWORD(wp)) {
        case THE_BUTTON_ID:
            app = nullptr;
            PostQuitMessage(0);
            break;
        default:;
        }
    } break;

    case WM_CLOSE:
        app = nullptr;
        PostQuitMessage(0);
        break;
    default:
        return DefWindowProc(h, msg, wp, lp);
    }
    return 0;
}

extern "C" int WINAPI WinMain(HINSTANCE hinst, HINSTANCE hprev, LPSTR cmdline, int show)
{
    if (!hprev) {
        WNDCLASS c = { 0 };
        c.lpfnWndProc = (WNDPROC)WindowProc;
        c.hInstance = hinst;
        c.hIcon = LoadIcon(0, IDI_APPLICATION);
        c.hCursor = LoadCursor(0, IDC_ARROW);
        c.hbrBackground = (HBRUSH)GetStockObject(WHITE_BRUSH);
        c.lpszClassName = "MainWindow";
        RegisterClass(&c);
    }

    HWND h = CreateWindow("MainWindow", /* window class name*/
                          "WindowTitle", /* title  */
                          WS_OVERLAPPEDWINDOW, /* style */
                          CW_USEDEFAULT, CW_USEDEFAULT, /* position */
                          CW_USEDEFAULT, CW_USEDEFAULT, /* size */
                          0, /* parent */
                          0, /* menu */
                          hinst, 0 /* lparam */
    );

    ShowWindow(h, show);

    while (1) { /* or while(running) */
        MSG msg;
        while (PeekMessage(&msg, 0, 0, 0, PM_REMOVE)) {
            if (msg.message == WM_QUIT)
                return (int)msg.wParam;
            TranslateMessage(&msg);
            DispatchMessage(&msg);
        }
    }

    return 0;
}
