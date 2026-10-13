// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::time::Duration;

use futures_util::{Sink, SinkExt as _};
use i_slint_live_preview::protocol::PreviewToLspMessage;
use tokio_tungstenite_wasm::Message;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const PING_INTERVAL: Duration = Duration::from_secs(5);
pub const PONG_TIMEOUT: Duration = Duration::from_secs(15);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);

pub fn is_allowed_message(message: &PreviewToLspMessage) -> bool {
    matches!(
        message,
        PreviewToLspMessage::Diagnostics { .. }
            | PreviewToLspMessage::DebugMessage { .. }
            | PreviewToLspMessage::RequestState { .. }
    )
}

pub async fn close(socket: &mut (impl Sink<Message> + Unpin)) {
    let _ = tokio::time::timeout(CLOSE_TIMEOUT, socket.close()).await;
}
