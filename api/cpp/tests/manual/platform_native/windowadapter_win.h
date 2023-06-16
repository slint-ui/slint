// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

#pragma once

#include <optional>
#include <slint_platform.h>
#include "appview.h"
#include <cassert>
#include <windows.h>

namespace slint_platform = slint::experimental::platform;

struct Geometry
{
    int x = 0;
    int y = 0;
    uint32_t width = 0;
    uint32_t height = 0;
};

struct MyWindowAdapter : public slint_platform::WindowAdapter
{
    HWND hwnd;
    Geometry geometry = { 0, 0, 600, 300 };
    std::optional<slint_platform::SkiaRenderer> m_renderer;

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

        m_renderer.emplace(slint_platform::NativeWindowHandle::from_win32(hwnd, hInstance),
                           slint::PhysicalSize({ 600, 300 }));

        SetWindowLongPtr(hwnd, GWLP_USERDATA, (LONG_PTR)this);
    }

    slint_platform::AbstractRenderer &renderer() override { return m_renderer.value(); }

    slint::PhysicalSize physical_size() const override
    {
        RECT r;
        GetWindowRect(hwnd, &r);
        return slint::PhysicalSize({ uint32_t(r.right - r.left), uint32_t(r.bottom - r.top) });
    }

    void show() const override
    {
        ShowWindow(hwnd, SW_SHOWNORMAL);
        m_renderer->show();
    }

    void hide() const override
    {
        // TODO: destroy window
        m_renderer->hide();
    }

    void request_redraw() const override { InvalidateRect(hwnd, nullptr, false); }

    void render()
    {
        m_renderer->render(window());
        if (has_active_animations())
            request_redraw();
    }

    void resize(uint32_t width, uint32_t height)
    {
        slint::PhysicalSize windowSize({ width, height });
        m_renderer->resize(windowSize);
        dispatch_resize_event(slint::LogicalSize({ (float)width, (float)height }));
    }

    void setGeometry(int x, int y, int width, int height)
    {
        SetWindowPos(hwnd, nullptr, x, y, width, height, 0);
    }

    std::optional<slint::cbindgen_private::MouseEvent> mouseEventForMessage(UINT uMsg,
                                                                            LPARAM lParam)
    {
        float x = float(LOWORD(lParam));
        float y = float(HIWORD(lParam));

        auto makePressEvent = [=](UINT uMsg) {
            slint::cbindgen_private::PointerEventButton button;
            switch (uMsg) {
            case WM_LBUTTONDOWN:
                button = slint::cbindgen_private::PointerEventButton::Left;
                break;
            case WM_MBUTTONDOWN:
                button = slint::cbindgen_private::PointerEventButton::Middle;
                break;
            case WM_RBUTTONDOWN:
                button = slint::cbindgen_private::PointerEventButton::Right;
                break;
            default:
                assert(!"not implemented");
            }
            return slint::cbindgen_private::MouseEvent {
                .tag = slint::cbindgen_private::MouseEvent::Tag::Pressed,
                .pressed =
                        slint::cbindgen_private::MouseEvent::Pressed_Body {
                                .position = { x, y },
                                .button = button,
                        }
            };
        };

        auto makeReleaseEvent = [=](UINT uMsg) {
            slint::cbindgen_private::PointerEventButton button;
            switch (uMsg) {
            case WM_LBUTTONUP:
                button = slint::cbindgen_private::PointerEventButton::Left;
                break;
            case WM_MBUTTONUP:
                button = slint::cbindgen_private::PointerEventButton::Middle;
                break;
            case WM_RBUTTONUP:
                button = slint::cbindgen_private::PointerEventButton::Right;
                break;
            default:
                assert(!"not implemented");
            }
            return slint::cbindgen_private::MouseEvent {
                .tag = slint::cbindgen_private::MouseEvent::Tag::Released,
                .released =
                        slint::cbindgen_private::MouseEvent::Released_Body {
                                .position = { x, y },
                                .button = button,
                        }
            };
        };

        switch (uMsg) {
        case WM_LBUTTONUP:
        case WM_MBUTTONUP:
        case WM_RBUTTONUP:
            return makeReleaseEvent(uMsg);
        case WM_LBUTTONDOWN:
        case WM_MBUTTONDOWN:
        case WM_RBUTTONDOWN:
            return makePressEvent(uMsg);
        case WM_MOUSEMOVE:
            return slint::cbindgen_private::MouseEvent {
                .tag = slint::cbindgen_private::MouseEvent::Tag::Moved,
                .moved =
                        slint::cbindgen_private::MouseEvent::Moved_Body {
                                .position = { x, y },
                        }
            };

        default:
            break;
        }
        return std::nullopt;
    }

    void mouse_event(UINT uMsg, LPARAM lParam)
    {
        if (auto event = mouseEventForMessage(uMsg, lParam)) {
            dispatch_pointer_event(*event);
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
            slint_platform::update_timers_and_animations();
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
            slint_platform::update_timers_and_animations();
            self->mouse_event(uMsg, lParam);
            return 0;
        }
        return DefWindowProc(hwnd, uMsg, wParam, lParam);
    }

private:
};
