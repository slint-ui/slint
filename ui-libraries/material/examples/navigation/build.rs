// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

fn main() {
    // The `navigator` construct is experimental. slint-build has no typed setter
    // for it, but `CompilerConfiguration::new()` reads this env var, and the
    // compile runs in this same build-script process, so setting it here enables
    // experimental for exactly this crate's `.slint` compile. Edition 2024 makes
    // `set_var` unsafe; it is sound here because the build script is still
    // single-threaded when this runs.
    unsafe {
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    slint_build::compile("ui/app.slint").expect("Slint build failed");
}
