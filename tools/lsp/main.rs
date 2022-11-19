// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#![cfg(not(target_arch = "wasm32"))]

mod completion;
mod goto;
mod lsp_ext;
#[cfg(feature = "preview")]
mod preview;
mod properties;
mod semantic_tokens;
mod server_loop;
#[cfg(test)]
mod test;
mod util;

use i_slint_compiler::CompilerConfiguration;
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidOpenTextDocument, Notification,
};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams};
use server_loop::*;

use clap::Parser;
use lsp_server::{Connection, ErrorCode, Message, RequestId, Response};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{atomic, Arc, Mutex};
use std::task::{Poll, Waker};

#[derive(Clone, clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(
        short = 'I',
        name = "Add include paths for the import statements",
        number_of_values = 1,
        action
    )]
    include_paths: Vec<std::path::PathBuf>,

    /// The style name for the preview ('native' or 'fluent')
    #[arg(long, name = "style name", default_value_t, action)]
    style: String,

    /// The backend used for the preview ('GL' or 'Qt')
    #[arg(long, name = "backend", default_value_t, action)]
    backend: String,
}

enum OutgoingRequest {
    Pending(Waker),
    Done(lsp_server::Response),
}

type OutgoingRequestQueue = Arc<Mutex<HashMap<RequestId, OutgoingRequest>>>;

/// A handle that can be used to communicate with the client
///
/// This type is duplicated, with the same interface, in wasm_main.rs
#[derive(Clone)]
pub struct ServerNotifier(crossbeam_channel::Sender<Message>, OutgoingRequestQueue);
impl ServerNotifier {
    pub fn send_notification(
        &self,
        method: String,
        params: impl serde::Serialize,
    ) -> Result<(), Error> {
        self.0.send(Message::Notification(lsp_server::Notification::new(method, params)))?;
        Ok(())
    }

    pub fn send_request<T: lsp_types::request::Request>(
        &self,
        request: T::Params,
    ) -> Result<impl Future<Output = Result<T::Result, Error>>, Error> {
        static REQ_ID: atomic::AtomicI32 = atomic::AtomicI32::new(0);
        let id = RequestId::from(REQ_ID.fetch_add(1, atomic::Ordering::Relaxed));
        let msg =
            Message::Request(lsp_server::Request::new(id.clone(), T::METHOD.to_string(), request));
        self.0.send(msg)?;
        let queue = self.1.clone();
        Ok(std::future::poll_fn(move |ctx| {
            let mut queue = queue.lock().unwrap();
            match queue.remove(&id) {
                None | Some(OutgoingRequest::Pending(_)) => {
                    queue.insert(id.clone(), OutgoingRequest::Pending(ctx.waker().clone()));
                    Poll::Pending
                }
                Some(OutgoingRequest::Done(d)) => {
                    if let Some(err) = d.error {
                        Poll::Ready(Err(err.message.into()))
                    } else if let Some(d) = d.result {
                        Poll::Ready(
                            serde_json::from_value(d)
                                .map_err(|e| format!("cannot deserialize response: {e:?}").into()),
                        )
                    } else {
                        Poll::Ready(Err("No response".into()))
                    }
                }
            }
        }))
    }
}

impl RequestHandler {
    async fn handle_request(
        &self,
        request: lsp_server::Request,
        ctx: &Rc<Context>,
    ) -> Result<(), Error> {
        if let Some(x) = self.0.get(&request.method.as_str()) {
            match x(request.params, ctx.clone()).await {
                Ok(r) => ctx
                    .server_notifier
                    .0
                    .send(Message::Response(Response::new_ok(request.id, r)))?,
                Err(e) => ctx.server_notifier.0.send(Message::Response(Response::new_err(
                    request.id,
                    ErrorCode::InternalError as i32,
                    e.to_string(),
                )))?,
            };
        } else {
            ctx.server_notifier.0.send(Message::Response(Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                "Cannot handle request".into(),
            )))?;
        }
        Ok(())
    }
}

