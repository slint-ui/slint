// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    #[cfg(feature = "springboard")]
    slint_build::compile_with_config(
        "springboard/springboard.slint",
        slint_build::CompilerConfiguration::new()
            .with_debug_info(true)
            .as_library("springboard")
            .rust_module("springboard_ui"),
    )
    .unwrap();
}
