// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::path::Path;
mod cbindgen;

fn main() -> Result<(), anyhow::Error> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();

    // Go from $root/api/cpp down to $root
    let root_dir = Path::new(&manifest_dir).ancestors().nth(2).expect(&format!(
        "Failed to locate root directory, relative to {}",
        manifest_dir.to_string_lossy()
    ));

    let output_dir = std::env::var_os("SLINT_GENERATED_INCLUDE_DIR").unwrap_or_else(|| {
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated_include").into()
    });
    let output_dir = Path::new(&output_dir);

    println!("cargo:GENERATED_INCLUDE_DIR={}", output_dir.display());

    let include_interpreter = std::env::var("CARGO_FEATURE_SLINT_INTERPRETER").is_ok();
    let dependencies = cbindgen::gen_all(&root_dir, &output_dir, include_interpreter)?;
    for path in dependencies {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(())
}
