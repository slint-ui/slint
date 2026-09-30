// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::source_path::{SourcePath, clean_path};
use std::fs;

/// Return `true` if `path` has a font file extension supported by Slint
/// (`.ttf`, `.ttc`, or `.otf`).
pub fn is_font_file(path: &str) -> bool {
    path.ends_with(".ttf") || path.ends_with(".ttc") || path.ends_with(".otf")
}

pub fn styles() -> Vec<&'static str> {
    builtin_library::styles()
}

/// Returns the canonical path of the file `path` names, if it exists:
/// the absolute path of a native file, or the builtin file of the style an alias names.
pub fn find_file(path: &SourcePath) -> Option<SourcePath> {
    match path {
        SourcePath::Builtin(builtin_path) => {
            builtin_library::find(builtin_path).map(|(canon_path, _)| canon_path)
        }
        SourcePath::File(path) => path.exists().then(|| {
            SourcePath::File(clean_path(
                &std::path::absolute(path).unwrap_or_else(|_| path.clone()),
            ))
        }),
        SourcePath::Url(_) => None,
    }
}

/// Returns the contents of a builtin file, such as `fluent/button.slint`.
pub fn builtin_contents(builtin_path: &str) -> Option<&'static [u8]> {
    builtin_library::find(builtin_path).map(|(_, contents)| contents)
}

#[test]
fn test_find_file() {
    let builtin =
        find_file(&SourcePath::new("builtin:/foo/../common/./MadeWithSlint-logo-dark.svg"))
            .unwrap();
    assert_eq!(builtin.to_string(), "builtin:/common/MadeWithSlint-logo-dark.svg");
    assert!(builtin.read().is_ok());
    assert!(find_file(&SourcePath::new("https://slint.dev/Cargo.toml")).is_none());

    let dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap().to_string_lossy().to_string();
    let dir_path = std::path::PathBuf::from(dir);

    let non_existing = dir_path.join("XXXCargo.tomlXXX");
    assert!(find_file(&SourcePath::new(non_existing)).is_none());

    assert!(dir_path.exists()); // We need some existing path for all the rest

    let cargo_toml = dir_path.join("Cargo.toml");
    let abs_cargo_toml = find_file(&SourcePath::new(&cargo_toml)).unwrap();
    assert!(abs_cargo_toml.to_url().is_some());
    assert!(abs_cargo_toml.exists());

    let current = std::env::current_dir().unwrap();
    assert!(current.ends_with("compiler")); // This test is run in .../internal/compiler

    let cargo_toml = std::path::PathBuf::from("./tests/../Cargo.toml");
    let rel_cargo_toml = find_file(&SourcePath::new(&cargo_toml)).unwrap();
    assert!(rel_cargo_toml.to_url().is_some());
    assert!(rel_cargo_toml.exists());

    assert_eq!(abs_cargo_toml, rel_cargo_toml);
}

/// Writes a buffer into a file, but only if the content differs from the file content
///
/// Tries to read the destination file first, and only writes the new content if
/// the file didn't exist or the file content differs from the content to write.
/// This avoids unnecessary mtime modification of the file, which caused build
/// systems like Ninja to rebuild other things even though the output of
/// slint-compiler didn't change.
pub fn write_file_if_changed(path: &std::path::Path, content: &[u8]) -> std::io::Result<()> {
    if fs::read(path).is_ok_and(|existing| existing == content) {
        return Ok(());
    }
    fs::write(path, content)
}

mod builtin_library {
    include!(env!("SLINT_WIDGETS_LIBRARY"));

    pub type BuiltinDirectory<'a> = [&'a BuiltinFile<'a>];

    pub struct BuiltinFile<'a> {
        pub path: &'a str,
        pub contents: &'static [u8],
    }

    use super::SourcePath;

    const ALIASES: &[(&str, &str)] = &[
        ("cosmic-light", "cosmic"),
        ("cosmic-dark", "cosmic"),
        ("fluent-light", "fluent"),
        ("fluent-dark", "fluent"),
        ("material-light", "material"),
        ("material-dark", "material"),
        ("cupertino-light", "cupertino"),
        ("cupertino-dark", "cupertino"),
    ];

    pub(crate) fn styles() -> Vec<&'static str> {
        widget_library()
            .iter()
            .filter_map(|(style, directory)| {
                if directory.iter().any(|f| f.path == "std-widgets.slint") {
                    Some(*style)
                } else {
                    None
                }
            })
            .chain(ALIASES.iter().map(|x| x.0))
            .collect()
    }

    /// The canonical path and the contents of a builtin file.
    pub(crate) fn find(builtin_path: &str) -> Option<(SourcePath, &'static [u8])> {
        let mut components = Vec::new();
        for part in builtin_path.split('/').filter(|part| !part.is_empty()) {
            if part == ".." {
                components.pop();
            } else if part != "." {
                components.push(part);
            }
        }
        if let Some(f) = components.first_mut()
            && let Some((_, x)) = ALIASES.iter().find(|x| x.0 == *f)
        {
            *f = x;
        }
        let &[folder, file] = components.as_slice() else { return None };
        let library = widget_library().iter().find(|x| x.0 == folder)?.1;
        let builtin_file = library.iter().find(|builtin_file| builtin_file.path == file)?;
        Some((SourcePath::Builtin(format!("{folder}/{file}").into()), builtin_file.contents))
    }
}
