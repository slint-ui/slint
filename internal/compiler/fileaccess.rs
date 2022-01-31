// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

use std::borrow::Cow;

#[derive(Clone)]
pub struct VirtualFile<'a> {
    pub path: Cow<'a, str>,
    pub builtin_contents: Option<&'static [u8]>,
}

impl<'a> VirtualFile<'a> {
    pub fn read(&self) -> Cow<'static, [u8]> {
        match self.builtin_contents {
            Some(static_data) => Cow::Borrowed(static_data),
            None => Cow::Owned(std::fs::read(self.path.as_ref()).unwrap()),
        }
    }

    pub fn is_builtin(&self) -> bool {
        self.builtin_contents.is_some()
    }
}

pub fn load_file<'a>(path: &'a std::path::Path) -> Option<VirtualFile<'static>> {
    match path.strip_prefix("builtin:/") {
        Ok(builtin_path) => builtin_library::load_builtin_file(builtin_path),
        Err(_) => path.exists().then(|| VirtualFile {
            path: Cow::Owned(path.to_string_lossy().to_string()),
            builtin_contents: None,
        }),
    }
}

mod builtin_library {
    include!(env!("SIXTYFPS_WIDGETS_LIBRARY"));

    pub type BuiltinDirectory<'a> = [&'a BuiltinFile<'a>];

    pub struct BuiltinFile<'a> {
        pub path: &'a str,
        pub contents: &'static [u8],
    }

    use super::VirtualFile;

    pub(crate) fn load_builtin_file(
        builtin_path: &std::path::Path,
    ) -> Option<VirtualFile<'static>> {
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
                        path: builtin_file.path.into(),
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
