// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use slint::Model;
use std::rc::Rc;

slint::include_modules!();

/// Returns the current time formated as a string
fn current_time() -> slint::SharedString {
    #[cfg(not(target_arch = "wasm32"))]
    return chrono::Local::now().format("%H:%M:%S %d/%m/%Y").to_string().into();
    #[cfg(target_arch = "wasm32")]
    return "".into();
}

struct PrinterQueueData {
    data: Rc<slint::VecModel<PrinterQueueItem>>,
    print_progress_timer: slint::Timer,
}

impl PrinterQueueData {
    fn push_job(&self, title: slint::SharedString) {
        self.data.push(PrinterQueueItem {
            status: "WAITING...".into(),
            progress: 0,
            title,
            owner: env!("CARGO_PKG_AUTHORS").into(),
            pages: 1,
            size: "100kB".into(),
            submission_date: current_time(),
        })
    }
}

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

    let default_queue: Vec<PrinterQueueItem> =
        main_window.global::<PrinterQueue>().get_printer_queue().iter().collect();
    let printer_queue = Rc::new(PrinterQueueData {
        data: Rc::new(slint::VecModel::from(default_queue)),
        print_progress_timer: Default::default(),
    });
    main_window.global::<PrinterQueue>().set_printer_queue(printer_queue.data.clone().into());

    main_window.on_quit(move |_| {
        #[cfg(not(target_arch = "wasm32"))]
        std::process::exit(0);
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_start_job(move |_, title| {
        printer_queue_copy.push_job(title);
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_cancel_job(move |_, idx| {
        printer_queue_copy.data.remove(idx as usize);
    });

    let printer_queue_weak = Rc::downgrade(&printer_queue);
    printer_queue.print_progress_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            if let Some(printer_queue) = printer_queue_weak.upgrade() {
                if printer_queue.data.row_count() > 0 {
                    let mut top_item = printer_queue.data.row_data(0).unwrap();
                    top_item.progress += 1;
                    top_item.status = "PRINTING".into();
                    if top_item.progress > 100 {
                        printer_queue.data.remove(0);
                        if printer_queue.data.row_count() == 0 {
                            return;
                        }
                        top_item = printer_queue.data.row_data(0).unwrap();
                    }
                    printer_queue.data.set_row_data(0, top_item);
                } else {
                    // FIXME: stop this timer?
                }
            }
        },
    );

    main_window.run();
}
