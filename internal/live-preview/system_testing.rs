// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author Robin Cramer <robin.cramer@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::rc::Weak;

use crate::protocol::{PreviewToDebuggerMessage, PreviewToLsp, PreviewToLspMessage};

/// Answers a [`crate::protocol::DebuggerToPreviewMessage::SystemTestRequest`]
/// with a [`PreviewToDebuggerMessage::SystemTestResponse`] through `to_editor`.
/// The subscriptions the request creates push their changes through `to_editor` too.
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
            let sink = forward_notifications(to_editor.clone());
            let response =
                i_slint_backend_testing::systest::handle_encoded_request(&request, sink).await;
            send(&to_editor, id, Ok(response));
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
    std::rc::Rc::new(move |notification| {
        let Some(to_editor) = to_editor.upgrade() else { return };
        to_editor
            .send(&PreviewToLspMessage::DebuggerMessage(
                PreviewToDebuggerMessage::SubscriptionNotification { notification },
            ))
            .ok();
    })
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
    to_editor
        .send(&PreviewToLspMessage::DebuggerMessage(PreviewToDebuggerMessage::SystemTestResponse {
            id,
            response,
        }))
        .ok();
}

#[cfg(all(test, feature = "system-testing"))]
mod tests {
    use super::*;
    use i_slint_backend_testing::proto::{
        self, aut_response::Msg as Resp, request_to_aut::Msg as Req,
    };
    use i_slint_backend_testing::{TestingBackend, TestingBackendOptions};
    use i_slint_core::SlintContext;
    use prost::Message;
    use slint_interpreter::ComponentHandle;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct CaptureToDebugger(RefCell<Vec<PreviewToDebuggerMessage>>);

    impl PreviewToLsp for CaptureToDebugger {
        fn send(&self, message: &PreviewToLspMessage) -> crate::protocol::Result<()> {
            let PreviewToLspMessage::DebuggerMessage(message) = message else {
                unreachable!("System-test responses go to the debugger")
            };
            self.0.borrow_mut().push(message.clone());
            Ok(())
        }
    }

    #[test]
    fn answers_and_pushes_on_the_event_loop() {
        let context = SlintContext::new(Box::new(TestingBackend::new(TestingBackendOptions {
            threading: true,
            ..Default::default()
        })));
        i_slint_backend_testing::systest::init().unwrap();
        let capture = Rc::new(CaptureToDebugger::default());
        let to_editor: Rc<dyn PreviewToLsp> = capture.clone();
        answer_request(7, vec![0xff], Rc::downgrade(&to_editor));
        let subscribe_to_window_roots = proto::RequestToAut {
            msg: Some(Req::RequestSubscribeElementChildren(
                proto::RequestSubscribeElementChildren { element_handle: None },
            )),
        };
        answer_request(8, subscribe_to_window_roots.encode_to_vec(), Rc::downgrade(&to_editor));
        let proxy = context.event_loop_proxy().unwrap();
        context.spawn_local(async move { proxy.quit_event_loop().unwrap() }).unwrap();
        context.run_event_loop().unwrap();
        let messages = capture.0.take();
        let [
            PreviewToDebuggerMessage::SystemTestResponse { id: 7, response: Ok(response) },
            PreviewToDebuggerMessage::SystemTestResponse { id: 8, response: Ok(subscribed) },
        ] = messages.as_slice()
        else {
            panic!("expected two responses, got {messages:?}");
        };
        assert!(!response.is_empty());
        let Some(Resp::SubscribeElementChildrenResponse(subscribed)) =
            proto::AutResponse::decode(&subscribed[..]).unwrap().msg
        else {
            panic!("expected a children subscription");
        };

        let compiled = tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(
            slint_interpreter::Compiler::default().build_from_source(
                "export component App inherits Window {}".into(),
                "/app.slint".into(),
            ),
        );
        let app = compiled.component("App").unwrap().create().unwrap();
        app.show().unwrap();
        i_slint_core::properties::ChangeTracker::run_change_handlers();
        let messages = capture.0.take();
        let [PreviewToDebuggerMessage::SubscriptionNotification { notification }] =
            messages.as_slice()
        else {
            panic!("expected a notification, got {messages:?}");
        };
        let notification = proto::SubscriptionNotification::decode(&notification[..]).unwrap();
        assert_eq!(notification.subscription_id, subscribed.subscription_id);
        assert!(matches!(
            notification.msg,
            Some(proto::subscription_notification::Msg::ChildrenChanged(_))
        ));
    }
}
