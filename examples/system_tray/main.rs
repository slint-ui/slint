// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    export { MainWindow, ExampleTray } from "system_tray.slint";
}

fn main() {
    let main_window = MainWindow::new().unwrap();
    let tray = ExampleTray::new().unwrap();

    // Push the window's initial state onto the tray so the two are in
    // sync at startup. They don't share globals (each component has its
    // own `SharedGlobals`), so all wiring is explicit.
    tray.set_tray_title(main_window.get_tray_title());
    tray.set_tray_tooltip(main_window.get_tray_tooltip());
    tray.set_tray_visible(main_window.get_tray_visible());
    tray.set_menu_enabled(main_window.get_tray_menu_enabled());

    // Forward edits in the window onto the tray.
    let tray_weak = tray.as_weak();
    main_window.on_tray_title_changed(move |s| {
        if let Some(t) = tray_weak.upgrade() {
            t.set_tray_title(s);
        }
    });
    let tray_weak = tray.as_weak();
    main_window.on_tray_tooltip_changed(move |s| {
        if let Some(t) = tray_weak.upgrade() {
            t.set_tray_tooltip(s);
        }
    });
    let tray_weak = tray.as_weak();
    main_window.on_tray_visible_changed(move |v| {
        if let Some(t) = tray_weak.upgrade() {
            t.set_tray_visible(v);
        }
    });
    let tray_weak = tray.as_weak();
    main_window.on_tray_menu_enabled_changed(move |v| {
        if let Some(t) = tray_weak.upgrade() {
            t.set_menu_enabled(v);
        }
    });

    // Hide-to-tray button: hide the window but keep the loop alive via
    // the visible tray. Pressing the OS close button does the same thing
    // by default (Slint's `CloseRequestResponse::HideWindow`).
    let win_weak = main_window.as_weak();
    main_window.on_hide_to_tray(move || {
        if let Some(w) = win_weak.upgrade() {
            w.hide().unwrap();
        }
    });

    // Tray click / "Show / hide window" menu item toggles the window.
    let win_weak = main_window.as_weak();
    tray.on_toggle_window(move || {
        let Some(w) = win_weak.upgrade() else { return };
        let now_visible = !w.window().is_visible();
        if now_visible {
            w.show().unwrap();
        } else {
            w.hide().unwrap();
        }
        let label = if now_visible { "shown" } else { "hidden" };
        w.set_activation_log(format!("Tray activated → window {label}").into());
    });

    main_window.on_quit(|| slint::quit_event_loop().unwrap());
    tray.on_quit(|| slint::quit_event_loop().unwrap());

    main_window.show().unwrap();
    slint::run_event_loop().unwrap();
}
