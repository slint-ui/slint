// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    // Enable experimental features for ComponentContainer
    // This is required for the component-factory type
    std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");

    slint_build::compile("ui/main.slint").unwrap();
}
