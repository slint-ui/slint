// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{collections::HashMap, path::PathBuf};

use lsp_types::Url;

use super::VersionedUrl;

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

#[derive(Clone, derive_more::Debug, serde::Deserialize, serde::Serialize)]
pub enum LspToPreviewMessage {
    InvalidateContents {
        url: lsp_types::Url,
    },
    ForgetFile {
        url: lsp_types::Url,
    },
    SetContents {
        url: VersionedUrl,
        #[debug("Vec<u8> {{ len: {} }}", contents.len())]
        contents: Vec<u8>,
    },
    SetConfiguration {
        config: PreviewConfig,
    },
    ShowPreview(PreviewComponent),
    HighlightFromEditor {
        url: Option<Url>,
        offset: u32,
    },
    /// State of the remote-preview WebSocket, as observed by the LSP main
    /// process. Sent back to the preview process so its Remote Preview
    /// dialog can show the live state.
    RemoteConnectionState {
        state: RemoteConnectionState,
        /// Human-readable `address:port` of the attempted target.
        target: String,
        /// Set on `Failed` to describe the cause.
        error: Option<String>,
    },
    Quit,
    /// Keepalive probe; the viewer answers with [`super::PreviewToLspMessage::Pong`].
    /// Never sent to local previews.
    /// A protocol message because the LSP's browser-compatible WebSocket layer
    /// doesn't expose frame-level pings.
    Ping,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

impl lsp_types::notification::Notification for LspToPreviewMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/lsp_to_preview";
}
