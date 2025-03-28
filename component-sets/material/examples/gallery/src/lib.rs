// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

slint::include_modules!();

fn ui() -> MainWindow {
    let ui = MainWindow::new().unwrap();
    navigation_view(&ui);
    ui
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let ui = ui();
    ui.run().unwrap();
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(android_app: slint::android::AndroidApp) {
    slint::android::init(android_app).unwrap();
    let ui = ui();
    ui.run().unwrap();
}

fn navigation_view(ui: &MainWindow) {
    let adapter = NavigationViewAdapter::get(ui);

    adapter.on_search(|text| {
        println!("Search {}", text);
    });
}
