// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#ifndef UNICODE
#    define UNICODE
#endif

#include "appwindow.h"
#include <slint_platform.h>

#if defined(_WIN32) || defined(_WIN64)
#    include "windowadapter_win.h"
#endif

namespace slint_platform = slint::experimental::platform;

struct MyPlatform : public slint_platform::Platform
{
    std::unique_ptr<MyWindowAdapter> the_window;
    std::unique_ptr<slint_platform::WindowAdapter> create_window_adapter() override
    {
        return std::move(the_window);
    }
};

AppView::AppView() { }

void AppView::setGeometry(int x, int y, int width, int height)
{
    myWindow->setGeometry(x, y, width, height);
}

void AppView::attachToWindow(WINDOW_HANDLE winId)
{
    auto p = std::make_unique<MyPlatform>();
    p->the_window = std::make_unique<MyWindowAdapter>(winId);
    myWindow = p->the_window.get();
    slint_platform::set_platform(std::move(p));

    // AppWindow is the auto-generated slint code
    static auto app = AppWindow::create();
    app->show();
}
