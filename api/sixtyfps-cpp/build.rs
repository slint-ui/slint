/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use std::path::Path;
mod cbindgen;

fn main() -> Result<(), anyhow::Error> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();

    // Go from $root/api/sixtyfps-cpp down to $root
    let root_dir = Path::new(&manifest_dir).ancestors().nth(2).expect(&format!(
        "Failed to locate root directory, relative to {}",
        manifest_dir.to_string_lossy()
    ));

    let output_dir = std::env::var_os("SIXTYFPS_GENERATED_INCLUDE_DIR").unwrap_or_else(|| {
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated_include").into()
    });
    let output_dir = Path::new(&output_dir);

    println!("cargo:GENERATED_INCLUDE_DIR={}", output_dir.display());

    cbindgen::gen_all(&root_dir, &output_dir)
}
