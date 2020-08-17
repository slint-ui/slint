/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
use std::env;
use std::path::PathBuf;
use std::process::Command;

fn qmake_query(var: &str) -> Option<String> {
    let qmake = std::env::var_os("QMAKE").unwrap_or("qmake".into());
    Command::new(qmake).env("QT_SELECT", "qt5").args(&["-query", var]).output().ok().map(|output| {
        String::from_utf8(output.stdout).expect("UTF-8 conversion from ouytput of qmake failed")
    })
}
fn run_cpp_build() {
    if qmake_query("QT_VERSION").is_some() {
        println!("cargo:rustc-cfg=have_qt");

        let qt_include_path = qmake_query("QT_INSTALL_HEADERS").unwrap();
        let qt_library_path = qmake_query("QT_INSTALL_LIBS").unwrap();
        let mut config = cpp_build::Config::new();

        if cfg!(target_os = "macos") {
            config.flag("-F");
            config.flag(qt_library_path.trim());
        }

        config.include(qt_include_path.trim()).build("lib.rs");

        let macos_lib_search = if cfg!(target_os = "macos") { "=framework" } else { "" };
        let macos_lib_framework = if cfg!(target_os = "macos") { "" } else { "5" };

        println!("cargo:rustc-link-search{}={}", macos_lib_search, qt_library_path.trim());
        println!("cargo:rustc-link-lib{}=Qt{}Widgets", macos_lib_search, macos_lib_framework);
        println!("cargo:rustc-link-lib{}=Qt{}Gui", macos_lib_search, macos_lib_framework);
        println!("cargo:rustc-link-lib{}=Qt{}Core", macos_lib_search, macos_lib_framework);
        println!("cargo:rustc-link-lib{}=Qt{}Quick", macos_lib_search, macos_lib_framework);
        println!("cargo:rustc-link-lib{}=Qt{}Qml", macos_lib_search, macos_lib_framework);
    }
}

fn run_cbindgen() {
    let config = cbindgen::Config {
        pragma_once: true,
        include_version: true,
        namespaces: Some(vec!["sixtyfps".into(), "qtstyle".into()]),
        line_length: 100,
        tab_width: 4,
        language: cbindgen::Language::Cxx,
        cpp_compat: true,
        documentation: true,
        export: cbindgen::ExportConfig { ..Default::default() },
        ..Default::default()
    };

    let mut include_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    include_dir.pop();
    include_dir.pop();
    include_dir.pop(); // target/{debug|release}/build/package/out/ -> target/{debug|release}
    include_dir.push("include");

    std::fs::create_dir_all(include_dir.clone()).unwrap();

    let crate_dir = env::var_os("CARGO_MANIFEST_DIR").unwrap();
    cbindgen::Builder::new()
        .with_config(config)
        .with_crate(crate_dir)
        .with_header("#include <sixtyfps_internal.h>")
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(include_dir.join("sixtyfps_qtstyle.h"));
}

fn main() {
    run_cbindgen();
    run_cpp_build();
}
