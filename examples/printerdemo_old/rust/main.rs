// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new();
    main_window.set_ink_levels(slint::VecModel::from_slice(&[
        InkLevel { color: slint::Color::from_rgb_u8(0, 255, 255), level: 0.40 },
        InkLevel { color: slint::Color::from_rgb_u8(255, 0, 255), level: 0.20 },
        InkLevel { color: slint::Color::from_rgb_u8(255, 255, 0), level: 0.50 },
        InkLevel { color: slint::Color::from_rgb_u8(0, 0, 0), level: 0.80 },
    ]));

    main_window.on_fax_number_erase(move |main_window| {
        let mut fax_number = main_window.get_fax_number().to_string();
        fax_number.pop();
        main_window.set_fax_number(fax_number.into());
    });

    main_window.on_fax_send(move |main_window| {
        let fax_number = main_window.get_fax_number().to_string();
        println!("Sending a fax to {}", fax_number);
        main_window.set_fax_number(slint::SharedString::default());
    });

    main_window.on_quit(move |_| {
        #[cfg(not(target_arch = "wasm32"))]
        std::process::exit(0);
    });

    main_window.run();
}
