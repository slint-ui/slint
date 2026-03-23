use std::{collections::HashMap, path::PathBuf};

use lsp_types::Url;

use crate::VersionedUrl;

/// The Component to preview
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct PreviewComponent {
    /// The file name to preview
    pub url: Url,
    /// The name of the component within that file.
    /// If None, then the last component is going to be shown.
    pub component: Option<String>,
}

#[derive(Default, Clone, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewConfig {
    pub hide_ui: Option<bool>,
    pub style: String,
    pub include_paths: Vec<PathBuf>,
    pub library_paths: HashMap<String, PathBuf>,
    pub format_utf8: bool,
    pub enable_experimental: bool,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    InvalidateContents { url: lsp_types::Url },
    ForgetFile { url: lsp_types::Url },
    SetContents { url: VersionedUrl, contents: String },
    SetConfiguration { config: PreviewConfig },
    ShowPreview(PreviewComponent),
    HighlightFromEditor { url: Option<Url>, offset: u32 },
    Quit,
}

impl lsp_types::notification::Notification for LspToPreviewMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/lsp_to_preview";
}
