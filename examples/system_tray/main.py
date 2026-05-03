# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

from typing import Optional

import slint

ui = slint.loader.system_tray


class ExampleTray(ui.ExampleTray):
    """Tray instance that owns the toggle-window logic and tracks the
    main window's visibility (the Python binding doesn't currently expose
    a `window` attribute through which we could read it)."""

    def __init__(self) -> None:
        super().__init__()
        self.main_window: Optional["MainWindow"] = None
        self.window_visible: bool = True

    @slint.callback
    def toggle_window(self) -> None:
        if self.main_window is None:
            return
        new_visible = not self.window_visible
        if new_visible:
            self.main_window.show()
        else:
            self.main_window.hide()
        self.window_visible = new_visible
        label = "shown" if new_visible else "hidden"
        self.main_window.activation_log = f"Tray activated → window {label}"

    @slint.callback
    def quit(self) -> None:
        slint.quit_event_loop()


class MainWindow(ui.MainWindow):
    """Window instance that drives the tray's properties."""

    def __init__(self, tray: ExampleTray) -> None:
        super().__init__()
        self.tray = tray

    @slint.callback
    def tray_title_changed(self, value: str) -> None:
        self.tray.tray_title = value

    @slint.callback
    def tray_tooltip_changed(self, value: str) -> None:
        self.tray.tray_tooltip = value

    @slint.callback
    def tray_visible_changed(self, value: bool) -> None:
        self.tray.tray_visible = value

    @slint.callback
    def tray_menu_enabled_changed(self, value: bool) -> None:
        self.tray.menu_enabled = value

    @slint.callback
    def hide_to_tray(self) -> None:
        # Hide the window but keep the loop alive via the visible tray.
        # Pressing the OS close button does the same thing by default
        # (Slint's CloseRequestResponse::HideWindow).
        self.hide()
        self.tray.window_visible = False

    @slint.callback
    def quit(self) -> None:
        slint.quit_event_loop()


tray = ExampleTray()
main_window = MainWindow(tray)
tray.main_window = main_window

# Push the window's initial state onto the tray so the two are in sync at
# startup. They don't share globals (each component has its own
# SharedGlobals), so all wiring is explicit.
tray.tray_title = main_window.tray_title
tray.tray_tooltip = main_window.tray_tooltip
tray.tray_visible = main_window.tray_visible
tray.menu_enabled = main_window.tray_menu_enabled

main_window.show()
slint.run_event_loop()
