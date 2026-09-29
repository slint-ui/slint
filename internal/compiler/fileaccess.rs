// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::source_path::SourcePath;
use std::borrow::Cow;
use std::fs;

#[derive(Clone)]
pub struct VirtualFile {
    pub canon_path: SourcePath,
    pub builtin_contents: Option<&'static [u8]>,
}

impl VirtualFile {
    pub fn read(&self) -> Cow<'static, [u8]> {
        self.try_read().unwrap()
    }

    pub fn try_read(&self) -> std::io::Result<Cow<'static, [u8]>> {
        match self.builtin_contents {
            Some(static_data) => Ok(Cow::Borrowed(static_data)),
            None => self.canon_path.read().map(Cow::Owned),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.builtin_contents.is_some()
    }
}

pub fn styles() -> Vec<&'static str> {
    builtin_library::styles()
}

pub fn load_file(path: &SourcePath) -> Option<VirtualFile> {
    match path {
        SourcePath::Builtin(builtin_path) => builtin_library::load_builtin_file(builtin_path),
        SourcePath::File(path) => path.exists().then(|| {
            let path =
                crate::pathutils::join(&std::env::current_dir().ok().unwrap_or_default(), path);
            VirtualFile { canon_path: SourcePath::File(path), builtin_contents: None }
        }),
        SourcePath::Url(_) => None,
    }
}

#[test]
fn test_load_file() {
    let builtin =
        load_file(&SourcePath::new("builtin:/foo/../common/./MadeWithSlint-logo-dark.svg"))
            .unwrap();
    assert!(builtin.is_builtin());
    assert_eq!(builtin.canon_path.to_string(), "builtin:/common/MadeWithSlint-logo-dark.svg");
    assert!(load_file(&SourcePath::new("https://slint.dev/Cargo.toml")).is_none());

    let dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap().to_string_lossy().to_string();
    let dir_path = std::path::PathBuf::from(dir);

    let non_existing = dir_path.join("XXXCargo.tomlXXX");
    assert!(load_file(&SourcePath::new(non_existing)).is_none());

    assert!(dir_path.exists()); // We need some existing path for all the rest

    let cargo_toml = dir_path.join("Cargo.toml");
    let abs_cargo_toml = load_file(&SourcePath::new(&cargo_toml)).unwrap();
    assert!(!abs_cargo_toml.is_builtin());
    assert!(abs_cargo_toml.canon_path.to_url().is_some());
    assert!(abs_cargo_toml.canon_path.as_native_path().unwrap().exists());

    let current = std::env::current_dir().unwrap();
    assert!(current.ends_with("compiler")); // This test is run in .../internal/compiler

    let cargo_toml = std::path::PathBuf::from("./tests/../Cargo.toml");
    let rel_cargo_toml = load_file(&SourcePath::new(&cargo_toml)).unwrap();
    assert!(!rel_cargo_toml.is_builtin());
    assert!(rel_cargo_toml.canon_path.to_url().is_some());
    assert!(rel_cargo_toml.canon_path.as_native_path().unwrap().exists());

    assert_eq!(abs_cargo_toml.canon_path, rel_cargo_toml.canon_path);
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

    use super::{SourcePath, VirtualFile};

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

    pub(crate) fn load_builtin_file(builtin_path: &str) -> Option<VirtualFile> {
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
        if let &[folder, file] = components.as_slice() {
            let library = widget_library().iter().find(|x| x.0 == folder)?.1;
            library.iter().find_map(|builtin_file| {
                if builtin_file.path == file {
                    Some(VirtualFile {
                        canon_path: SourcePath::Builtin(
                            format!("{folder}/{}", builtin_file.path).into(),
                        ),
                        builtin_contents: Some(builtin_file.contents),
                    })
                } else {
                    None
                }
            })
        } else {
            None
        }
    }
}
