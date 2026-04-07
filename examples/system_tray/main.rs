// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

fn main() {
    let _tray = ExampleTray::new().unwrap();
    slint::run_event_loop().unwrap();
}
