// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile("../ui/printerdemo.slint").unwrap();
    slint_build::print_rustc_flags().unwrap();
}