fn main() {
    let args: Cli = Cli::parse();
    if !args.backend.is_empty() {
        std::env::set_var("SLINT_BACKEND", &args.backend);
    }

    #[cfg(feature = "preview")]
    {
        let lsp_thread = std::thread::spawn(|| {
            /// Make sure we quit the event loop even if we panic
            struct QuitEventLoop;
            impl Drop for QuitEventLoop {
                fn drop(&mut self) {
                    preview::quit_ui_event_loop();
                }
            }
            let _quit_ui_loop = QuitEventLoop;

            match run_lsp_server() {
                Ok(_) => {}
                Err(error) => {
                    eprintln!("Error running LSP server: {}", error);
                }
            }
        });

        preview::start_ui_event_loop();
        lsp_thread.join().unwrap();
    }
    #[cfg(not(feature = "preview"))]
    match run_lsp_server() {
        Ok(_) => {}
        Err(error) => {
            eprintln!("Error running LSP server: {}", error);
        }
    }
}

pub fn run_lsp_server() -> Result<(), Error> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;

    let initialize_result = serde_json::to_value(server_loop::server_initialize_result())?;
    connection.initialize_finish(id, initialize_result)?;

    main_loop(&connection, params)?;
    io_threads.join()?;
    Ok(())
}

fn main_loop(connection: &Connection, params: serde_json::Value) -> Result<(), Error> {
    let init_params: InitializeParams = serde_json::from_value(params).unwrap();
    let mut compiler_config =
        CompilerConfiguration::new(i_slint_compiler::generator::OutputFormat::Interpreter);

    let cli_args = Cli::parse();
    compiler_config.style =
        Some(if cli_args.style.is_empty() { "fluent".into() } else { cli_args.style });
    compiler_config.include_paths = cli_args.include_paths;

    let mut rh = RequestHandler::default();
    register_request_handlers(&mut rh);

    let request_queue = OutgoingRequestQueue::default();
    let server_notifier = ServerNotifier(connection.sender.clone(), request_queue.clone());
    let ctx = Rc::new(Context {
        document_cache: RefCell::new(DocumentCache::new(compiler_config)),
        server_notifier: server_notifier.clone(),
        init_param: init_params,
    });

    let mut futures = Vec::<Pin<Box<dyn Future<Output = Result<(), Error>>>>>::new();
    let mut first_future = Box::pin(load_configuration(&ctx));

    // We are waiting in this loop for two kind of futures:
    //  - The compiler future should always be ready immediately because we do not set a callback to load files
    //  - the future from `send_request` are blocked waiting for a response from the client.
    //    Responses are sent on the `connection.reciever` which will wake the loop, so there
    //    is no need to do anything in the Waker.
    struct DummyWaker;
    impl std::task::Wake for DummyWaker {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Arc::new(DummyWaker).into();
    match first_future.as_mut().poll(&mut std::task::Context::from_waker(&waker)) {
        Poll::Ready(x) => x?,
        Poll::Pending => futures.push(first_future),
    };

    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                futures.push(Box::pin(rh.handle_request(req, &ctx)));
            }
            Message::Response(resp) => {
                if let Some(q) = request_queue.lock().unwrap().get_mut(&resp.id) {
                    match q {
                        OutgoingRequest::Done(_) => {
                            return Err("Response to unknown request".into())
                        }
                        OutgoingRequest::Pending(x) => x.wake_by_ref(),
                    };
                    *q = OutgoingRequest::Done(resp)
                } else {
                    return Err("Response to unknown request".into());
                }
            }
            Message::Notification(notification) => {
                futures.push(Box::pin(handle_notification(notification, &ctx)))
            }
        }

        let mut result = Ok(());
        futures.retain_mut(|f| {
            if result.is_err() {
                return true;
            }
            match f.as_mut().poll(&mut std::task::Context::from_waker(&waker)) {
                Poll::Ready(x) => {
                    result = x;
                    false
                }
                Poll::Pending => true,
            }
        });
        result?;
    }
    Ok(())
}

async fn handle_notification(req: lsp_server::Notification, ctx: &Context) -> Result<(), Error> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                &ctx.server_notifier,
                params.text_document.text,
                params.text_document.uri,
                params.text_document.version,
                &mut ctx.document_cache.borrow_mut(),
            )
            .await?;
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                &ctx.server_notifier,
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
                params.text_document.version,
                &mut ctx.document_cache.borrow_mut(),
            )
            .await?;
        }
        DidChangeConfiguration::METHOD => {
            load_configuration(ctx).await?;
        }

        #[cfg(feature = "preview")]
        "slint/showPreview" => {
            show_preview_command(
                req.params.as_array().map_or(&[], |x| x.as_slice()),
                &ctx.server_notifier,
                &ctx.document_cache.borrow().documents.compiler_config,
            )?;
        }
        _ => (),
    }
    Ok(())
}
