// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    import { AppWindow } from "fullscreen_toggle.slint";
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    let mut fullscreen = false;

    let ui_handle = ui.as_weak();
    ui.on_fullscreen_toggle(move || {
        let ui = ui_handle.unwrap();
        fullscreen = !fullscreen;
        ui.window().set_fullscreen(fullscreen);
    });

    ui.run()
}
