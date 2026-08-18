// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared project and device management for Slint development sessions.

pub mod persistence;
pub mod project;
pub mod protocol;
pub mod session;

pub use persistence::{
    DEVICE_STATE_SCHEMA_VERSION, DeviceStateStore, GlobalDeviceState, LoadDeviceState,
    RememberedDevice, SaveDeviceState,
};
pub use protocol::{
    ClientCommand, ClientRequest, EventEnvelope, ProjectSnapshot, ProtocolErrorCode,
    RequestDecodeError, RequestId, ResponseEnvelope, ResponsePayload, SPRINGBOARD_PROTOCOL_VERSION,
    ServerEvent, ServerMessage, decode_request,
};
pub use session::{
    Device, DeviceCapabilities, DeviceId, DeviceIdError, DeviceKind, DeviceOrigin, DeviceStatus,
    DiagnosticSeverity, LogLevel, SessionAction, SessionError, SessionEvent, SpringboardSession,
};
