// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use crate::protocol::{PreviewToDebuggerMessage, PreviewToLsp};

/// Answers a [`crate::protocol::DebuggerToPreviewMessage::SystemTestRequest`]
/// with a [`PreviewToDebuggerMessage::SystemTestResponse`] through `to_editor`.
/// Call it on the thread that runs the Slint event loop.
pub fn answer_request(id: u64, request: Vec<u8>, to_editor: Weak<dyn PreviewToLsp>) {
    if let Err(error) = spawn_answer(id, request, to_editor.clone()) {
        send(&to_editor, id, Err(error));
    }
}

#[cfg(feature = "system-testing")]
fn spawn_answer(
    id: u64,
    request: Vec<u8>,
    to_editor: Weak<dyn PreviewToLsp>,
) -> Result<(), String> {
    i_slint_backend_selector::with_global_context(|context| {
        context.spawn_local(async move {
            let response = i_slint_backend_testing::systest::handle_encoded_request(
                &request,
                std::rc::Rc::new(|_| {}),
            )
            .await;
            send(&to_editor, id, Ok(response));
        })
    })
    .map_err(|error| error.to_string())?
    .map(drop)
    .map_err(|error| error.to_string())
}

#[cfg(not(feature = "system-testing"))]
fn spawn_answer(
    _id: u64,
    _request: Vec<u8>,
    _to_editor: Weak<dyn PreviewToLsp>,
) -> Result<(), String> {
    Err("This preview was built without the 'system-testing' feature".into())
}

fn send(to_editor: &Weak<dyn PreviewToLsp>, id: u64, response: Result<Vec<u8>, String>) {
    let Some(to_editor) = to_editor.upgrade() else { return };
    to_editor.send_to_debugger(&PreviewToDebuggerMessage::SystemTestResponse { id, response }).ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct CaptureToDebugger(RefCell<Vec<PreviewToDebuggerMessage>>);

    impl PreviewToLsp for CaptureToDebugger {
        fn send(&self, _: &crate::protocol::PreviewToLspMessage) -> crate::protocol::Result<()> {
            unreachable!("System-test responses go to the debugger")
        }

        fn send_to_debugger(
            &self,
            message: &PreviewToDebuggerMessage,
        ) -> crate::protocol::Result<()> {
            self.0.borrow_mut().push(message.clone());
            Ok(())
        }
    }

    fn answer(request: Vec<u8>, run: impl FnOnce()) -> Vec<PreviewToDebuggerMessage> {
        let capture = Rc::new(CaptureToDebugger::default());
        let to_editor: Rc<dyn PreviewToLsp> = capture.clone();
        answer_request(7, request, Rc::downgrade(&to_editor));
        run();
        capture.0.take()
    }

    #[cfg(not(feature = "system-testing"))]
    #[test]
    fn answers_with_error_when_disabled() {
        assert!(matches!(
            answer(vec![], || {}).as_slice(),
            [PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Err(_) }]
        ));
    }

    #[cfg(all(feature = "system-testing", feature = "preview-session"))]
    #[test]
    fn answers_on_the_event_loop() {
        i_slint_backend_testing::init_integration_test_with_system_time();
        let messages = answer(vec![0xff], || {
            i_slint_backend_selector::with_global_context(|context| {
                context
                    .spawn_local(async { i_slint_core::api::quit_event_loop().unwrap() })
                    .unwrap();
                context.run_event_loop().unwrap();
            })
            .unwrap();
        });
        assert!(matches!(
            messages.as_slice(),
            [PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Ok(response) }]
                if !response.is_empty()
        ));
    }
}
