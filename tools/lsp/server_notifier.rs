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
            self.sender.send(msg)?;
            let queue = self.queue.clone();
            queue.insert(id.clone(), OutgoingRequest::Start);
            Ok(std::future::poll_fn(move |ctx| match queue.remove(&id).unwrap().1 {
                OutgoingRequest::Pending(_) | OutgoingRequest::Start => {
                    queue.insert(id.clone(), OutgoingRequest::Pending(ctx.waker().clone()));
                    Poll::Pending
                }
                OutgoingRequest::Done(d) => match d.response_result {
                    Err(err) => Poll::Ready(Err(err.message.into())),
                    Ok(result) => Poll::Ready(
                        serde_json::from_value(result)
                            .map_err(|e| format!("cannot deserialize response: {e:?}").into()),
                    ),
                },
            }))
        }

        #[cfg(test)]
        pub fn dummy() -> Self {
            Self { sender: crossbeam_channel::unbounded().0, queue: Default::default() }
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
