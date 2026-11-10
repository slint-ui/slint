// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    DebuggerToPreviewMessage, LspToPreviewMessage, PreviewToDebuggerMessage, PreviewToLspMessage,
};

/// What a preview receives once its session is established,
/// from a local preview's stdin or a remote viewer's connection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[cfg_attr(test, derive(PartialEq))]
#[allow(clippy::large_enum_variant)]
pub enum ToPreview {
    Lsp(LspToPreviewMessage),
    Debug(DebuggerToPreviewMessage),
}

/// What a preview sends once its session is established,
/// on a local preview's stdout or a remote viewer's connection.
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[cfg_attr(test, derive(PartialEq))]
pub enum FromPreview {
    Lsp(PreviewToLspMessage),
    Debug(PreviewToDebuggerMessage),
}

impl From<LspToPreviewMessage> for ToPreview {
    fn from(message: LspToPreviewMessage) -> Self {
        Self::Lsp(message)
    }
}

/// Serializes like [`ToPreview`], for senders that only borrow the message.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub enum ToPreviewRef<'a> {
    Lsp(&'a LspToPreviewMessage),
    Debug(&'a DebuggerToPreviewMessage),
}

/// Serializes like [`FromPreview`], for senders that only borrow the message.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub enum FromPreviewRef<'a> {
    Lsp(&'a PreviewToLspMessage),
    Debug(&'a PreviewToDebuggerMessage),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_envelopes_serialize_like_owned_ones() {
        let lsp = LspToPreviewMessage::Ping;
        let debug = DebuggerToPreviewMessage::SystemTestRequest { id: 7, request: vec![1] };
        let pong = PreviewToLspMessage::Pong;
        let response =
            PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Ok(vec![1]) };
        fn assert_same(owned: impl serde::Serialize, borrowed: impl serde::Serialize) {
            assert_eq!(
                postcard::to_allocvec(&owned).unwrap(),
                postcard::to_allocvec(&borrowed).unwrap()
            );
            assert_eq!(
                serde_json::to_string(&owned).unwrap(),
                serde_json::to_string(&borrowed).unwrap()
            );
        }
        assert_same(ToPreview::Lsp(lsp.clone()), ToPreviewRef::Lsp(&lsp));
        assert_same(ToPreview::Debug(debug.clone()), ToPreviewRef::Debug(&debug));
        assert_same(FromPreview::Lsp(pong.clone()), FromPreviewRef::Lsp(&pong));
        assert_same(FromPreview::Debug(response.clone()), FromPreviewRef::Debug(&response));
    }
}
