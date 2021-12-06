/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::path::Path;

fn main() {
    // This is part code tries to detect automatically what default style to use and tries to
    // use the native style automatically if Qt is available.
    //
    // The way this work is this
    // 1. `qttypes`' crate's build script already detects Qt and set the DEP_QT_VERSION
    // 2. The qt rendering backend's build script will check if the qttype crates found Qt and
    //    look at the SIXTYFPS_NO_QT env variable, and sets the DEP_SIXTYFPS_RENDERING_BACKEND_QT_SUPPORTS_NATIVE_STYLE
    //    env variable so that the default rendering backend can know if Qt was there.
    // 3. here, in the default rendering backend, we know if we depends on the qt backend and if it
    //    has set the DEP_SIXTYFPS_RENDERING_BACKEND_QT_SUPPORTS_NATIVE_STYLE env variable.
    //    We then write a file in the build directory with the default style that depends on the
    //    Qt availability
    // 4a. When using the sixtyfps-build crate from a build script, it will be able to read this file
    //     from `sixtyfps_build::compile_with_config`
    // 4b. Same when using the `sixtyfps!` macro,

    let has_native_style = std::env::var("DEP_SIXTYFPS_RENDERING_BACKEND_QT_SUPPORTS_NATIVE_STYLE")
        .unwrap_or_default()
        == "1";

    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    // out_dir is something like
    // <target_dir>/build/sixtyfps-rendering-backend-default-1fe5c4ab61eb0584/out
    // and we want to write to a common directory, so write in the build/ dir
    let target_path =
        Path::new(&out_dir).parent().unwrap().parent().unwrap().join("SIXTYFPS_DEFAULT_STYLE.txt");
    std::fs::write(target_path, if has_native_style { b"native\n" as &[u8] } else { b"fluent\n" })
        .unwrap();
}
