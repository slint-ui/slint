// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

// cSpell: ignore execpath parseable

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

/// Resolves package entry-points for a Cargo workspace.
///
/// Specified in `Cargo.toml`:
/// ```toml
/// [package.metadata.slint]
/// index = "ui/lib.slint"
/// ```
pub fn cargo_entry_points(path: &Path) -> Option<HashMap<String, PathBuf>> {
    let metadata = MetadataCommand::new().current_dir(&path).exec().ok()?;
    Some(metadata.packages.into_iter().fold(HashMap::new(), |mut paths, package| {
        let manifest_dir = package.manifest_path.parent().unwrap();
        if let Some(entry_point) =
            entry_point_from_metadata(manifest_dir.as_std_path(), &package.metadata)
        {
            paths.insert(package.name, entry_point);
        }
        paths
    }))
}

/// Resolves entry-points for an NPM workspace.
///
/// Specified in `package.json`:
/// ```json
/// {
///     "slint": {
///         "index": "ui/lib.slint"
///     }
/// }
/// ```
pub fn npm_entry_point(path: &Path) -> Option<HashMap<String, PathBuf>> {
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
                if let Some(entry_point) = entry_point_from_metadata(&path, &json) {
                    let name = path.file_name().unwrap().to_str().unwrap().to_string();
                    paths.insert(name, entry_point);
                }
            }
        }
        paths
    }))
}

fn entry_point_from_metadata(path: &Path, json: &serde_json::Value) -> Option<PathBuf> {
    let entry_point = json.get("slint").and_then(|s| s.get("index"));
    match entry_point {
        Some(serde_json::Value::String(s)) => path.join(s).into(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entry_point_from_metadata() {
        let path = Path::new("/home/user/.cargo/registry/src/index.crates.io-abc/foo-1.2.3");
        assert_eq!(entry_point_from_metadata(path, &serde_json::Value::Null), None);
        assert_eq!(
            entry_point_from_metadata(
                path,
                &serde_json::json!({"slint": {"index": "foo/bar.slint"}})
            ),
            Some(path.join("foo/bar.slint").to_path_buf())
        );
    }
}
