// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::await_holding_refcell_ref)]

#[cfg(all(feature = "preview-engine", not(feature = "preview-builtin")))]
compile_error!("Feature preview-engine and preview-builtin need to be enabled together when building native LSP");

mod common;
mod fmt;
mod language;
#[cfg(feature = "preview-engine")]
mod preview;
mod standalone;
pub mod util;

use common::Result;
use language::*;

use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, Notification,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, Url,
};

use clap::{Args, Parser, Subcommand};
use itertools::Itertools;
use lsp_server::{Connection, ErrorCode, IoThreads, Message, RequestId, Response};
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::io::Write as _;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{atomic, Arc, Mutex};
use std::task::{Poll, Waker};

use crate::common::document_cache::CompilerConfiguration;

#[cfg(not(any(
    target_os = "windows",
    target_arch = "wasm32",
    all(target_arch = "aarch64", target_os = "linux")
)))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(any(
    target_os = "windows",
    target_arch = "wasm32",
    all(target_arch = "aarch64", target_os = "linux")
)))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

#[derive(Clone, clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Add include paths for the import statements
    #[arg(short = 'I', name = "/path/to/import", number_of_values = 1, action)]
    include_paths: Vec<std::path::PathBuf>,

    /// Specify library location of the '@library' in the form 'library=/path/to/library'
    #[arg(short = 'L', value_name = "library=path", number_of_values = 1, action)]
    library_paths: Vec<String>,

    /// The style name for the preview ('native' or 'fluent')
    #[arg(long, name = "style name", default_value_t, action)]
    style: String,

    /// The backend or renderer used for the preview ('qt', 'femtovg', 'skia' or 'software')
    #[arg(long, name = "backend", default_value_t, action)]
    backend: String,

    /// Start the preview in full screen mode
    #[arg(long, action)]
    fullscreen: bool,

    /// Hide the preview toolbar
    #[arg(long, action)]
    no_toolbar: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Format slint files
    Format(FormatCmd),
    Open(OpenCmd),
}

#[derive(Args, Clone)]
struct FormatCmd {
    #[arg(name = "path to .slint file(s)", action)]
    paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of printing to stdout
    #[arg(short, long, action)]
    inline: bool,
}

#[derive(Args, Clone)]
struct OpenCmd {
    #[arg(name = "path to .slint file", action)]
    path: std::path::PathBuf,

    /// The name of the component to view. If unset, the last exported component of the file is used.
    /// If the component name is not in the .slint file , nothing will be shown
    #[arg(long, value_name = "component name", action)]
    component: Option<String>,
}

impl Cli {
    fn into_compiler_config(
        self,
        send_message_to_preview: impl Fn(common::LspToPreviewMessage) + Clone + 'static,
    ) -> CompilerConfiguration {
        CompilerConfiguration {
            style: Some(if self.style.is_empty() { "native".into() } else { self.style }),
            include_paths: self.include_paths,
            library_paths: self
                .library_paths
                .iter()
                .filter_map(|entry| {
                    entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into()))
                })
                .collect(),
            open_import_fallback: Some(Rc::new(move |path| {
                let send_message_to_preview = send_message_to_preview.clone();
                Box::pin(async move {
                    let contents = std::fs::read_to_string(&path);
                    if let Ok(url) = Url::from_file_path(&path) {
                        if let Ok(contents) = &contents {
                            send_message_to_preview(common::LspToPreviewMessage::SetContents {
                                url: common::VersionedUrl::new(url, None),
                                contents: contents.clone(),
                            })
                        } else {
                            send_message_to_preview(common::LspToPreviewMessage::ForgetFile { url })
                        }
                    }
                    Some(contents.map(|c| (None, c)))
                })
            })),
            ..Default::default()
        }
    }
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
pub struct ServerNotifier {
    sender: crossbeam_channel::Sender<Message>,
    queue: OutgoingRequestQueue,
    use_external_preview: Arc<atomic::AtomicBool>,
    #[cfg(feature = "preview-engine")]
    preview_to_lsp_sender: crossbeam_channel::Sender<crate::common::PreviewToLspMessage>,
}

impl ServerNotifier {
    pub fn use_external_preview(&self) -> bool {
        self.use_external_preview.load(atomic::Ordering::Relaxed)
    }

