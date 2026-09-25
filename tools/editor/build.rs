// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_build::CompilerConfiguration;

fn main() {
    // Safety: there are no other threads at this point
    unsafe {
        // Make the compiler handle ComponentContainer:
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }

    // Some tests use the ElementHandle API, which requires debug info
    slint_build::compile_with_config(
        "ui/main.slint",
        CompilerConfiguration::new().with_debug_info(true),
    )
    .unwrap();
}
