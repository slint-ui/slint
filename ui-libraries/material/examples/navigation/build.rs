// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    unsafe {
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    slint_build::compile("ui/app.slint").expect("Slint build failed");
}
