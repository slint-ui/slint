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

    if cfg!(target_os = "macos") {
        config.flag("-F");
        config.flag(&std::env::var("DEP_QT_LIBRARY_PATH").unwrap());
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
}
