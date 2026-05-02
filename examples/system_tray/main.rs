// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    import { LineEdit, CheckBox, Button, VerticalBox, HorizontalBox, GroupBox } from "std-widgets.slint";

    // The window the user interacts with. Edits to the inputs propagate
    // to the SystemTray instance via Rust callbacks.
    export component MainWindow inherits Window {
        title: "Slint SystemTray Demo";
        preferred-width: 480px;
        preferred-height: 360px;

        // State edited by the UI. Bound two-way to the corresponding
        // LineEdit / CheckBox below; Rust forwards changes onto the tray
        // via the *-changed callbacks.
        in-out property <string> tray-title: "Slint";
        in-out property <string> tray-tooltip: "Slint SystemTray demo";
        in-out property <bool> tray-visible: true;
        in-out property <string> activation-log: "(no tray events yet)";

        callback tray-title-changed(string);
        callback tray-tooltip-changed(string);
        callback tray-visible-changed(bool);
        callback hide-to-tray;
        callback quit;

        VerticalBox {
            spacing: 12px;

            GroupBox {
                title: "Tray properties";
                VerticalBox {
                    Text {
                        text: "Title — visible label next to the icon on macOS, "
                            + "accessibility / overflow name elsewhere.";
                        wrap: word-wrap;
                    }
                    LineEdit {
                        text <=> root.tray-title;
                        edited(s) => { root.tray-title-changed(s); }
                    }
                    Text {
                        text: "Tooltip — hover text on every platform.";
                        wrap: word-wrap;
                    }
                    LineEdit {
                        text <=> root.tray-tooltip;
                        edited(s) => { root.tray-tooltip-changed(s); }
                    }
                    CheckBox {
                        text: "Tray icon visible";
                        checked <=> root.tray-visible;
                        toggled => { root.tray-visible-changed(self.checked); }
                    }
                }
            }

            GroupBox {
                title: "Last tray activation";
                Text { text: root.activation-log; }
            }

            HorizontalBox {
                alignment: end;
                Button {
                    text: "Hide to tray";
                    clicked => { root.hide-to-tray(); }
                }
                Button {
                    text: "Quit";
                    clicked => { root.quit(); }
                }
            }
        }
    }

    export component ExampleTray inherits SystemTray {
        icon: @image-url("favicon-white.png");

        // Wrapping properties so Rust gets typed `set_*` accessors. The
        // inherited `tooltip`/`title`/`visible` from the SystemTray builtin
        // aren't auto-exposed on the public component handle.
        in-out property <string> tray-tooltip <=> root.tooltip;
        in-out property <string> tray-title <=> root.title;
        in-out property <bool> tray-visible <=> root.visible;

        callback toggle-window;
        callback quit;

        // Left-click (or platform equivalent) toggles the main window.
        activated => { toggle-window(); }

        Menu {
            MenuItem {
                title: "Show / hide window";
                activated => { toggle-window(); }
            }
            MenuSeparator {}
            MenuItem {
                title: "Quit";
                activated => { quit(); }
            }
        }
    }
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
