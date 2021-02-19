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

use sixtyfps::Model;
use std::rc::Rc;

sixtyfps::include_modules!();

struct PrinterQueue {
    data: Rc<sixtyfps::VecModel<PrinterQueueItem>>,
}

impl PrinterQueue {
    fn push_job(&self, title: sixtyfps::SharedString) {
        self.data.push(PrinterQueueItem {
            status: "WAITING...".into(),
            progress: 0,
            title,
            owner: env!("CARGO_PKG_AUTHORS").into(),
            pages: 1,
            size: "100kB".into(),
            submission_date: chrono::Local::now().format("%H:%M:%S %d/%m/%Y").to_string().into(),
        })
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    sixtyfps::register_font_from_memory(include_bytes!("../ui/fonts/NotoSans-Regular.ttf"))
        .expect("error registering noto sans regular");
    sixtyfps::register_font_from_memory(include_bytes!("../ui/fonts/NotoSans-Bold.ttf"))
        .expect("error registering noto sans bold");

    let main_window = MainWindow::new();
    main_window.set_ink_levels(sixtyfps::VecModel::from_slice(&[
        InkLevel { color: sixtyfps::Color::from_rgb_u8(0, 255, 255), level: 0.40 },
        InkLevel { color: sixtyfps::Color::from_rgb_u8(255, 0, 255), level: 0.20 },
        InkLevel { color: sixtyfps::Color::from_rgb_u8(255, 255, 0), level: 0.50 },
        InkLevel { color: sixtyfps::Color::from_rgb_u8(0, 0, 0), level: 0.80 },
    ]));

    let default_queue: Vec<PrinterQueueItem> = main_window.get_printer_queue().iter().collect();
    let printer_queue =
        Rc::new(PrinterQueue { data: Rc::new(sixtyfps::VecModel::from(default_queue)) });
    main_window.set_printer_queue(sixtyfps::ModelHandle::new(printer_queue.data.clone()));

    main_window.on_quit(move || {
        #[cfg(not(target_arch = "wasm32"))]
        std::process::exit(0);
    });

    main_window.on_start_job(move |title| {
        printer_queue.push_job(title);
    });

    main_window.run();
}
