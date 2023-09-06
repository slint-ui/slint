// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Data structures common between LSP and previewer

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

/// API used by the LSP to talk to the Preview. The other direction uses the
/// ServerNotifier
pub trait PreviewApi {
    fn set_contents(&self, path: &Path, contents: &str);
    fn load_preview(&self, component: PreviewComponent);
    fn config_changed(
        &self,
        style: &str,
        include_paths: &[PathBuf],
        library_paths: &HashMap<String, PathBuf>,
    );
    fn highlight(&self, path: Option<PathBuf>, offset: u32) -> Result<()>;
}

/// The Component to preview
#[allow(unused)]
#[derive(Default, Clone)]
pub struct PreviewComponent {
    /// The file name to preview
    pub path: PathBuf,
    /// The name of the component within that file.
    /// If None, then the last component is going to be shown.
    pub component: Option<String>,

    /// The list of include paths
    pub include_paths: Vec<PathBuf>,

    /// The map of library paths
    pub library_paths: HashMap<String, PathBuf>,

    /// The style name for the preview
    pub style: String,
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    SetContents {
        path: String,
        contents: String,
    },
    SetConfiguration {
        style: String,
        include_paths: Vec<String>,
        library_paths: Vec<(String, String)>,
    },
    ShowPreview {
        path: String,
        component: Option<String>,
        style: String,
        include_paths: Vec<String>,
        library_paths: Vec<(String, String)>,
    },
    HighlightFromEditor {
        path: Option<String>,
        offset: u32,
    },
}
