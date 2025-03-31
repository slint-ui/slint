// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use slint::Model;
use std::rc::Rc;

slint::include_modules!();

/// Returns the current time formatted as a string
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
            status: "waiting".into(),
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

    let main_window = MainWindow::new().unwrap();
    main_window.set_ink_levels(
        [
            InkLevel { color: slint::Color::from_rgb_u8(0, 255, 255), level: 0.40 },
            InkLevel { color: slint::Color::from_rgb_u8(255, 0, 255), level: 0.20 },
            InkLevel { color: slint::Color::from_rgb_u8(255, 255, 0), level: 0.50 },
            InkLevel { color: slint::Color::from_rgb_u8(0, 0, 0), level: 0.80 },
        ]
        .into(),
    );

    let default_queue: Vec<PrinterQueueItem> =
        main_window.global::<PrinterQueue>().get_printer_queue().iter().collect();
    let printer_queue = Rc::new(PrinterQueueData {
        data: Rc::new(slint::VecModel::from(default_queue)),
        print_progress_timer: Default::default(),
    });
    main_window.global::<PrinterQueue>().set_printer_queue(printer_queue.data.clone().into());

    main_window.on_quit(move || {
        #[cfg(not(target_arch = "wasm32"))]
        std::process::exit(0);
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_start_job(move |title| {
        printer_queue_copy.push_job(title);
    });

    let printer_queue_copy = printer_queue.clone();
    main_window.global::<PrinterQueue>().on_cancel_job(move |idx| {
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
                    top_item.status = "printing".into();
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

    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    {
        slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/../lang/"));
        main_window.global::<PrinterSettings>().on_change_language(|l| {
            let lang = match l {
                0 => "en",
                1 => "fr",
                _ => return,
            };
            std::env::set_var("LANGUAGE", lang);
            slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/../lang/"));
        })
    }
    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    main_window.global::<PrinterSettings>().on_change_language(|l| {
        let lang = match l {
            0 => "en",
            1 => "fr",
            _ => return,
        };
        slint::select_bundled_translation(lang).unwrap();
    });

    main_window.run().unwrap();
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: slint::android::AndroidApp) {
    slint::android::init(app).unwrap();
    main()
}
