use preview_protocol::lsp_types::notification::Notification;

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct RemoteViewerDiscoveredMessage {
    pub host: String,
    pub port: u16,
    pub addresses: Vec<String>,
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
