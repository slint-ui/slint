// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// A message from the debugger to a preview.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum DebuggerToPreviewMessage {
    /// A `RequestToAut` from `slint_systest.proto`.
    /// The preview answers with a [`PreviewToDebuggerMessage::SystemTestResponse`].
    SystemTestRequest(SystemTestMessage),
}

/// A message from a preview to the debugger.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum PreviewToDebuggerMessage {
    /// The `AutResponse` from `slint_systest.proto` to a
    /// [`DebuggerToPreviewMessage::SystemTestRequest`],
    /// or an error if the request gets no answer,
    /// such as when no preview runs or it was built without system testing.
    SystemTestResponse(Result<SystemTestMessage, String>),
}

/// A protobuf-encoded message from `slint_systest.proto`.
/// Human-readable formats carry it as base64, where JSON would spell out every byte as a number.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct SystemTestMessage(#[serde(with = "protobuf")] pub Vec<u8>);

mod protobuf {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use serde::{Deserialize, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            serializer.serialize_str(&STANDARD.encode(bytes))
        } else {
            serializer.serialize_bytes(bytes)
        }
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<u8>, D::Error> {
        if deserializer.is_human_readable() {
            let encoded = String::deserialize(deserializer)?;
            STANDARD.decode(encoded).map_err(serde::de::Error::custom)
        } else {
            Vec::deserialize(deserializer)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_encodes_protobuf_as_base64() {
        let request = DebuggerToPreviewMessage::SystemTestRequest(SystemTestMessage(vec![8, 1]));
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, r#"{"SystemTestRequest":"CAE="}"#);
        assert_eq!(serde_json::from_str::<DebuggerToPreviewMessage>(&json).unwrap(), request);

        for response in [Ok(SystemTestMessage(vec![8, 1])), Err("failed".to_string())] {
            let response = PreviewToDebuggerMessage::SystemTestResponse(response);
            let json = serde_json::to_string(&response).unwrap();
            assert_eq!(serde_json::from_str::<PreviewToDebuggerMessage>(&json).unwrap(), response);
        }
        let ok = PreviewToDebuggerMessage::SystemTestResponse(Ok(SystemTestMessage(vec![8, 1])));
        assert_eq!(serde_json::to_string(&ok).unwrap(), r#"{"SystemTestResponse":{"Ok":"CAE="}}"#);
    }
}
