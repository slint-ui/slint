// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::env;

fn main() {
    env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "true");

    slint_build::compile("ui/main.slint").unwrap();
}
