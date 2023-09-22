// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use cargo_metadata::{MetadataCommand, Package};

/// Resolves include paths for a Cargo workspace.
///
/// Defaults to the manifest directory of each dependency unless specified in
/// `Cargo.toml`:
/// ```toml
/// [package.metadata.slint]
/// include_path = "ui" # or ["ui", "fonts", "images"]
/// ```
pub fn cargo_include_paths(path: &Path) -> Option<HashMap<String, Vec<PathBuf>>> {
    let metadata = MetadataCommand::new().current_dir(&path).exec().ok()?;
    Some(metadata.packages.into_iter().fold(HashMap::new(), |mut paths, package| {
        let include_paths = cargo_include_paths_for_package(&package);
        paths.insert(package.name, include_paths);
        paths
    }))
}

/// Resolves include paths for a cargo package that may provide .slint files.
///
/// Defaults to the manifest directory but can be specified in `Cargo.toml`:
/// ```toml
/// [package.metadata.slint]
/// include_path = "ui" # or ["ui", "fonts", "images"]
/// ```
fn cargo_include_paths_for_package(package: &Package) -> Vec<PathBuf> {
    let manifest_dir = package.manifest_path.parent().unwrap();
    let include_path = package.metadata.get("slint").and_then(|s| s.get("include_path"));
    match include_path {
        Some(serde_json::Value::String(s)) => vec![manifest_dir.join(s).into()],
        Some(serde_json::Value::Array(a)) => {
            a.iter().map(|s| manifest_dir.join(s.as_str().unwrap()).into()).collect()
        }
        _ => vec![manifest_dir.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cargo_include_paths_for_package() {
        let mut json = serde_json::json!({
            "name": "foo",
            "version": "1.2.3",
            "id": "foo 1.2.3 (registry+https://github.com/rust-lang/crates.io-index)",
            "dependencies": [],
            "targets": [],
            "features": {},
            "manifest_path": "/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3/Cargo.toml",
        });

        let pkg: Package = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            cargo_include_paths_for_package(&pkg),
            vec![PathBuf::from("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3")]
        );

        json.as_object_mut()
            .unwrap()
            .insert("metadata".into(), serde_json::json!({"slint": {"include_path": "foo"}}));

        let pkg: Package = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            cargo_include_paths_for_package(&pkg),
            vec![PathBuf::from("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3/foo")]
        );

        json.as_object_mut().unwrap().insert(
            "metadata".into(),
            serde_json::json!({"slint": {"include_path": ["foo", "bar"]}}),
        );

        let pkg: Package = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            cargo_include_paths_for_package(&pkg),
            vec![
                PathBuf::from("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3/foo"),
                PathBuf::from("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3/bar")
            ]
        );
    }
}
