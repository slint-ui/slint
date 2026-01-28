// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    // Safety: there are no other threads at this point
    unsafe {
        // Make the compiler handle ComponentContainer:
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    slint_build::compile("ui/index.slint").unwrap();
}