    pub fn set_use_external_preview(&self, is_external: bool) {
        self.use_external_preview.store(is_external, atomic::Ordering::Release);
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
        let msg =
            Message::Request(lsp_server::Request::new(id.clone(), T::METHOD.to_string(), request));
        self.sender.send(msg)?;
        let queue = self.queue.clone();
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

    pub fn send_message_to_preview(&self, message: common::LspToPreviewMessage) {
        if self.use_external_preview() {
            let _ = self.send_notification::<common::LspToPreviewMessage>(message);
        } else {
            #[cfg(feature = "preview-builtin")]
            preview::lsp_to_preview_message(message);
        }
    }

    #[cfg(feature = "preview-engine")]
    pub fn send_message_to_lsp(&self, message: common::PreviewToLspMessage) {
        let _ = self.preview_to_lsp_sender.send(message);
    }

    #[cfg(test)]
    pub fn dummy() -> Self {
        Self {
            sender: crossbeam_channel::unbounded().0,
            queue: Default::default(),
            use_external_preview: Default::default(),
            #[cfg(feature = "preview-engine")]
            preview_to_lsp_sender: crossbeam_channel::unbounded().0,
        }
    }
}

impl RequestHandler {
    async fn handle_request(&self, request: lsp_server::Request, ctx: &Rc<Context>) -> Result<()> {
        if let Some(x) = self.0.get(&request.method.as_str()) {
            match x(request.params, ctx.clone()).await {
                Ok(r) => ctx
                    .server_notifier
                    .sender
                    .send(Message::Response(Response::new_ok(request.id, r)))?,
                Err(e) => ctx.server_notifier.sender.send(Message::Response(Response::new_err(
                    request.id,
                    match e.code {
                        LspErrorCode::InvalidParameter => ErrorCode::InvalidParams as i32,
                        LspErrorCode::InternalError => ErrorCode::InternalError as i32,
                        LspErrorCode::RequestFailed => ErrorCode::RequestFailed as i32,
                        LspErrorCode::ContentModified => ErrorCode::ContentModified as i32,
                    },
                    e.message,
                )))?,
            };
        } else {
            ctx.server_notifier.sender.send(Message::Response(Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                "Cannot handle request".into(),
            )))?;
        }
        Ok(())
    }
}

fn main() {
    let mut args: Cli = Cli::parse();
    if !args.backend.is_empty() {
        std::env::set_var("SLINT_BACKEND", &args.backend);
    }

    match args.command.take() {
        Some(Commands::Format(args)) => {
            let _ = fmt::tool::run(args.paths, args.inline).map_err(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            std::process::exit(0);
        }
        Some(Commands::Open(cmd)) => {
            let _ = standalone::open(args, cmd.path, cmd.component).map_err(|e| {
                eprintln!("{e}");
                std::process::exit(1);
            });
            std::process::exit(0);
        }
        None => {}
    }

    if let Ok(panic_log_file) = std::env::var("SLINT_LSP_PANIC_LOG") {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = std::path::Path::new(&panic_log_file).parent().map(std::fs::create_dir_all);
            if let Ok(mut file) = std::fs::File::create(&panic_log_file) {
                let _ = writeln!(
                    file,
                    "slint-lsp v{}.{}.{}",
                    env!("CARGO_PKG_VERSION_MAJOR"),
                    env!("CARGO_PKG_VERSION_MINOR"),
                    env!("CARGO_PKG_VERSION_PATCH")
                );
                let _ = if let Some(l) = info.location() {
                    writeln!(file, "{}:{}:{}", l.file(), l.line(), l.column())
                } else {
                    writeln!(file, "unknown location")
                };
                let _ = writeln!(file, "{:?}", std::backtrace::Backtrace::force_capture());
                let _ = writeln!(file, "{info}");
            }
            default_hook(info);
        }));
    }

    #[cfg(feature = "preview-engine")]
    {
        let cli_args = args.clone();
        let lsp_thread = std::thread::Builder::new()
            .name("LanguageServer".into())
            .spawn(move || {
                /// Make sure we quit the event loop even if we panic
                struct QuitEventLoop;
                impl Drop for QuitEventLoop {
                    fn drop(&mut self) {
                        preview::quit_ui_event_loop();
                    }
                }
                let quit_ui_loop = QuitEventLoop;

                let threads = match run_lsp_server(args) {
                    Ok(threads) => threads,
                    Err(error) => {
                        eprintln!("Error running LSP server: {error}");
                        return;
                    }
                };

                drop(quit_ui_loop);
                threads.join().unwrap();
            })
            .unwrap();

        preview::start_ui_event_loop(cli_args);
        lsp_thread.join().unwrap();
    }

    #[cfg(not(feature = "preview-engine"))]
    match run_lsp_server(args) {
        Ok(threads) => threads.join().unwrap(),
        Err(error) => {
            eprintln!("Error running LSP server: {}", error);
        }
    }
}

fn run_lsp_server(args: Cli) -> Result<IoThreads> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;

    let init_param: InitializeParams = serde_json::from_value(params).unwrap();
    let initialize_result =
        serde_json::to_value(language::server_initialize_result(&init_param.capabilities))?;
    connection.initialize_finish(id, initialize_result)?;

    main_loop(connection, init_param, args)?;

    Ok(io_threads)
}

