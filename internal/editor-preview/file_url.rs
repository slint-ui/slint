// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Bubuntu

use lsp_types::Url;
use std::path::{Path, PathBuf};

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

pub fn uri_to_file(uri: &Url) -> Option<PathBuf> {
    if ["builtin", "vscode-remote"].contains(&uri.scheme()) {
        Some(PathBuf::from(uri.to_string()))
    } else {
        let path = uri.to_file_path().ok()?;
        let cleaned_path = i_slint_compiler::pathutils::clean_path(&path);
        Some(cleaned_path)
    }
}

pub fn file_to_uri(path: &Path) -> Option<Url> {
    if ["builtin:/", "vscode-remote:/"].iter().any(|prefix| path.starts_with(prefix)) {
        Url::parse(path.to_str()?).ok()
    } else {
        Url::from_file_path(path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_conversion_of_builtins() {
        let builtin_path = PathBuf::from("builtin:/fluent/button.slint");
        let url = file_to_uri(&builtin_path).unwrap();
        assert_eq!(url.scheme(), "builtin");

        let back_conversion = uri_to_file(&url).unwrap();
        assert_eq!(back_conversion, builtin_path);

        assert!(Url::from_file_path(&builtin_path).is_err());
    }

    #[test]
    fn test_uri_conversion_of_slashed_builtins() {
        let builtin_path1 = PathBuf::from("builtin:/fluent/button.slint");
        let builtin_path3 = PathBuf::from("builtin:///fluent/button.slint");

        let url1 = file_to_uri(&builtin_path1).unwrap();
        let url3 = file_to_uri(&builtin_path3).unwrap();
        assert_ne!(url1, url3);

        let back_conversion1 = uri_to_file(&url1).unwrap();
        let back_conversion3 = uri_to_file(&url3).unwrap();
        assert_eq!(back_conversion1, back_conversion3);

        assert_eq!(back_conversion1, builtin_path1);
    }

    #[test]
    fn test_uri_to_file_vscode_remote() {
        let vscode_remote_path = PathBuf::from("vscode-remote://wsl%2Bubuntu/path/to/file.slint");

        let url = file_to_uri(&vscode_remote_path).unwrap();
        let back_conversion = uri_to_file(&url).unwrap();

        assert_eq!(vscode_remote_path, back_conversion);
    }
}
