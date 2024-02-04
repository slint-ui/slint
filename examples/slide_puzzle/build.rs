// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    let config = slint_build::CompilerConfiguration::new()
        .with_user_data_type("std::cell::RefCell<crate::AppState>");
    slint_build::compile_with_config("slide_puzzle.slint", config).unwrap();
}
