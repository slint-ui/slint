// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![no_std]
#![cfg_attr(feature = "mcu-pico-st7789", no_main)]

extern crate alloc;

use alloc::string::ToString;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

#[cfg(not(feature = "mcu-pico-st7789"))]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    #[cfg(feature = "sixtyfps-rendering-backend-mcu")]
    {
        #[cfg(feature = "mcu-simulator")]
        sixtyfps_rendering_backend_mcu::init_simulator();
        #[cfg(not(feature = "mcu-simulator"))]
        sixtyfps_rendering_backend_mcu::init_with_mock_display();
    }

    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    printerdemo_main();
}

#[cfg(feature = "mcu-pico-st7789")]
#[sixtyfps_rendering_backend_mcu::entry]
fn main() -> ! {
    sixtyfps_rendering_backend_mcu::init_board();
    printerdemo_main();
    loop {}
}

fn printerdemo_main() {
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

    let main_weak = main_window.as_weak();
    main_window.on_fax_number_erase(move || {
        let main_window = main_weak.unwrap();
        let mut fax_number = main_window.get_fax_number().to_string();
        fax_number.pop();
        main_window.set_fax_number(fax_number.into());
    });

    let main_weak = main_window.as_weak();
    main_window.on_fax_send(move || {
        let main_window = main_weak.upgrade().unwrap();
        #[cfg(feature = "std")]
        {
            let fax_number = main_window.get_fax_number().to_string();
            println!("Sending a fax to {}", fax_number);
        }
        main_window.set_fax_number(slint::SharedString::default());
    });

    main_window.on_quit(move || {
        #[cfg(all(feature = "std", not(target_arch = "wasm32")))]
        std::process::exit(0);
    });

    main_window.run();
}
