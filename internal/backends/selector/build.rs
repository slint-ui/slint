// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::Path;

fn main() {
    println!("cargo:rustc-check-cfg=cfg(no_qt)");

    // This is part code tries to detect automatically what default style to use and tries to
    // use the native style automatically if Qt is available.
    //
    // The way this work is this
    // 1. `qttypes`' crate's build script already detects Qt and set the DEP_QT_VERSION
    // 2. The qt rendering backend's build script will check if the qttype crates found Qt and
    //    look at the SLINT_NO_QT env variable, and sets the DEP_i_slint_backend_qt_SUPPORTS_NATIVE_STYLE
    //    env variable so that the default rendering backend can know if Qt was there.
    // 3. here, in the default rendering backend, we know if we depends on the qt backend and if it
    //    has set the DEP_i_slint_backend_qt_SUPPORTS_NATIVE_STYLE env variable.
    //    We then write a file in the build directory with the default style that depends on the
    //    Qt availability
    // 4a. When using the slint-build crate from a build script, it will be able to read this file
    //     from `slint_build::compile_with_config`
    // 4b. Same when using the `slint!` macro,

    let has_native_style =
        std::env::var("DEP_I_SLINT_BACKEND_QT_SUPPORTS_NATIVE_STYLE").unwrap_or_default() == "1";

    if !has_native_style {
        println!("cargo:rustc-cfg=no_qt");
    }

    let style = i_slint_common::get_native_style(
        has_native_style,
        &std::env::var("TARGET").unwrap_or_default(),
    );
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    // out_dir is something like
    // <target_dir>/build/i-slint-backend-selector-1fe5c4ab61eb0584/out
    // and we want to write to a common directory, so write in the build/ dir
    let target_path =
        Path::new(&out_dir).parent().unwrap().parent().unwrap().join("SLINT_DEFAULT_STYLE.txt");
    std::fs::write(target_path, style).unwrap();
}
