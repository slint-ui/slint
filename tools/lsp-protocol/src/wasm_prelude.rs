// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::{Path, PathBuf};

/// lsp_url doesn't have method to convert to and from PathBuf for wasm, so just make some
pub trait UrlWasm {
    fn to_file_path(&self) -> Result<PathBuf, ()>;
    fn from_file_path<P: AsRef<Path>>(path: P) -> Result<lsp_types::Url, ()>;
}
impl UrlWasm for lsp_types::Url {
    fn to_file_path(&self) -> Result<PathBuf, ()> {
        Ok(self.to_string().into())
    }
    fn from_file_path<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        Self::parse(path.as_ref().to_str().ok_or(())?).map_err(|_| ())
    }
}
