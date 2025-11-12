// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "sw-renderer")]
slint::slint! {
    export { MainWindow } from "../demo.slint";
}

#[cfg(not(feature = "sw-renderer"))]
slint::slint! {
    export { MainWindow } from "../demo.slint";
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let app = MainWindow::new().expect("MainWindow::new() failed");

    app.run().expect("MainWindow::run() failed");
}
