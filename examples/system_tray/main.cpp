// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include "system_tray.h"

#include <slint.h>

int main()
{
    auto main_window = MainWindow::create();
    auto tray = ExampleTray::create();

    // Push the window's initial state onto the tray so the two are in
    // sync at startup. They don't share globals (each component has its
    // own `SharedGlobals`), so all wiring is explicit.
    tray->set_tray_title(main_window->get_tray_title());
    tray->set_tray_tooltip(main_window->get_tray_tooltip());
    tray->set_tray_visible(main_window->get_tray_visible());

    auto tray_weak = slint::ComponentWeakHandle(tray);
    auto window_weak = slint::ComponentWeakHandle(main_window);

    // Forward edits in the window onto the tray.
    main_window->on_tray_title_changed([tray_weak](slint::SharedString value) {
        if (auto t = tray_weak.lock()) {
            (*t)->set_tray_title(value);
        }
    });
    main_window->on_tray_tooltip_changed([tray_weak](slint::SharedString value) {
        if (auto t = tray_weak.lock()) {
            (*t)->set_tray_tooltip(value);
        }
    });
    main_window->on_tray_visible_changed([tray_weak](bool value) {
        if (auto t = tray_weak.lock()) {
            (*t)->set_tray_visible(value);
        }
    });

    // Hide-to-tray button: hide the window but keep the loop alive via
    // the visible tray. Pressing the OS close button does the same thing
    // by default (Slint's `CloseRequestResponse::HideWindow`).
    main_window->on_hide_to_tray([window_weak] {
        if (auto w = window_weak.lock()) {
            (*w)->hide();
        }
    });

    // Tray click / "Show / hide window" menu item toggles the window.
    tray->on_toggle_window([window_weak] {
        auto w = window_weak.lock();
        if (!w) {
            return;
        }
        bool now_visible = !(*w)->window().is_visible();
        if (now_visible) {
            (*w)->show();
        } else {
            (*w)->hide();
        }
        (*w)->set_activation_log(now_visible
                                         ? slint::SharedString("Tray activated → window shown")
                                         : slint::SharedString("Tray activated → window hidden"));
    });

    main_window->on_quit([] { slint::quit_event_loop(); });
    tray->on_quit([] { slint::quit_event_loop(); });

    main_window->show();
    slint::run_event_loop();
}
