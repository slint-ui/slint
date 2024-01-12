// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

//! Data structures common between LSP and previewer

use lsp_types::Url;

use std::{collections::HashMap, path::PathBuf};

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;
pub type UrlVersion = Option<i32>;

/// A versioned file
#[derive(Clone, serde::Deserialize, serde::Serialize)]
pub struct VersionedUrl {
    /// The file url
    pub url: Url,
    // The file version
    pub version: UrlVersion,
}

impl std::fmt::Debug for VersionedUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version = self.version.map(|v| format!("v{v}")).unwrap_or_else(|| "none".to_string());
        write!(f, "{}@{}", self.url, version)
    }
}

/// A versioned file
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Position {
    /// The file url
    pub url: Url,
    /// The offset in the file pointed to by the `url`
    pub offset: u32,
}

#[derive(Default, Clone, PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewConfig {
    pub hide_ui: Option<bool>,
    pub style: String,
    pub include_paths: Vec<PathBuf>,
    pub library_paths: HashMap<String, PathBuf>,
}

/// API used by the LSP to talk to the Preview. The other direction uses the
/// ServerNotifier
pub trait PreviewApi {
    fn set_use_external_previewer(&self, use_external: bool);
    fn set_contents(&self, url: &VersionedUrl, contents: &str);
    fn load_preview(&self, component: PreviewComponent);
    fn config_changed(&self, config: PreviewConfig);
    fn highlight(&self, url: Option<Url>, offset: u32) -> Result<()>;

    /// What is the current component to preview?
    fn current_component(&self) -> Option<PreviewComponent>;

    fn report_known_components(
        &self,
        url: Option<VersionedUrl>,
        components: Vec<ComponentInformation>,
    );
}

/// The Component to preview
#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct PreviewComponent {
    /// The file name to preview
    pub url: Url,
    /// The name of the component within that file.
    /// If None, then the last component is going to be shown.
    pub component: Option<String>,

    /// The style name for the preview
    pub style: String,
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    SetContents { url: VersionedUrl, contents: String },
    SetConfiguration { config: PreviewConfig },
    ShowPreview(PreviewComponent),
    HighlightFromEditor { url: Option<Url>, offset: u32 },
    KnownComponents { url: Option<VersionedUrl>, components: Vec<ComponentInformation> },
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct Diagnostic {
    pub message: String,
    pub file: Option<String>,
    pub line: usize,
    pub column: usize,
    pub level: String,
}

#[allow(unused)]
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub enum PreviewToLspMessage {
    Status { message: String, health: crate::lsp_ext::Health },
    Diagnostics { uri: Url, diagnostics: Vec<lsp_types::Diagnostic> },
    ShowDocument { file: String, selection: lsp_types::Range },
    PreviewTypeChanged { is_external: bool },
    RequestState { unused: bool }, // send all documents!
}

/// Information on the Element types available
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct ComponentInformation {
    /// The name of the type
    pub name: String,
    /// A broad category to group types by
    pub category: String,
    /// This type is a global component
    pub is_global: bool,
    /// This type is built into Slint
    pub is_builtin: bool,
    /// This type is a standard widget
    pub is_std_widget: bool,
    /// This type was exported
    pub is_exported: bool,
    /// The URL to the file containing this type
    pub defined_at: Option<Position>,
}

impl ComponentInformation {
    pub fn import_file_name(&self, current_uri: &lsp_types::Url) -> Option<String> {
        if self.is_std_widget {
            Some("std-widgets.slint".to_string())
        } else {
            let url = self.defined_at.as_ref().map(|p| &p.url)?;
            lsp_types::Url::make_relative(current_uri, url)
        }
    }
}
