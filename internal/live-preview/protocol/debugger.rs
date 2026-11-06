// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// A message from the debugger to a preview.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum DebuggerToPreviewMessage {
    /// A protobuf-encoded `RequestToAut` from `slint_systest.proto`.
    /// The preview answers with a [`PreviewToDebuggerMessage::SystemTestResponse`]
    /// with the same `id`.
    /// The debugger picks the `id`, unique among its unanswered requests.
    SystemTestRequest {
        id: u64,
        #[serde(with = "protobuf")]
        request: Vec<u8>,
    },
}

/// A message from a preview to the debugger.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum PreviewToDebuggerMessage {
    /// The protobuf-encoded `AutResponse` to the
    /// [`DebuggerToPreviewMessage::SystemTestRequest`] with the same `id`,
    /// or an error if the preview was built without system testing,
    /// or ended before answering.
    SystemTestResponse {
        id: u64,
        #[serde(with = "protobuf_result")]
        response: Result<Vec<u8>, String>,
    },
}

/// Base64 in human-readable formats, where JSON would spell out every byte as a number.
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

mod protobuf_result {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize)]
    enum ResultRef<'a> {
        Ok(#[serde(with = "super::protobuf")] &'a [u8]),
        Err(&'a str),
    }

    #[derive(Deserialize)]
    enum ResultOwned {
        Ok(#[serde(with = "super::protobuf")] Vec<u8>),
        Err(String),
    }

    pub fn serialize<S: serde::Serializer>(
        response: &Result<Vec<u8>, String>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match response {
            Ok(bytes) => ResultRef::Ok(bytes),
            Err(error) => ResultRef::Err(error),
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Result<Vec<u8>, String>, D::Error> {
        Ok(match ResultOwned::deserialize(deserializer)? {
            ResultOwned::Ok(bytes) => Ok(bytes),
            ResultOwned::Err(error) => Err(error),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_encodes_protobuf_as_base64() {
        let request = DebuggerToPreviewMessage::SystemTestRequest { id: 7, request: vec![8, 1] };
        let json = serde_json::to_string(&request).unwrap();
        assert_eq!(json, r#"{"SystemTestRequest":{"id":7,"request":"CAE="}}"#);
        assert_eq!(serde_json::from_str::<DebuggerToPreviewMessage>(&json).unwrap(), request);

        for response in [Ok(vec![8, 1]), Err("failed".to_string())] {
            let response = PreviewToDebuggerMessage::SystemTestResponse { id: 7, response };
            let json = serde_json::to_string(&response).unwrap();
            assert_eq!(serde_json::from_str::<PreviewToDebuggerMessage>(&json).unwrap(), response);
        }
        let ok = PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Ok(vec![8, 1]) };
        assert_eq!(
            serde_json::to_string(&ok).unwrap(),
            r#"{"SystemTestResponse":{"id":7,"response":{"Ok":"CAE="}}}"#
        );
    }
}
