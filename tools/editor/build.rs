// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint_build::CompilerConfiguration;

fn main() {
    // Some tests use the ElementHandle API, which requires debug info
    slint_build::compile_with_config(".", CompilerConfiguration::new().with_debug_info(true))
        .unwrap();
    // TODO: Find a way to configure the gallery's slint! macro from the project file.
    // slint-build configures only its own compilation.
    println!("cargo:rustc-env=SLINT_ENABLE_EXPERIMENTAL_FEATURES=1");
}
