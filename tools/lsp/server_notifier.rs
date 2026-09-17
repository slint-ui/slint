// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The handle used to talk to the LSP client.
//!
//! The native implementation writes `lsp_server::Message`s into the connection
//! channel, the wasm one calls into the JavaScript client.

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::editor_preview::Result;
    use lsp_server::{Message, RequestId, Response};
    use lsp_types::notification::Notification;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, atomic};
    use tokio::sync::oneshot;

    /// The requests sent to the client that still wait for their response.
    pub type OutgoingRequestQueue = Arc<Mutex<HashMap<RequestId, oneshot::Sender<Response>>>>;

    /// Delivers a response from the client to the future waiting for it.
    /// Returns false if no request with that id is waiting.
    pub fn complete_request(queue: &OutgoingRequestQueue, response: Response) -> bool {
        let Some(sender) = queue.lock().unwrap().remove(&response.id) else { return false };
        // Sending fails when the future was dropped before the response came.
        let _ = sender.send(response);
        true
    }

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
        ) -> Result<impl Future<Output = Result<T::Result>> + use<T>> {
            /// Forgets the request when its future is dropped before the client answers.
            struct Registration {
                queue: OutgoingRequestQueue,
                id: RequestId,
            }

            impl Drop for Registration {
                fn drop(&mut self) {
                    self.queue.lock().unwrap().remove(&self.id);
                }
            }

            static REQ_ID: atomic::AtomicI32 = atomic::AtomicI32::new(0);
            let id = RequestId::from(REQ_ID.fetch_add(1, atomic::Ordering::Relaxed));
            let msg = Message::Request(lsp_server::Request::new(
                id.clone(),
                T::METHOD.to_string(),
                request,
            ));
            let (sender, receiver) = oneshot::channel();
            // Register before sending: the client may answer before `send` returns.
            let registration = Registration { queue: self.queue.clone(), id: id.clone() };
            registration.queue.lock().unwrap().insert(id, sender);
            self.sender.send(msg)?;
            Ok(async move {
                let _registration = registration;
                let response = receiver.await.map_err(|_| "no response from the client")?;
                match response.response_result {
                    Err(err) => Err(err.message.into()),
                    Ok(result) => serde_json::from_value(result)
                        .map_err(|e| format!("cannot deserialize response: {e:?}").into()),
                }
            })
        }

        #[cfg(test)]
        pub fn dummy() -> Self {
            Self { sender: crossbeam_channel::unbounded().0, queue: Default::default() }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn notifier(
            sender: crossbeam_channel::Sender<Message>,
        ) -> (ServerNotifier, OutgoingRequestQueue) {
            let queue = OutgoingRequestQueue::default();
            (ServerNotifier::new(sender, queue.clone()), queue)
        }

        fn config_request(
            notifier: &ServerNotifier,
        ) -> Result<impl Future<Output = Result<Vec<serde_json::Value>>> + use<>> {
            notifier.send_request::<lsp_types::request::WorkspaceConfiguration>(
                lsp_types::ConfigurationParams { items: vec![] },
            )
        }

        #[test]
        fn request_is_registered_before_it_is_sent() {
            // A zero-capacity channel blocks `send` until the test receives,
            // so the queue can be inspected while the request is in flight.
            let (sender, receiver) = crossbeam_channel::bounded(0);
            let (notifier, queue) = notifier(sender);
            let sender_thread = std::thread::spawn(move || config_request(&notifier).unwrap());

            let mut select = crossbeam_channel::Select::new();
            select.recv(&receiver);
            select.ready_timeout(std::time::Duration::from_secs(5)).unwrap();
            assert_eq!(queue.lock().unwrap().len(), 1);

            receiver.recv().unwrap();
            drop(sender_thread.join().unwrap());
            assert!(queue.lock().unwrap().is_empty());
        }

        #[test]
        fn dropped_future_removes_request() {
            let (sender, _receiver) = crossbeam_channel::unbounded();
            let (notifier, queue) = notifier(sender);
            let response = config_request(&notifier).unwrap();
            assert_eq!(queue.lock().unwrap().len(), 1);
            drop(response);
            assert!(queue.lock().unwrap().is_empty());
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
