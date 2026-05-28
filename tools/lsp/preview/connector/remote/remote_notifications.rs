// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use lsp_types::notification::Notification;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteViewerDiscoveredMessage {
    pub host: String,
    pub port: u16,
    pub addresses: Vec<String>,
    /// Comma-separated list of subprotocols the viewer announced in its
    /// mDNS TXT record. `None` if the viewer pre-dates protocol versioning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer_protocols: Option<String>,
    /// Full Slint version of the viewer from its mDNS TXT record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewer_slint_version: Option<String>,
    /// Subprotocol identifier this LSP build expects. The extension uses
    /// it to decide whether the viewer is compatible.
    pub lsp_protocol: String,
    /// Full Slint version of this LSP build.
    pub lsp_slint_version: String,
}

impl Notification for RemoteViewerDiscoveredMessage {
    type Params = Self;
    const METHOD: &'static str = "slint/remote_viewer_discovered";
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ConnectionState {
    Disconnected,
    Connected,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct RemoteViewerConnectionState {
    pub address: String,
    pub port: u16,
    pub state: ConnectionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Notification for RemoteViewerConnectionState {
    type Params = Self;
    const METHOD: &'static str = "slint/remote_viewer_connection_state";
}
