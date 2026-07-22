// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#![cfg(any(target_os = "android", target_arch = "wasm32"))]

pub mod ui {
    slint::include_modules!();
}

mod app_main;
mod weather;

use crate::app_main::AppHandler;

// Android
#[cfg(target_os = "android")]
use {
    crate::android_activity::{MainEvent, PollEvent},
    core::cell::RefCell,
    slint::android::android_activity,
    std::rc::Rc,
};

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: slint::android::AndroidApp) -> Result<(), slint::PlatformError> {
    android_logger::init_once(android_logger::Config::default().with_max_level(
        if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info },
    ));

    let app_handler = Rc::new(RefCell::new(AppHandler::new()));

    // initialize android before creating main window
    slint::android::init_with_event_listener(android_app, {
        let app_handler = app_handler.clone();
        move |event| match event {
            PollEvent::Main(main_event) => match main_event {
                MainEvent::Start => {
                    app_handler.borrow().reload();
                }
                MainEvent::Resume { .. } => {
                    app_handler.borrow().reload();
                }
                MainEvent::SaveState { .. } => {
                    app_handler.borrow().save();
                }
                _ => {}
            },
            _ => {}
        }
    })
    .unwrap();

    {
        // create main window here
        let mut app_handler = app_handler.borrow_mut();
        app_handler.initialize_ui();
    }

    let app_handler = app_handler.borrow();
    app_handler.run()
}

// Wasm
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn main() {
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    console_log::init_with_level(if cfg!(debug_assertions) {
        log::Level::Debug
    } else {
        log::Level::Info
    })
    .ok();

    let mut app_handler = AppHandler::new();
    app_handler.initialize_ui();

    let res = app_handler.run();
    app_handler.save();

    match res {
        Ok(()) => {}
        Err(e) => {
            log::error!("Runtime error: {}", e);
        }
    }
}
