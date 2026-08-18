// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{Device, DeviceId, DiagnosticSeverity, LogLevel, SessionEvent};

/// The JSON-lines protocol version spoken by Springboard clients and servers.
pub const SPRINGBOARD_PROTOCOL_VERSION: u32 = 1;

/// A client-generated request ID.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct RequestId(pub u64);

/// A versioned request from a Springboard client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClientRequest {
    pub protocol_version: u32,
    pub request_id: RequestId,
    #[serde(flatten)]
    pub command: ClientCommand,
}

/// A command sent by a Springboard client.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "command")]
pub enum ClientCommand {
    Handshake { client_name: String },
    Snapshot,
    Launch { device_id: DeviceId },
    Stop { device_id: DeviceId },
    Refresh { device_id: DeviceId },
    AddManualDevice { address: String },
    Shutdown,
}

/// The complete observable state for one project session.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectSnapshot {
    pub project_root: PathBuf,
    pub entry_file: PathBuf,
    pub component: String,
    pub devices: Vec<Device>,
    pub active_device: Option<DeviceId>,
    pub last_used_device: Option<DeviceId>,
}

/// A response correlated with a client request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResponseEnvelope {
    pub protocol_version: u32,
    pub request_id: RequestId,
    #[serde(flatten)]
    pub response: ResponsePayload,
}

impl ResponseEnvelope {
    /// Create a response using the current protocol version.
    pub fn new(request_id: RequestId, response: ResponsePayload) -> Self {
        Self { protocol_version: SPRINGBOARD_PROTOCOL_VERSION, request_id, response }
    }
}

/// The result of a Springboard request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "response")]
pub enum ResponsePayload {
    Ok,
    Snapshot { snapshot: ProjectSnapshot },
    Error { code: ProtocolErrorCode, message: String },
}

/// A stable machine-readable protocol error category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProtocolErrorCode {
    InvalidRequest,
    UnknownCommand,
    VersionMismatch,
    HandshakeRequired,
    UnknownDevice,
    TargetLimitReached,
    Internal,
}

/// An asynchronous event sent by the Springboard server.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EventEnvelope {
    pub protocol_version: u32,
    #[serde(flatten)]
    pub event: ServerEvent,
}

impl EventEnvelope {
    /// Create an event using the current protocol version.
    pub fn new(event: ServerEvent) -> Self {
        Self { protocol_version: SPRINGBOARD_PROTOCOL_VERSION, event }
    }
}

/// An asynchronous project session event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "event")]
pub enum ServerEvent {
    Snapshot {
        snapshot: ProjectSnapshot,
    },
    DeviceChanged {
        device: Device,
    },
    ActiveDeviceChanged {
        device_id: Option<DeviceId>,
    },
    LastUsedDeviceChanged {
        device_id: Option<DeviceId>,
    },
    Log {
        device_id: Option<DeviceId>,
        level: LogLevel,
        message: String,
    },
    Diagnostic {
        device_id: DeviceId,
        severity: DiagnosticSeverity,
        message: String,
        file: Option<String>,
        line: Option<u32>,
        column: Option<u32>,
    },
    Error {
        device_id: Option<DeviceId>,
        message: String,
    },
    Shutdown,
}

impl From<SessionEvent> for ServerEvent {
    fn from(event: SessionEvent) -> Self {
        match event {
            SessionEvent::DeviceChanged { device } => Self::DeviceChanged { device },
            SessionEvent::ActiveDeviceChanged { device_id } => {
                Self::ActiveDeviceChanged { device_id }
            }
            SessionEvent::LastUsedDeviceChanged { device_id } => {
                Self::LastUsedDeviceChanged { device_id }
            }
            SessionEvent::Log { device_id, level, message } => {
                Self::Log { device_id, level, message }
            }
            SessionEvent::Diagnostic { device_id, severity, message, file, line, column } => {
                Self::Diagnostic { device_id, severity, message, file, line, column }
            }
            SessionEvent::Error { device_id, message } => Self::Error { device_id, message },
        }
    }
}

/// A message sent by the Springboard server.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum ServerMessage {
    Response(ResponseEnvelope),
    Event(EventEnvelope),
}

/// A request that could not be decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestDecodeError {
    pub request_id: Option<RequestId>,
    pub code: ProtocolErrorCode,
    pub message: String,
}

impl std::fmt::Display for RequestDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for RequestDecodeError {}

/// Decode one JSON-lines request while retaining its request ID for error responses.
pub fn decode_request(line: &str) -> Result<ClientRequest, RequestDecodeError> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|error| RequestDecodeError {
            request_id: None,
            code: ProtocolErrorCode::InvalidRequest,
            message: format!("Invalid JSON request: {error}"),
        })?;
    let request_id = value.get("request_id").and_then(serde_json::Value::as_u64).map(RequestId);
    let command = value.get("command").and_then(serde_json::Value::as_str);
    let known_command = matches!(
        command,
        Some(
            "handshake"
                | "snapshot"
                | "launch"
                | "stop"
                | "refresh"
                | "add-manual-device"
                | "shutdown"
        )
    );
    if command.is_some() && !known_command {
        return Err(RequestDecodeError {
            request_id,
            code: ProtocolErrorCode::UnknownCommand,
            message: format!("Unknown Springboard command {}", command.unwrap()),
        });
    }
    serde_json::from_value(value).map_err(|error| RequestDecodeError {
        request_id,
        code: ProtocolErrorCode::InvalidRequest,
        message: format!("Invalid Springboard request: {error}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trip_preserves_version_and_request_id() {
        let request = ClientRequest {
            protocol_version: SPRINGBOARD_PROTOCOL_VERSION,
            request_id: RequestId(42),
            command: ClientCommand::Launch {
                device_id: DeviceId::new("builtin:local-viewer").unwrap(),
            },
        };

        let json = serde_json::to_string(&request).unwrap();

        assert_eq!(decode_request(&json).unwrap(), request);
    }

    #[test]
    fn unknown_commands_retain_the_request_id() {
        let error = decode_request(r#"{"protocol_version":1,"request_id":7,"command":"explode"}"#)
            .unwrap_err();

        assert_eq!(error.request_id, Some(RequestId(7)));
        assert_eq!(error.code, ProtocolErrorCode::UnknownCommand);
    }

    #[test]
    fn malformed_json_has_no_request_id() {
        let error = decode_request("not json").unwrap_err();

        assert_eq!(error.request_id, None);
        assert_eq!(error.code, ProtocolErrorCode::InvalidRequest);
    }
}
