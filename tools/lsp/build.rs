// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Mirror the `/STACK` setting from `.cargo/config.toml`, which is not shipped
/// in the published crate.
fn bump_windows_stack_size() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8000000");
    }
}

fn main() {
    bump_windows_stack_size();

    // Safety: there are no other threads at this point
    unsafe {
        // Make the compiler handle ComponentContainer:
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    #[cfg(feature = "preview-engine")]
    slint_build::compile("ui/main.slint").unwrap();
}
