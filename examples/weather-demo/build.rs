// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::env;

fn main() {
    slint_build::compile("ui/main.slint").unwrap();
}
