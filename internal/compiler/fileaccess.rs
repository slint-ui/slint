// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::borrow::Cow;

#[derive(Clone)]
pub struct VirtualFile {
    pub canon_path: std::path::PathBuf,
    pub builtin_contents: Option<&'static [u8]>,
}

impl VirtualFile {
    pub fn read(&self) -> Cow<'static, [u8]> {
        match self.builtin_contents {
            Some(static_data) => Cow::Borrowed(static_data),
            None => Cow::Owned(std::fs::read(&self.canon_path).unwrap()),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.builtin_contents.is_some()
    }
}

pub fn styles() -> Vec<&'static str> {
    builtin_library::styles()
}

pub fn load_file(path: &std::path::Path) -> Option<VirtualFile> {
    match path.strip_prefix("builtin:/") {
        Ok(builtin_path) => builtin_library::load_builtin_file(builtin_path),
        Err(_) => path.exists().then(|| {
            let path =
                crate::pathutils::join(&std::env::current_dir().ok().unwrap_or_default(), path)
                    .unwrap_or_else(|| path.to_path_buf());
            VirtualFile { canon_path: crate::pathutils::clean_path(&path), builtin_contents: None }
        }),
    }
}

#[test]
fn test_load_file() {
    let builtin = load_file(&std::path::PathBuf::from(
        "builtin:/foo/../common/./MadeWithSlint-logo-dark.svg",
    ))
    .unwrap();
    assert!(builtin.is_builtin());
    assert_eq!(
        builtin.canon_path,
        std::path::PathBuf::from("builtin:/common/MadeWithSlint-logo-dark.svg")
    );

    let dir = std::env::var_os("CARGO_MANIFEST_DIR").unwrap().to_string_lossy().to_string();
    let dir_path = std::path::PathBuf::from(dir);

    let non_existing = dir_path.join("XXXCargo.tomlXXX");
    assert!(load_file(&non_existing).is_none());

    assert!(dir_path.exists()); // We need some existing path for all the rest

    let cargo_toml = dir_path.join("Cargo.toml");
    let abs_cargo_toml = load_file(&cargo_toml).unwrap();
    assert!(!abs_cargo_toml.is_builtin());
    assert!(crate::pathutils::is_absolute(&abs_cargo_toml.canon_path));
    assert!(abs_cargo_toml.canon_path.exists());

    let current = std::env::current_dir().unwrap();
    assert!(current.ends_with("compiler")); // This test is run in .../internal/compiler

    let cargo_toml = std::path::PathBuf::from("./tests/../Cargo.toml");
    let rel_cargo_toml = load_file(&cargo_toml).unwrap();
    assert!(!rel_cargo_toml.is_builtin());
    assert!(crate::pathutils::is_absolute(&rel_cargo_toml.canon_path));
    assert!(rel_cargo_toml.canon_path.exists());

    assert_eq!(abs_cargo_toml.canon_path, rel_cargo_toml.canon_path);
}

mod builtin_library {
    include!(env!("SLINT_WIDGETS_LIBRARY"));

    pub type BuiltinDirectory<'a> = [&'a BuiltinFile<'a>];

    pub struct BuiltinFile<'a> {
        pub path: &'a str,
        pub contents: &'static [u8],
    }

    use super::VirtualFile;

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

    pub(crate) fn load_builtin_file(builtin_path: &std::path::Path) -> Option<VirtualFile> {
        let mut components = vec![];
        for part in builtin_path.iter() {
            if part == ".." {
                components.pop();
            } else if part != "." {
                components.push(part);
            }
        }
        if let Some(f) = components.first_mut() {
            if let Some((_, x)) = ALIASES.iter().find(|x| x.0 == *f) {
                *f = std::ffi::OsStr::new(x);
            }
        }
        if let &[folder, file] = components.as_slice() {
            let library = widget_library().iter().find(|x| x.0 == folder)?.1;
            library.iter().find_map(|builtin_file| {
                if builtin_file.path == file {
                    Some(VirtualFile {
                        canon_path: std::path::PathBuf::from(format!(
                            "builtin:/{}/{}",
                            folder.to_str().unwrap(),
                            builtin_file.path
                        )),
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
