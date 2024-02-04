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

pub struct PrinterQueueData {
    data: Rc<slint::VecModel<PrinterQueueItem>>,
    print_progress_timer: slint::Timer,
}

impl PrinterQueueData {
    fn push_job(&self, title: slint::SharedString) {
        self.data.push(PrinterQueueItem {
            status: JobStatus::Waiting,
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

    let printer_queue = PrinterQueueData {
        data: Rc::new(slint::VecModel::default()),
        print_progress_timer: Default::default(),
    };
    let main_window = MainWindow::new(printer_queue).unwrap();
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
    main_window.user_data().data.set_vec(default_queue);
    main_window
        .global::<PrinterQueue>()
        .set_printer_queue(main_window.user_data().data.clone().into());

    main_window.on_quit(move || {
        #[cfg(not(target_arch = "wasm32"))]
        std::process::exit(0);
    });

    let weak = main_window.as_weak();
    main_window.global::<PrinterQueue>().on_start_job(move |title| {
        weak.unwrap().user_data().push_job(title);
    });

    let weak = main_window.as_weak();
    main_window.global::<PrinterQueue>().on_cancel_job(move |idx| {
        weak.unwrap().user_data().data.remove(idx as usize);
    });

    let weak = main_window.as_weak();
    main_window.user_data().print_progress_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            if let Some(main_window) = weak.upgrade() {
                if main_window.user_data().data.row_count() > 0 {
                    let mut top_item = main_window.user_data().data.row_data(0).unwrap();
                    top_item.progress += 1;
                    top_item.status = JobStatus::Waiting;
                    if top_item.progress > 100 {
                        main_window.user_data().data.remove(0);
                        if main_window.user_data().data.row_count() == 0 {
                            return;
                        }
                        top_item = main_window.user_data().data.row_data(0).unwrap();
                    }
                    main_window.user_data().data.set_row_data(0, top_item);
                } else {
                    // FIXME: stop this timer?
                }
            }
        },
    );

    main_window.run().unwrap();
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: i_slint_backend_android_activity::AndroidApp) {
    slint::platform::set_platform(Box::new(
        i_slint_backend_android_activity::AndroidPlatform::new(app),
    ))
    .unwrap();
    main()
}