fn main_loop(connection: Connection, init_param: InitializeParams, cli_args: Cli) -> Result<()> {
    let mut rh = RequestHandler::default();
    register_request_handlers(&mut rh);

    let request_queue = OutgoingRequestQueue::default();
    #[cfg_attr(not(feature = "preview-engine"), allow(unused))]
    let (preview_to_lsp_sender, preview_to_lsp_receiver) =
        crossbeam_channel::unbounded::<crate::common::PreviewToLspMessage>();

    let server_notifier = ServerNotifier {
        sender: connection.sender.clone(),
        queue: request_queue.clone(),
        use_external_preview: Default::default(),
        #[cfg(feature = "preview-engine")]
        preview_to_lsp_sender,
    };

    #[cfg(feature = "preview-builtin")]
    preview::set_server_notifier(server_notifier.clone());

    let server_notifier_ = server_notifier.clone();
    let compiler_config =
        cli_args.into_compiler_config(move |m| server_notifier_.send_message_to_preview(m));

    let ctx = Rc::new(Context {
        document_cache: RefCell::new(crate::common::DocumentCache::new(compiler_config)),
        preview_config: RefCell::new(Default::default()),
        server_notifier,
        init_param,
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: Default::default(),
    });

    let mut futures = Vec::<Pin<Box<dyn Future<Output = Result<()>>>>>::new();
    let mut first_future = Box::pin(startup_lsp(&ctx));

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

    loop {
        crossbeam_channel::select! {
            recv(connection.receiver) -> msg => {
                match msg? {
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
             },
             recv(preview_to_lsp_receiver) -> _msg => {
                // Messages from the native preview come in here:
                #[cfg(feature = "preview-engine")]
                futures.push(Box::pin(handle_preview_to_lsp_message(_msg?, &ctx)))
             },
        };

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
}

async fn handle_notification(req: lsp_server::Notification, ctx: &Rc<Context>) -> Result<()> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            open_document(
                ctx,
                params.text_document.text,
                params.text_document.uri,
                Some(params.text_document.version),
                &mut ctx.document_cache.borrow_mut(),
            )
            .await
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(req.params)?;
            close_document(ctx, params.text_document.uri).await
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            reload_document(
                ctx,
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
                Some(params.text_document.version),
                &mut ctx.document_cache.borrow_mut(),
            )
            .await
        }
        DidChangeConfiguration::METHOD => load_configuration(ctx).await,
        DidChangeWatchedFiles::METHOD => {
            let params: DidChangeWatchedFilesParams = serde_json::from_value(req.params)?;
            for fe in params.changes {
                trigger_file_watcher(ctx, fe.uri, fe.typ).await?;
            }
            Ok(())
        }

        #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
        language::SHOW_PREVIEW_COMMAND => {
            match language::show_preview_command(
                req.params.as_array().map_or(&[], |x| x.as_slice()),
                ctx,
            ) {
                Ok(()) => Ok(()),
                Err(e) => match e.code {
                    LspErrorCode::RequestFailed => ctx
                        .server_notifier
                        .send_notification::<lsp_types::notification::ShowMessage>(
                        lsp_types::ShowMessageParams {
                            typ: lsp_types::MessageType::ERROR,
                            message: e.message,
                        },
                    ),
                    _ => Err(e.message.into()),
                },
            }
        }

        // Messages from the WASM preview come in as notifications sent by the "editor":
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        "slint/preview_to_lsp" => {
            handle_preview_to_lsp_message(serde_json::from_value(req.params)?, ctx).await
        }
        _ => Ok(()),
    }
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
async fn send_workspace_edit(
    server_notifier: ServerNotifier,
    label: Option<String>,
    edit: Result<lsp_types::WorkspaceEdit>,
) -> Result<()> {
    let edit = edit?;

    let response = server_notifier
        .send_request::<lsp_types::request::ApplyWorkspaceEdit>(
            lsp_types::ApplyWorkspaceEditParams { label, edit },
        )?
        .await?;
    if !response.applied {
        return Err(response
            .failure_reason
            .unwrap_or("Operation failed, no specific reason given".into())
            .into());
    }
    Ok(())
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
async fn handle_preview_to_lsp_message(
    message: crate::common::PreviewToLspMessage,
    ctx: &Rc<Context>,
) -> Result<()> {
    use crate::common::PreviewToLspMessage as M;
    match message {
        M::Diagnostics { uri, version, diagnostics } => {
            crate::common::lsp_to_editor::notify_lsp_diagnostics(
                &ctx.server_notifier,
                uri,
                version,
                diagnostics,
            );
        }
        M::ShowDocument { file, selection, take_focus } => {
            crate::common::lsp_to_editor::send_show_document_to_editor(
                ctx.server_notifier.clone(),
                file,
                selection,
                take_focus,
            )
            .await;
        }
        M::PreviewTypeChanged { is_external } => {
            ctx.server_notifier.set_use_external_preview(is_external);
        }
        M::RequestState { .. } => {
            crate::language::request_state(ctx);
        }
        M::SendWorkspaceEdit { label, edit } => {
            let _ = send_workspace_edit(ctx.server_notifier.clone(), label, Ok(edit)).await;
        }
        M::SendShowMessage { message } => {
            ctx.server_notifier
                .send_notification::<lsp_types::notification::ShowMessage>(message)?;
        }
        M::TelemetryEvent(object) => {
            ctx.server_notifier.send_notification::<lsp_types::notification::TelemetryEvent>(
                lsp_types::OneOf::Left(object),
            )?
        }
    }
    Ok(())
}
