// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    let config =
        slint_build::CompilerConfiguration::new().with_user_data_type("crate::PrinterQueueData");
    slint_build::compile_with_config("../ui/printerdemo.slint", config).unwrap();
}
