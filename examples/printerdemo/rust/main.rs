/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new();
    // FIXME: better represtation of the models
    main_window.set_ink_levels(sixtyfps::VecModel::from_slice(&[
        (sixtyfps::Color::from_rgb_u8(0, 255, 255), 0.40),
        (sixtyfps::Color::from_rgb_u8(255, 0, 255), 0.20),
        (sixtyfps::Color::from_rgb_u8(255, 255, 0), 0.50),
        (sixtyfps::Color::from_rgb_u8(0, 0, 0), 0.80),
    ]));

    main_window.run();
}
