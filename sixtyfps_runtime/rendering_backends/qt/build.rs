// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

fn main() {
    println!("cargo:rerun-if-env-changed=SIXTYFPS_NO_QT");
    if std::env::var("TARGET").map_or(false, |t| t.starts_with("wasm"))
        || std::env::var("SIXTYFPS_NO_QT").is_ok()
    {
        println!("cargo:rustc-cfg=no_qt");
        return;
    }
    if std::env::var("DEP_QT_FOUND").unwrap() != "1" {
        println!("cargo:rustc-cfg=no_qt");
        println!(
            "cargo:warning=Could not find a Qt installation. The Qt backend will not be functional. \
            See https://github.com/sixtyfpsui/sixtyfps/blob/master/docs/install_qt.md for more info"
        );
        return;
    }
    let qt_version = std::env::var("DEP_QT_VERSION").unwrap();
    if !qt_version.starts_with("5.15") && !qt_version.starts_with("6.") {
        println!("cargo:rustc-cfg=no_qt");
        println!(
            "cargo:warning=Qt {} is not supported, you need at least Qt 5.15. The Qt backend will not be functional. \
             See https://github.com/sixtyfpsui/sixtyfps/blob/master/docs/install_qt.md for more info",
            qt_version
        );
        return;
    }

    let mut config = cpp_build::Config::new();

    config.flag_if_supported("-std=c++17");
    config.flag_if_supported("/std:c++17");
    // Make sure that MSVC reports the correct value for __cplusplus
    config.flag_if_supported("/Zc:__cplusplus");
    // Make sure that MSVC is using utf-8 for source encoding
    // Ref: https://docs.microsoft.com/en-us/cpp/build/reference/utf-8-set-source-and-executable-character-sets-to-utf-8
    config.flag_if_supported("/utf-8");

    let qt_library_path = std::env::var("DEP_QT_LIBRARY_PATH").unwrap();

    if cfg!(target_os = "macos") {
        config.flag("-F");
        config.flag(&qt_library_path);
    }
    config.include(std::env::var("DEP_QT_INCLUDE_PATH").unwrap()).build("lib.rs");

    println!("cargo:rerun-if-changed=qt_window.rs");
    println!("cargo:rerun-if-changed=qt_widgets.rs");
    println!("cargo:rerun-if-changed=qt_widgets/button.rs");
    println!("cargo:rerun-if-changed=qt_widgets/checkbox.rs");
    println!("cargo:rerun-if-changed=qt_widgets/combobox.rs");
    println!("cargo:rerun-if-changed=qt_widgets/lineedit.rs");
    println!("cargo:rerun-if-changed=qt_widgets/listviewitem.rs");
    println!("cargo:rerun-if-changed=qt_widgets/scrollview.rs");
    println!("cargo:rerun-if-changed=qt_widgets/slider.rs");
    println!("cargo:rerun-if-changed=qt_widgets/spinbox.rs");
    println!("cargo:rerun-if-changed=qt_widgets/stylemetrics.rs");
    println!("cargo:rerun-if-changed=qt_widgets/tabwidget.rs");
    println!("cargo:rerun-if-changed=lib.rs");
    println!("cargo:SUPPORTS_NATIVE_STYLE=1");

    // Cargo doesn't support implicit transitive link flags for crates (https://github.com/rust-lang/cargo/issues/9554 | https://github.com/rust-lang/cargo/issues/9562 | https://github.com/sixtyfpsui/sixtyfps/issues/566).
    // Instead of requiring Rust apps to have Qt backend specific code, propagate the needed rpath link options via a DEP_ variable to the default backend, which
    // can write it to a file for use by sixtyfps-build.
    // For C++ apps that's not an issue because we create a cdylib, qttypes emits `rustc-cdylib-link-arg` and that *is* propagated.
    // This also means that the Qt backend cannot be combined with another backend that also writes to this file. The GL backend doesn't, but the MCU
    // backend might/will.
    if std::env::var("CARGO_CFG_TARGET_FAMILY").as_ref().map(|s| s.as_ref()) == Ok("unix") {
        println!("cargo:LINK_ARGS=-Wl,-rpath,{}", &qt_library_path)
    }
}
