// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::cbindgen::EnabledFeatures;
use std::path::Path;
mod cbindgen;

fn main() -> Result<(), anyhow::Error> {
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap();

    // Go from $root/api/cpp down to $root
    let root_dir = Path::new(&manifest_dir).ancestors().nth(2).expect(&format!(
        "Failed to locate root directory, relative to {}",
        manifest_dir.to_string_lossy()
    ));

    println!("cargo:rerun-if-env-changed=SLINT_GENERATED_INCLUDE_DIR");
    let output_dir = std::env::var_os("SLINT_GENERATED_INCLUDE_DIR").unwrap_or_else(|| {
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("generated_include").into()
    });
    let output_dir = Path::new(&output_dir);

    println!("cargo:GENERATED_INCLUDE_DIR={}", output_dir.display());

    let enabled_features = EnabledFeatures::from_env();

    let dependencies = cbindgen::gen_all(root_dir, output_dir, enabled_features)?;
    for path in dependencies {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    Ok(())
}
