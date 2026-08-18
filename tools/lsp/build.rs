// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Mirror the `/STACK` setting from `.cargo/config.toml`, which is not shipped
/// in the published crate.
fn bump_windows_stack_size() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8000000");
    }
}

/// Link against the downloaded Sparkle.framework. The bundle build sets
/// `SLINT_SPARKLE_BUNDLED` because it brings its own rpath into
/// `Contents/Frameworks`, and a checkout path in a shipped binary could
/// shadow that copy.
fn link_sparkle() {
    println!("cargo::rerun-if-env-changed=SLINT_SPARKLE_BUNDLED");
    println!("cargo::rerun-if-env-changed=SPARKLE_FRAMEWORK_DIR");

    let Some(dir) = std::env::var_os("SPARKLE_FRAMEWORK_DIR").filter(|dir| !dir.is_empty()) else {
        panic!(
            "\n\nSparkle.framework not found. Set SPARKLE_FRAMEWORK_DIR to the \
             directory holding it. In a Slint checkout, scripts/download-sparkle.sh \
             downloads it to the repository root.\n"
        );
    };
    let dir = dir.to_string_lossy();

    println!("cargo::rustc-link-search=framework={dir}");
    // -needed_framework, because the classes are looked up through the
    // Objective-C runtime: nothing references a symbol, so a plain
    // -framework gets dead-stripped back out and the classes are missing.
    println!("cargo::rustc-link-arg=-Wl,-needed_framework,Sparkle");

    if std::env::var_os("SLINT_SPARKLE_BUNDLED").is_none() {
        println!("cargo::rustc-link-arg=-Wl,-rpath,{dir}");
    }
}

fn main() {
    bump_windows_stack_size();

    if cfg!(feature = "sparkle-updater")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        link_sparkle();
    }

    // Safety: there are no other threads at this point
    unsafe {
        // Make the compiler handle ComponentContainer:
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    #[cfg(feature = "preview-engine")]
    slint_build::compile("ui/main.slint").unwrap();
}
