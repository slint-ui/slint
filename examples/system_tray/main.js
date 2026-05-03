#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import * as slint from "slint-ui";

const ui = slint.loadFile(new URL("system_tray.slint", import.meta.url));
const main_window = new ui.MainWindow();
const tray = new ui.ExampleTray();

// Push the window's initial state onto the tray so the two are in sync at
// startup. They don't share globals (each component has its own
// SharedGlobals), so all wiring is explicit.
tray.tray_title = main_window.tray_title;
tray.tray_tooltip = main_window.tray_tooltip;
tray.tray_visible = main_window.tray_visible;

// Forward edits in the window onto the tray.
main_window.tray_title_changed = (value) => {
    tray.tray_title = value;
};
main_window.tray_tooltip_changed = (value) => {
    tray.tray_tooltip = value;
};
main_window.tray_visible_changed = (value) => {
    tray.tray_visible = value;
};

// Hide-to-tray button: hide the window but keep the loop alive via the
// visible tray. Pressing the OS close button does the same thing by
// default (Slint's CloseRequestResponse::HideWindow).
main_window.hide_to_tray = () => {
    main_window.window.hide();
};

// Tray click / "Show / hide window" menu item toggles the window.
tray.toggle_window = () => {
    const now_visible = !main_window.window.visible;
    if (now_visible) {
        main_window.window.show();
    } else {
        main_window.window.hide();
    }
    main_window.activation_log = `Tray activated → window ${now_visible ? "shown" : "hidden"}`;
};

main_window.quit = () => slint.quitEventLoop();
tray.quit = () => slint.quitEventLoop();

main_window.window.show();
await slint.runEventLoop();
