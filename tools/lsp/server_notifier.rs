// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The handle used to talk to the LSP client.
//!
//! The native implementation writes `lsp_server::Message`s into the connection
//! channel, the wasm one calls into the JavaScript client.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::editor_preview::Result;
    use lsp_server::{Message, RequestId};
    use lsp_types::notification::Notification;
    use std::sync::{Arc, atomic};
    use std::task::{Poll, Waker};

    pub enum OutgoingRequest {
        Start,
        Pending(Waker),
        Done(lsp_server::Response),
    }

    pub type OutgoingRequestQueue = Arc<dashmap::DashMap<RequestId, OutgoingRequest>>;

    /// A handle that can be used to communicate with the client
    #[derive(Clone)]
    pub struct ServerNotifier {
        sender: crossbeam_channel::Sender<Message>,
        queue: OutgoingRequestQueue,
    }

    impl ServerNotifier {
        pub fn new(
            sender: crossbeam_channel::Sender<Message>,
            queue: OutgoingRequestQueue,
        ) -> Self {
            Self { sender, queue }
        }

        /// Send a message to the client verbatim, e.g. a response to a request.
        pub fn send_message(&self, message: Message) -> Result<()> {
            self.sender.send(message)?;
            Ok(())
        }

        pub fn send_notification<N: Notification>(&self, params: N::Params) -> Result<()> {
            self.sender.send(Message::Notification(lsp_server::Notification::new(
                N::METHOD.to_string(),
                params,
            )))?;
            Ok(())
        }

        pub fn send_request<T: lsp_types::request::Request>(
            &self,
            request: T::Params,
        ) -> Result<impl Future<Output = Result<T::Result>>> {
            static REQ_ID: atomic::AtomicI32 = atomic::AtomicI32::new(0);
            let id = RequestId::from(REQ_ID.fetch_add(1, atomic::Ordering::Relaxed));
            let msg = Message::Request(lsp_server::Request::new(
                id.clone(),
                T::METHOD.to_string(),
                request,
            ));
            let queue = self.queue.clone();
            queue.insert(id.clone(), OutgoingRequest::Start);
            if let Err(err) = self.sender.send(msg) {
                queue.remove(&id);
                return Err(err.into());
            }
            Ok(std::future::poll_fn(move |ctx| {
                let mut entry = queue.get_mut(&id).unwrap();
                if !matches!(*entry, OutgoingRequest::Done(_)) {
                    *entry = OutgoingRequest::Pending(ctx.waker().clone());
                    return Poll::Pending;
                }
                drop(entry);
                let OutgoingRequest::Done(response) = queue.remove(&id).unwrap().1 else {
                    unreachable!();
                };
                match response.response_result {
                    Err(err) => Poll::Ready(Err(err.message.into())),
                    Ok(result) => Poll::Ready(
                        serde_json::from_value(result)
                            .map_err(|e| format!("cannot deserialize response: {e:?}").into()),
                    ),
                }
            }))
        }

        #[cfg(test)]
        pub fn dummy() -> Self {
            Self { sender: crossbeam_channel::unbounded().0, queue: Default::default() }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn request_is_registered_before_transport_receives_it() {
            let (sender, receiver) = crossbeam_channel::bounded(0);
            let queue = OutgoingRequestQueue::default();
            let notifier = ServerNotifier::new(sender, queue.clone());
            let sender_thread = std::thread::spawn(move || {
                let _request = notifier
                    .send_request::<lsp_types::request::WorkspaceConfiguration>(
                        lsp_types::ConfigurationParams { items: vec![] },
                    )
                    .unwrap();
            });

            let mut select = crossbeam_channel::Select::new();
            select.recv(&receiver);
            select.ready_timeout(std::time::Duration::from_secs(5)).unwrap();
            let registered = queue.len();
            receiver.recv().unwrap();
            sender_thread.join().unwrap();

            assert_eq!(registered, 1, "The transport can receive an unregistered request");
        }

        #[test]
        fn failed_send_removes_request() {
            let (sender, receiver) = crossbeam_channel::unbounded();
            let queue = OutgoingRequestQueue::default();
            let notifier = ServerNotifier::new(sender, queue.clone());
            drop(receiver);

            assert!(
                notifier
                    .send_request::<lsp_types::request::WorkspaceConfiguration>(
                        lsp_types::ConfigurationParams { items: vec![] },
                    )
                    .is_err()
            );
            assert!(queue.is_empty());
        }

        #[test]
        fn response_before_first_poll_completes() {
            let (sender, receiver) = crossbeam_channel::unbounded();
            let queue = OutgoingRequestQueue::default();
            let notifier = ServerNotifier::new(sender, queue.clone());
            let response = notifier
                .send_request::<lsp_types::request::WorkspaceConfiguration>(
                    lsp_types::ConfigurationParams { items: vec![] },
                )
                .unwrap();
            let Message::Request(request) = receiver.recv().unwrap() else {
                panic!("Expected a request");
            };
            *queue.get_mut(&request.id).unwrap() = OutgoingRequest::Done(
                lsp_server::Response::new_ok(request.id.clone(), serde_json::json!([])),
            );

            let mut response = std::pin::pin!(response);
            let mut context = std::task::Context::from_waker(Waker::noop());
            assert!(
                matches!(response.as_mut().poll(&mut context), Poll::Ready(Ok(value)) if value.is_empty())
            );
            assert!(queue.is_empty());
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::editor_preview::Result;
    // The JSON friendly serializer of the wasm entry point, so that params end
    // up as JS objects instead of ES maps.
    use crate::to_value;
    use js_sys::Function;
    use wasm_bindgen::prelude::*;

    /// A handle that can be used to communicate with the client
    #[derive(Clone)]
    pub struct ServerNotifier {
        send_notification: Function,
        send_request: Function,
    }

    impl ServerNotifier {
        pub fn new(send_notification: Function, send_request: Function) -> Self {
            Self { send_notification, send_request }
        }

        pub fn send_notification<N: lsp_types::notification::Notification>(
            &self,
            params: N::Params,
        ) -> Result<()> {
            self.send_notification
                .call2(&JsValue::UNDEFINED, &N::METHOD.into(), &to_value(&params)?)
                .map_err(|x| format!("Error calling send_notification: {x:?}"))?;
            Ok(())
        }

        pub fn send_request<T: lsp_types::request::Request>(
            &self,
            request: T::Params,
        ) -> Result<impl Future<Output = Result<T::Result>>> {
            let promise = self
                .send_request
                .call2(&JsValue::UNDEFINED, &T::METHOD.into(), &to_value(&request)?)
                .map_err(|x| format!("Error calling send_request: {x:?}"))?;
            let future = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::from(promise));
            Ok(async move {
                future.await.map_err(|e| format!("{e:?}").into()).and_then(|v| {
                    serde_wasm_bindgen::from_value(v).map_err(|e| format!("{e:?}").into())
                })
            })
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
