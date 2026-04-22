// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

slint::slint! {
    export component ExampleTray inherits SystemTray {
        icon: @image-url("favicon-white.png");
    }
}

fn main() {
    let _tray = ExampleTray::new().unwrap();
    slint::run_event_loop().unwrap();
}
