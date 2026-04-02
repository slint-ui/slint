// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::include_modules!();

fn main() {
    let my_tray = App::new().unwrap();
    let _tray = slint::private_unstable_api::create_system_tray(slint::system_tray::Params {
        icon: &my_tray.get_icon(),
        tooltip: "my-tray",
    })
    .unwrap();
    slint::run_event_loop().unwrap();
}
