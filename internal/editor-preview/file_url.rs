// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Bubuntu

use i_slint_compiler::source_path::SourcePath;
use lsp_types::Url;

/// Returns `None` for a document that isn't a file, such as an `untitled:` one.
pub fn uri_to_file(uri: &Url) -> Option<SourcePath> {
    let path = SourcePath::from_url(uri.clone());
    let is_file = match &path {
        SourcePath::Url(url) => ["file", "vscode-remote"].contains(&url.scheme()),
        SourcePath::File(_) | SourcePath::Builtin(_) => true,
    };
    (cfg!(target_arch = "wasm32") || is_file).then_some(path)
}

#[test]
fn test_uri_to_file() {
    for url in ["builtin:/fluent/button.slint", "vscode-remote://wsl%2Bubuntu/path/to/file.slint"] {
        let url = Url::parse(url).unwrap();
        assert_eq!(uri_to_file(&url).unwrap().to_url(), Some(url));
    }
    assert_eq!(uri_to_file(&Url::parse("untitled:Untitled-1").unwrap()), None);
}
