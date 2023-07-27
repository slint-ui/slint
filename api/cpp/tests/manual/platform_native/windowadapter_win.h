// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#pragma once

#include <optional>
#include <slint_platform.h>
#include "appview.h"
#include <cassert>
#include <windows.h>

struct Geometry
{
    int x = 0;
    int y = 0;
    uint32_t width = 0;
    uint32_t height = 0;
};

struct MyWindowAdapter : public slint::platform::WindowAdapter
{
    HWND hwnd;
    Geometry geometry = { 0, 0, 600, 300 };
    std::optional<slint::platform::SkiaRenderer> m_renderer;

    MyWindowAdapter(HWND winId)
    {
        HINSTANCE hInstance = GetModuleHandleW(nullptr);

        // Register the window class.
        const wchar_t CLASS_NAME[] = L"Sample Window Class";

        WNDCLASS wc = {};

        wc.lpfnWndProc = MyWindowAdapter::windowProc;
        wc.hInstance = hInstance;
        wc.lpszClassName = CLASS_NAME;

        RegisterClass(&wc);

        // Create the window.

        hwnd = CreateWindowEx(0, // Optional window styles.
                              CLASS_NAME, // Window class
                              L"Learn to Program Windows", // Window text
                              WS_CHILDWINDOW, // Window style

                              // Size and position
                              0, 0, 600, 300,

                              winId,
                              NULL, // Menu
                              hInstance, // Instance handle
                              NULL // Additional application data
        );

        m_renderer.emplace(slint::platform::NativeWindowHandle::from_win32(hwnd, hInstance),
                           slint::PhysicalSize({ 600, 300 }));

        SetWindowLongPtr(hwnd, GWLP_USERDATA, (LONG_PTR)this);
    }

    slint::platform::AbstractRenderer &renderer() override { return m_renderer.value(); }

    slint::PhysicalSize physical_size() const override
    {
        RECT r;
        GetWindowRect(hwnd, &r);
        return slint::PhysicalSize({ uint32_t(r.right - r.left), uint32_t(r.bottom - r.top) });
    }

    void set_visible(bool visible) override { ShowWindow(hwnd, visible ? SW_SHOWNORMAL : SW_HIDE); }

    void request_redraw() override { InvalidateRect(hwnd, nullptr, false); }

    void render()
    {
        m_renderer->render();
        if (window().has_active_animations())
            request_redraw();
    }

    void resize(uint32_t width, uint32_t height)
    {
        window().dispatch_resize_event(slint::LogicalSize({ (float)width, (float)height }));
    }

    void setGeometry(int x, int y, int width, int height)
    {
        SetWindowPos(hwnd, nullptr, x, y, width, height, 0);
    }

    void mouse_event(UINT uMsg, LPARAM lParam)
    {
        using slint::LogicalPosition;
        using slint::PointerEventButton;
        float x = float(LOWORD(lParam));
        float y = float(HIWORD(lParam));
        switch (uMsg) {
        case WM_LBUTTONUP:
            window().dispatch_pointer_release_event(LogicalPosition({ x, y }),
                                                    PointerEventButton::Left);
            break;
        case WM_MBUTTONUP:
            window().dispatch_pointer_release_event(LogicalPosition({ x, y }),
                                                    PointerEventButton::Middle);
            break;
        case WM_RBUTTONUP:
            window().dispatch_pointer_release_event(LogicalPosition({ x, y }),
                                                    PointerEventButton::Right);
            break;
        case WM_LBUTTONDOWN:
            window().dispatch_pointer_press_event(LogicalPosition({ x, y }),
                                                  PointerEventButton::Left);
            break;
        case WM_MBUTTONDOWN:
            window().dispatch_pointer_press_event(LogicalPosition({ x, y }),
                                                  PointerEventButton::Middle);
            break;
        case WM_RBUTTONDOWN:
            window().dispatch_pointer_press_event(LogicalPosition({ x, y }),
                                                  PointerEventButton::Right);
            break;
        case WM_MOUSEMOVE:
            window().dispatch_pointer_move_event(LogicalPosition({ x, y }));
            break;
        default:
            break;
        }
    }

    static LRESULT CALLBACK windowProc(HWND hwnd, UINT uMsg, WPARAM wParam, LPARAM lParam)
    {
        MyWindowAdapter *self =
                reinterpret_cast<MyWindowAdapter *>(GetWindowLongPtr(hwnd, GWLP_USERDATA));
        if (self == nullptr) {
            return DefWindowProc(hwnd, uMsg, wParam, lParam);
        }
        switch (uMsg) {
        case WM_DESTROY:
            PostQuitMessage(0);
            return 0;

        case WM_PAINT: {
            PAINTSTRUCT ps;
            BeginPaint(hwnd, &ps);
            slint::platform::update_timers_and_animations();
            self->render();
            EndPaint(hwnd, &ps);
            return 0;
        }

        case WM_SIZE:
            self->resize(LOWORD(lParam), HIWORD(lParam));
            return 0;

        case WM_LBUTTONUP:
        case WM_LBUTTONDOWN:
        case WM_MBUTTONUP:
        case WM_MBUTTONDOWN:
        case WM_RBUTTONUP:
        case WM_RBUTTONDOWN:
        case WM_MOUSEMOVE:
            slint::platform::update_timers_and_animations();
            self->mouse_event(uMsg, lParam);
            return 0;
        }
        return DefWindowProc(hwnd, uMsg, wParam, lParam);
    }

private:
};
