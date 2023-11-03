// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

fn main() {
    // Make the compiler handle ComponentContainer:
    std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    #[cfg(feature = "preview-engine")]
    slint_build::compile("ui/main.slint").unwrap();
}
