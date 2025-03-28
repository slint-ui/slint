// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::env;
use std::path::PathBuf;

use std::collections::HashMap;

/// Provides the `material` library paths used by slint-build to make them accessible through `@slint/material.slint` in Slint.
///
/// ```ignore
/// fn main() {
///   slint_build::compile_with_config(
///          "ui/main.slint",
///          slint_build::CompilerConfiguration::new().with_library_paths(slint_material::import_path()),
///      )
///      .unwrap();
/// }
/// ```
///
/// Import `slint-material` library in `ui/main.slint`
///
/// ```slint
/// import { FilledButton } from "@slint/material.slint";
/// ````
pub fn import_path() -> HashMap<String, PathBuf> {
    let mut import_paths = HashMap::new();
    import_paths.insert("slint".to_string(), PathBuf::from(env!("CARGO_MANIFEST_DIR")));
    import_path
}
