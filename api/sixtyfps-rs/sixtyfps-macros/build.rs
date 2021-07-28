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
    // This file is written by the sixtyfps-rendering-backend-default's built script. At this point
    // the build script might not have ran yet, but we just need to pass the path to the build directory
    // to the macro crate itself.
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let target_path =
        Path::new(&out_dir).parent().unwrap().parent().unwrap().join("SIXTYFPS_DEFAULT_STYLE.txt");
    println!("cargo:rustc-env=SIXTYFPS_DEFAULT_STYLE_PATH={}", target_path.display());
}
