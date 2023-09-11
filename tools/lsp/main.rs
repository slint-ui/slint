// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

#![cfg(not(target_arch = "wasm32"))]

#[cfg(all(feature = "preview-engine", not(feature = "preview-builtin")))]
compile_error!("Feature preview-engine and preview-builtin need to be enabled together when building native LSP");

mod common;
mod language;
pub mod lsp_ext;
#[cfg(feature = "preview-engine")]
mod preview;
pub mod util;

use common::{PreviewApi, Result};
use language::*;

use i_slint_compiler::CompilerConfiguration;
use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidOpenTextDocument, Notification,
};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, InitializeParams};

use clap::Parser;
use lsp_server::{Connection, ErrorCode, IoThreads, Message, RequestId, Response};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{atomic, Arc, Mutex};
use std::task::{Poll, Waker};

struct Previewer {
    #[allow(unused)]
    server_notifier: ServerNotifier,
    use_external_previewer: RefCell<bool>,
    to_show: RefCell<Option<common::PreviewComponent>>,
}

impl PreviewApi for Previewer {
    fn set_use_external_previewer(&self, _use_external: bool) {
        // Only allow switching if both options are available
        #[cfg(all(feature = "preview-builtin", feature = "preview-external"))]
        {
            self.use_external_previewer.replace(_use_external);

            if _use_external {
                preview::close_ui();
            }
        }
    }

    fn request_state(&self, _ctx: &Rc<crate::language::Context>) {
        #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
        {
            let documents = &_ctx.document_cache.borrow().documents;

            for (p, d) in documents.all_file_documents() {
                let Some(node) = &d.node else {
                    continue;
                };
                self.set_contents(p, &node.text().to_string());
            }
            let cc = &documents.compiler_config;
            let empty = String::new();
            self.config_changed(
                cc.style.as_ref().unwrap_or(&empty),
                &cc.include_paths,
                &cc.library_paths,
            );

            if let Some(c) = self.to_show.take() {
                self.load_preview(c);
            }
        }
    }

    fn set_contents(&self, _path: &std::path::Path, _contents: &str) {
        if *self.use_external_previewer.borrow() {
            #[cfg(feature = "preview-external")]
            let _ = self.server_notifier.send_notification(
                "slint/lsp_to_preview".to_string(),
                crate::common::LspToPreviewMessage::SetContents {
                    path: _path.to_string_lossy().to_string(),
                    contents: _contents.to_string(),
                },
            );
        } else {
            #[cfg(feature = "preview-builtin")]
            preview::set_contents(_path, _contents.to_string());
        }
    }

    fn load_preview(&self, component: common::PreviewComponent) {
        self.to_show.replace(Some(component.clone()));

        if *self.use_external_previewer.borrow() {
            #[cfg(feature = "preview-external")]
            let _ = self.server_notifier.send_notification(
                "slint/lsp_to_preview".to_string(),
                crate::common::LspToPreviewMessage::ShowPreview {
                    path: component.path.to_string_lossy().to_string(),
                    component: component.component,
                    style: component.style.to_string(),
                    include_paths: component
                        .include_paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect(),
                    library_paths: component
                        .library_paths
                        .iter()
                        .map(|(n, p)| (n.clone(), p.to_string_lossy().to_string()))
                        .collect(),
                },
            );
        } else {
            #[cfg(feature = "preview-builtin")]
            {
                preview::open_ui(&self.server_notifier);
                preview::load_preview(component);
            }
        }
    }

    fn config_changed(
        &self,
        _style: &str,
        _include_paths: &[PathBuf],
        _library_paths: &HashMap<String, PathBuf>,
    ) {
        if *self.use_external_previewer.borrow() {
            #[cfg(feature = "preview-external")]
            let _ = self.server_notifier.send_notification(
                "slint/lsp_to_preview".to_string(),
                crate::common::LspToPreviewMessage::SetConfiguration {
                    style: _style.to_string(),
                    include_paths: _include_paths
                        .iter()
                        .map(|p| p.to_string_lossy().to_string())
                        .collect(),
                    library_paths: _library_paths
                        .iter()
                        .map(|(n, p)| (n.clone(), p.to_string_lossy().to_string()))
                        .collect(),
                },
            );
        } else {
            #[cfg(feature = "preview-builtin")]
            preview::config_changed(_style, _include_paths, _library_paths);
        }
    }

    fn highlight(&self, _path: Option<std::path::PathBuf>, _offset: u32) -> Result<()> {
        {
            if *self.use_external_previewer.borrow() {
                #[cfg(feature = "preview-external")]
                self.server_notifier.send_notification(
                    "slint/lsp_to_preview".to_string(),
                    crate::common::LspToPreviewMessage::HighlightFromEditor {
                        path: _path.as_ref().map(|p| p.to_string_lossy().to_string()),
                        offset: _offset,
                    },
                )?;
                Ok(())
            } else {
                #[cfg(feature = "preview-builtin")]
                preview::highlight(&_path, _offset);
                Ok(())
            }
        }
    }
}

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
    Start,
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
    pub fn send_notification(&self, method: String, params: impl serde::Serialize) -> Result<()> {
        self.0.send(Message::Notification(lsp_server::Notification::new(method, params)))?;
        Ok(())
    }

    pub fn send_request<T: lsp_types::request::Request>(
        &self,
        request: T::Params,
    ) -> Result<impl Future<Output = Result<T::Result>>> {
        static REQ_ID: atomic::AtomicI32 = atomic::AtomicI32::new(0);
        let id = RequestId::from(REQ_ID.fetch_add(1, atomic::Ordering::Relaxed));
        let msg =
            Message::Request(lsp_server::Request::new(id.clone(), T::METHOD.to_string(), request));
        self.0.send(msg)?;
        let queue = self.1.clone();
        queue.lock().unwrap().insert(id.clone(), OutgoingRequest::Start);
        Ok(std::future::poll_fn(move |ctx| {
            let mut queue = queue.lock().unwrap();
            match queue.remove(&id).unwrap() {
                OutgoingRequest::Pending(_) | OutgoingRequest::Start => {
                    queue.insert(id.clone(), OutgoingRequest::Pending(ctx.waker().clone()));
                    Poll::Pending
                }
                OutgoingRequest::Done(d) => {
                    if let Some(err) = d.error {
                        Poll::Ready(Err(err.message.into()))
                    } else {
                        Poll::Ready(
                            serde_json::from_value(d.result.unwrap_or_default())
                                .map_err(|e| format!("cannot deserialize response: {e:?}").into()),
                        )
                    }
                }
            }
        }))
    }
}

