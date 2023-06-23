// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

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
        Err(_) => path.exists().then(|| VirtualFile {
            canon_path: dunce::canonicalize(path).unwrap_or_else(|_| path.into()),
            builtin_contents: None,
        }),
    }
}

mod builtin_library {
    include!(env!("SLINT_WIDGETS_LIBRARY"));

    pub type BuiltinDirectory<'a> = [&'a BuiltinFile<'a>];

    pub struct BuiltinFile<'a> {
        pub path: &'a str,
        pub contents: &'static [u8],
    }

    use super::VirtualFile;

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
        if let &[folder, file] = components.as_slice() {
            let library = widget_library().iter().find(|x| x.0 == folder)?.1;
            library.iter().find_map(|builtin_file| {
                if builtin_file.path == file {
                    Some(VirtualFile {
                        canon_path: ["builtin:/", folder.to_str().unwrap(), builtin_file.path]
                            .iter()
                            .collect::<std::path::PathBuf>(),
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
