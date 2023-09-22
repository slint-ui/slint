// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore parseable

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

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
        let manifest_dir = package.manifest_path.parent().unwrap();
        let include_paths =
            include_paths_from_metadata(manifest_dir.as_std_path(), &package.metadata);
        paths.insert(package.name, include_paths);
        paths
    }))
}

/// Resolves include paths for an NPM workspace.
///
/// Defaults to the manifest directory of each dependency unless specified in
/// `package.json`:
/// ```json
/// {
///     "slint": {
///         "include_path": "ui" // or ["ui", "fonts", "images"]
///     }
/// }
/// ```
pub fn npm_include_paths(path: &Path) -> Option<HashMap<String, Vec<PathBuf>>> {
    let output = Command::new("npm")
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
                let include_paths = include_paths_from_metadata(&path, &json);
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                paths.insert(name, include_paths);
            }
        }
        paths
    }))
}

fn include_paths_from_metadata(path: &Path, json: &serde_json::Value) -> Vec<PathBuf> {
    let include_path = json.get("slint").and_then(|s| s.get("include_path"));
    match include_path {
        Some(serde_json::Value::String(s)) => vec![path.join(s).into()],
        Some(serde_json::Value::Array(a)) => {
            a.iter().map(|s| path.join(s.as_str().unwrap()).into()).collect()
        }
        _ => vec![path.into()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_include_paths_from_metadata() {
        let path = Path::new("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3");
        assert_eq!(
            include_paths_from_metadata(path, &serde_json::Value::Null),
            vec![path.to_path_buf()]
        );
        assert_eq!(
            include_paths_from_metadata(
                path,
                &serde_json::json!({"slint": {"include_path": "foo"}})
            ),
            vec![path.join("foo").to_path_buf()]
        );
        assert_eq!(
            include_paths_from_metadata(
                path,
                &serde_json::json!({"slint": {"include_path": ["foo", "bar"]}})
            ),
            vec![path.join("foo").to_path_buf(), path.join("bar").to_path_buf(),]
        );
    }
}
