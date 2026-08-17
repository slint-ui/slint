// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// Mirror the `/STACK` setting from `.cargo/config.toml`, which is not shipped
/// in the published crate.
fn bump_windows_stack_size() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8000000");
    }
}

/// Sparkle is linked as `@rpath/Sparkle.framework`, so a binary that isn't run
/// from an app bundle needs an rpath pointing at the downloaded framework.
/// The bundle build sets `SLINT_SPARKLE_BUNDLED` because it copies the
/// framework into `Contents/Frameworks` and brings its own rpath, and because
/// a checkout path baked into a shipped binary could shadow that copy.
fn add_sparkle_rpath() {
    println!("cargo::rerun-if-env-changed=SLINT_SPARKLE_BUNDLED");
    println!("cargo::rerun-if-env-changed=SPARKLE_FRAMEWORK_DIR");

    if std::env::var_os("SLINT_SPARKLE_BUNDLED").is_some() {
        return;
    }

    let Some(dir) = std::env::var_os("SPARKLE_FRAMEWORK_DIR").filter(|dir| !dir.is_empty()) else {
        // The sparklers build script reports the missing framework already.
        return;
    };

    println!("cargo::rustc-link-arg=-Wl,-rpath,{}", dir.to_string_lossy());
}

fn main() {
    bump_windows_stack_size();

    if cfg!(feature = "sparkle-updater")
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos")
    {
        add_sparkle_rpath();
    }

    // Safety: there are no other threads at this point
    unsafe {
        // Make the compiler handle ComponentContainer:
        std::env::set_var("SLINT_ENABLE_EXPERIMENTAL_FEATURES", "1");
    }
    #[cfg(feature = "preview-engine")]
    slint_build::compile("ui/main.slint").unwrap();
}