impl RequestHandler {
    async fn handle_request(&self, request: lsp_server::Request, ctx: &Rc<Context>) -> Result<()> {
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

    #[cfg(feature = "preview-engine")]
    {
        let lsp_thread = std::thread::Builder::new()
            .name("LanguageServer".into())
            .spawn(|| {
                /// Make sure we quit the event loop even if we panic
                struct QuitEventLoop;
                impl Drop for QuitEventLoop {
                    fn drop(&mut self) {
                        preview::quit_ui_event_loop();
                    }
                }
                let quit_ui_loop = QuitEventLoop;

                let threads = match run_lsp_server() {
                    Ok(threads) => threads,
                    Err(error) => {
                        eprintln!("Error running LSP server: {}", error);
                        return;
                    }
                };

                drop(quit_ui_loop);
                threads.join().unwrap();
            })
            .unwrap();

        preview::start_ui_event_loop();
        lsp_thread.join().unwrap();
    }
    #[cfg(not(feature = "preview-engine"))]
    match run_lsp_server() {
        Ok(threads) => threads.join().unwrap(),
        Err(error) => {
            eprintln!("Error running LSP server: {}", error);
        }
    }
}

pub fn run_lsp_server() -> Result<IoThreads> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;

    let init_param: InitializeParams = serde_json::from_value(params).unwrap();
    let initialize_result =
        serde_json::to_value(language::server_initialize_result(&init_param.capabilities))?;
    connection.initialize_finish(id, initialize_result)?;

    main_loop(connection, init_param)?;

    Ok(io_threads)
}

fn main_loop(connection: Connection, init_param: InitializeParams) -> Result<()> {
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
        init_param,
        preview: Box::new(Previewer {
            server_notifier,
            #[cfg(all(not(feature = "preview-builtin"), not(feature = "preview-external")))]
            use_external_previewer: RefCell::new(false), // No preview, pick any.
            #[cfg(all(not(feature = "preview-builtin"), feature = "preview-external"))]
            use_external_previewer: RefCell::new(true), // external only
            #[cfg(all(feature = "preview-builtin", not(feature = "preview-external")))]
            use_external_previewer: RefCell::new(false), // internal only
            #[cfg(all(feature = "preview-builtin", feature = "preview-external"))]
            use_external_previewer: RefCell::new(false), // prefer internal
            to_show: RefCell::new(None),
        }),
    });

    let mut futures = Vec::<Pin<Box<dyn Future<Output = Result<()>>>>>::new();
    let mut first_future = Box::pin(load_configuration(&ctx));

    // We are waiting in this loop for two kind of futures:
    //  - The compiler future should always be ready immediately because we do not set a callback to load files
    //  - the future from `send_request` are blocked waiting for a response from the client.
    //    Responses are sent on the `connection.receiver` which will wake the loop, so there
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
                // ignore errors when shutdown
                if connection.handle_shutdown(&req).unwrap_or(false) {
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
                        OutgoingRequest::Start => { /* nothing to do */ }
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

async fn handle_notification(req: lsp_server::Notification, ctx: &Rc<Context>) -> Result<()> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                ctx,
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
                ctx,
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

        #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
        "slint/showPreview" => {
            language::show_preview_command(
                req.params.as_array().map_or(&[], |x| x.as_slice()),
                ctx,
            )?;
        }

        #[cfg(all(feature = "preview-external", feature = "preview-engine"))]
        "slint/preview_to_lsp" => {
            use common::PreviewToLspMessage as M;
            let params: M = serde_json::from_value(req.params)?;
            match params {
                M::Status { message, health } => {
                    crate::preview::send_status_notification(
                        &ctx.server_notifier,
                        &message,
                        health,
                    );
                }
                M::Diagnostics { uri, diagnostics } => {
                    crate::preview::notify_lsp_diagnostics(&ctx.server_notifier, uri, diagnostics);
                }
                M::ShowDocument { file, start_line, start_column, end_line, end_column } => {
                    crate::preview::ask_editor_to_show_document(
                        &ctx.server_notifier,
                        &file,
                        start_line,
                        start_column,
                        end_line,
                        end_column,
                    );
                }
                M::PreviewTypeChanged { is_external } => {
                    ctx.preview.set_use_external_previewer(is_external);
                }
                M::RequestState { .. } => {
                    ctx.preview.request_state(ctx);
                }
            }
        }
        _ => (),
    }
    Ok(())
}
