// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore execpath parseable

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

/// Resolves package import paths for dependencies in Cargo and NPM workspaces.
pub fn package_import_paths(path: &std::path::Path) -> Option<HashMap<String, std::path::PathBuf>> {
    let mut p = Some(path.clone());
    while let Some(path) = p {
        if path.join("Cargo.toml").exists() {
            return crate::workspace::cargo_package_imports(&path);
        }
        if path.join("package.json").exists() {
            return crate::workspace::npm_package_imports(&path);
        }
        p = path.parent();
    }
    None
}

/// Resolves package import paths for dependencies in a Cargo workspace.
///
/// Specified in `Cargo.toml`:
/// ```toml
/// [package.metadata.slint]
/// export = "ui/lib.slint"
/// ```
fn cargo_package_imports(path: &Path) -> Option<HashMap<String, PathBuf>> {
    let metadata = MetadataCommand::new().current_dir(&path).exec().ok()?;
    Some(metadata.packages.into_iter().fold(HashMap::new(), |mut paths, package| {
        let manifest_dir = package.manifest_path.parent().unwrap();
        if let Some(export_path) = slint_export_path(manifest_dir.as_std_path(), &package.metadata)
        {
            paths.insert(package.name, export_path);
        }
        paths
    }))
}

/// Resolves package import paths for dependencies in an NPM workspace.
///
/// Specified in `package.json`:
/// ```json
/// {
///     "slint": {
///         "export": "ui/lib.slint"
///     }
/// }
/// ```
fn npm_package_imports(path: &Path) -> Option<HashMap<String, PathBuf>> {
    let npm = std::env::var("npm_execpath").unwrap_or("npm".into());
    let output = Command::new(npm)
        .arg("ls")
        .arg("--all")
        .arg("--parseable")
        .current_dir(&path)
        .output()
        .ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    Some(stdout.lines().map(Path::new).fold(HashMap::new(), |mut paths, path| {
        if let Ok(manifest_file) = File::open(path.join("package.json")) {
            if let Ok(json) = serde_json::from_reader(manifest_file) {
                if let Some(export_path) = slint_export_path(&path, &json) {
                    let name = path.file_name().unwrap().to_str().unwrap().to_string();
                    paths.insert(name, export_path);
                }
            }
        }
        paths
    }))
}

fn slint_export_path(path: &Path, json: &serde_json::Value) -> Option<PathBuf> {
    let export_path = json.get("slint").and_then(|s| s.get("export"));
    match export_path {
        Some(serde_json::Value::String(s)) => path.join(s).into(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slint_export_path() {
        let path = Path::new("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3");
        assert_eq!(slint_export_path(path, &serde_json::Value::Null), None);
        assert_eq!(
            slint_export_path(path, &serde_json::json!({"slint": {"export": "foo/bar.slint"}})),
            Some(path.join("foo/bar.slint").to_path_buf())
        );
    }
}
