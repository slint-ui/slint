// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// A message from the debugger to a preview.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum DebuggerToPreviewMessage {
    /// A protobuf-encoded `RequestToAut` from `slint_systest.proto`.
    /// The preview answers with a [`PreviewToDebuggerMessage::SystemTestResponse`]
    /// with the same `id`.
    SystemTestRequest { id: u64, request: Vec<u8> },
}

/// A message from a preview to the debugger.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum PreviewToDebuggerMessage {
    /// The protobuf-encoded `AutResponse` to the
    /// [`DebuggerToPreviewMessage::SystemTestRequest`] with the same `id`,
    /// or an error if the preview was built without system testing,
    /// or ended before answering.
    SystemTestResponse { id: u64, response: Result<Vec<u8>, String> },
}
