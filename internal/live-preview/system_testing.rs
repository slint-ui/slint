// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use crate::protocol::{
    PreviewToDebuggerMessage, PreviewToLsp, PreviewToLspMessage, SystemTestMessage,
};

/// Answers a [`crate::protocol::DebuggerToPreviewMessage::SystemTestRequest`]
/// with a [`PreviewToDebuggerMessage::SystemTestResponse`] through `to_editor`.
/// The subscriptions the request creates push their changes through `to_editor` too.
/// Call it on the thread that runs the Slint event loop.
pub fn answer_request(id: u64, request: SystemTestMessage, to_editor: Weak<dyn PreviewToLsp>) {
    if let Err(error) = spawn_answer(id, request, to_editor.clone()) {
        send(&to_editor, id, Err(error));
    }
}

#[cfg(feature = "system-testing")]
fn spawn_answer(
    id: u64,
    request: SystemTestMessage,
    to_editor: Weak<dyn PreviewToLsp>,
) -> Result<(), String> {
    i_slint_backend_selector::with_global_context(|context| {
        context.spawn_local(async move {
            let sink = forward_notifications(to_editor.clone());
            let response =
                i_slint_backend_testing::systest::handle_encoded_request(&request.0, sink).await;
            send(&to_editor, id, Ok(SystemTestMessage(response)));
        })
    })
    .map_err(|error| error.to_string())?
    .map(drop)
    .map_err(|error| error.to_string())
}

#[cfg(feature = "system-testing")]
fn forward_notifications(
    to_editor: Weak<dyn PreviewToLsp>,
) -> i_slint_backend_testing::systest::NotificationSink {
    use i_slint_backend_testing::systest::Disconnected;
    std::rc::Rc::new(move |notification| {
        let to_editor = to_editor.upgrade().ok_or(Disconnected)?;
        to_editor
            .send(&PreviewToLspMessage::DebuggerNotification(
                crate::protocol::PreviewToDebuggerNotification::SubscriptionNotification(
                    SystemTestMessage(notification),
                ),
            ))
            .map_err(|_| Disconnected)
    })
}

#[cfg(not(feature = "system-testing"))]
fn spawn_answer(
    _id: u64,
    _request: SystemTestMessage,
    _to_editor: Weak<dyn PreviewToLsp>,
) -> Result<(), String> {
    Err("This preview was built without the 'system-testing' feature".into())
}

fn send(to_editor: &Weak<dyn PreviewToLsp>, id: u64, response: Result<SystemTestMessage, String>) {
    let Some(to_editor) = to_editor.upgrade() else { return };
    to_editor
        .send(&PreviewToLspMessage::Debugger {
            id,
            message: PreviewToDebuggerMessage::SystemTestResponse(response),
        })
        .ok();
}
