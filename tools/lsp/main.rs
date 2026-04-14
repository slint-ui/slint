// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::await_holding_refcell_ref)]

#[cfg(all(feature = "preview-engine", not(feature = "preview-builtin")))]
compile_error!(
    "Feature preview-engine and preview-builtin need to be enabled together when building native LSP"
);

mod common;
mod fmt;
mod language;
#[cfg(feature = "preview-engine")]
mod preview;
pub mod util;

use common::{LspToPreview, Result};
use language::*;

use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, Notification,
};
use lsp_types::{
    DidChangeTextDocumentParams, DidChangeWatchedFilesParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, InitializeParams, Url,
};
use tokio::sync::mpsc;

use clap::{Args, Parser, Subcommand};
use itertools::Itertools;
use lsp_server::{Connection, ErrorCode, IoThreads, Message, RequestId, Response};
use std::future::Future;
use std::io::Write as _;
use std::rc::Rc;
use std::sync::{Arc, atomic};
use std::task::{Poll, Waker};
use std::time::Duration;

use crate::common::document_cache::CompilerConfiguration;

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "windows",
    target_arch = "wasm32",
    all(target_arch = "aarch64", target_os = "linux")
)))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(any(
    target_os = "openbsd",
    target_os = "windows",
    target_arch = "wasm32",
    all(target_arch = "aarch64", target_os = "linux")
)))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

const RECOMPILE_IDLE_TIMEOUT: Duration = Duration::from_millis(50);

#[derive(Clone, clap::Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Add include paths for the import statements
    #[arg(short = 'I', name = "/path/to/import", number_of_values = 1, action)]
    include_paths: Vec<std::path::PathBuf>,

    /// Specify library location of the '@library' in the form 'library=/path/to/library'
    #[arg(short = 'L', value_name = "library=path", number_of_values = 1, action)]
    library_paths: Vec<String>,

    /// The style name for the preview. Defaults to 'fluent' if not specified
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
    Format(Format),
    /// Run live preview
    #[cfg(feature = "preview-engine")]
    #[command(hide(true))]
    LivePreview(LivePreview),
}

#[derive(Args, Clone)]
struct Format {
    #[arg(name = "path to .slint file(s)", action)]
    paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of printing to stdout
    #[arg(short, long, action)]
    inline: bool,
}

#[cfg(feature = "preview-engine")]
#[derive(Args, Clone, Debug)]
struct LivePreview {
    /// Run remote controlled by the LSP
    #[arg(long)]
    remote_controlled: bool,
    /// toggle fullscreen mode
    #[arg(long)]
    fullscreen: bool,
}
enum OutgoingRequest {
    Start,
    Pending(Waker),
    Done(lsp_server::Response),
}

type OutgoingRequestQueue = Arc<dashmap::DashMap<RequestId, OutgoingRequest>>;

/// A handle that can be used to communicate with the client
///
/// This type is duplicated, with the same interface, in wasm_main.rs
#[derive(Clone)]
pub struct ServerNotifier {
    sender: crossbeam_channel::Sender<Message>,
    queue: OutgoingRequestQueue,
}

impl ServerNotifier {
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
        queue.insert(id.clone(), OutgoingRequest::Start);
        Ok(std::future::poll_fn(move |ctx| match queue.remove(&id).unwrap().1 {
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
        }))
    }

    #[cfg(test)]
    pub fn dummy() -> Self {
        Self { sender: crossbeam_channel::unbounded().0, queue: Default::default() }
    }
}

impl RequestHandler {
    fn handle_request(&self, request: lsp_server::Request, ctx: &mut Context) -> Result<()> {
        if let Some(x) = self.0.get(&request.method.as_str()) {
            match x(request.params, ctx) {
                Ok(r) => {
                    ctx.server_notifier
                        .sender
                        .send(Message::Response(Response::new_ok(request.id, r)))?;
                }
                Err(e) => {
                    ctx.server_notifier.sender.send(Message::Response(Response::new_err(
                        request.id,
                        match e.code {
                            LspErrorCode::InvalidParameter => ErrorCode::InvalidParams as i32,
                            LspErrorCode::InternalError => ErrorCode::InternalError as i32,
                            LspErrorCode::RequestFailed => ErrorCode::RequestFailed as i32,
                            LspErrorCode::ContentModified => ErrorCode::ContentModified as i32,
                        },
                        e.message,
                    )))?;
                }
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
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args: Cli = Cli::parse();
    if !args.backend.is_empty() {
        // Safety: there are no other threads at this point
        unsafe {
            std::env::set_var("SLINT_BACKEND", &args.backend);
        }
    }

    if let Ok(panic_log_dir) = std::env::var("SLINT_LSP_PANIC_LOG_DIR") {
        // The editor may set the `SLINT_LSP_PANIC_LOG` env variable to a path in which we can write the panic log.
        // It will read that file if our process doesn't exit properly, and will use the content to report the panic via telemetry.
        // The content of the generated file will be the following:
        //  - The first line will be the version of slint-lsp
        //  - The second line will be the location of the panic, in the format `file:line:column`
        //  - The third line will be backtrace (in one line)
        //  - everything that follows is the actual panic message. It can span over multiple lines.
        let panic_log_file = std::path::Path::new(&panic_log_dir)
            .join(format!("slint_lsp_panic_{}.log", std::process::id()));

        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
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

    if let Some(command) = &args.command {
        match command {
            Commands::Format(fmt) => match fmt::tool::run(&fmt.paths, fmt.inline) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("Format Error: {e}");
                    std::process::exit(1)
                }
            },
            #[cfg(feature = "preview-engine")]
            Commands::LivePreview(live_preview) => match preview::run(live_preview) {
                Ok(()) => std::process::exit(0),
                Err(e) => {
                    eprintln!("Preview Error: {e}");
                    std::process::exit(2);
                }
            },
        }
    } else {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        match local_set.block_on(&rt, run_lsp_server(args)) {
            Ok(threads) => threads.join().unwrap(),
            Err(error) => {
                eprintln!("Error running LSP server: {error}");
                std::process::exit(3);
            }
        }
    }
}

async fn run_lsp_server(args: Cli) -> Result<IoThreads> {
    let (connection, io_threads) = Connection::stdio();
    let (id, params) = connection.initialize_start()?;

    let init_param: InitializeParams = serde_json::from_value(params).unwrap();
    let initialize_result =
        serde_json::to_value(language::server_initialize_result(&init_param.capabilities))?;
    connection.initialize_finish(id, initialize_result)?;

    main_loop(connection, init_param, args).await?;

    Ok(io_threads)
}

async fn main_loop(
    connection: Connection,
    init_param: InitializeParams,
    cli_args: Cli,
) -> Result<()> {
    let request_queue = OutgoingRequestQueue::default();
    #[cfg_attr(not(feature = "preview-engine"), allow(unused))]
    let (preview_to_lsp_sender, preview_to_lsp_receiver) =
        mpsc::unbounded_channel::<crate::common::PreviewToLspMessage>();

    let server_notifier =
        ServerNotifier { sender: connection.sender.clone(), queue: request_queue.clone() };

    #[cfg(not(feature = "preview-engine"))]
    let to_preview: Rc<dyn LspToPreview> = Rc::new(common::DummyLspToPreview::default());
    #[cfg(feature = "preview-engine")]
    let to_preview: Rc<dyn LspToPreview> = {
        let sn = server_notifier.clone();

        let child_preview: Box<dyn common::LspToPreview> =
            Box::new(preview::connector::ChildProcessLspToPreview::new(preview_to_lsp_sender));
        let embedded_preview: Box<dyn common::LspToPreview> =
            Box::new(preview::connector::EmbeddedLspToPreview::new(sn));
        Rc::new(
            preview::connector::SwitchableLspToPreview::new(
                std::collections::HashMap::from([
                    (common::PreviewTarget::ChildProcess, child_preview),
                    (common::PreviewTarget::EmbeddedWasm, embedded_preview),
                ]),
                common::PreviewTarget::ChildProcess,
            )
            .unwrap(),
        )
    };

    let result = run_main_loop(
        connection,
        init_param,
        cli_args,
        request_queue,
        server_notifier,
        preview_to_lsp_receiver,
        to_preview.clone(),
    )
    .await;

    to_preview.shutdown().await;

    result
}

async fn run_main_loop(
    connection: Connection,
    init_param: InitializeParams,
    cli_args: Cli,
    request_queue: OutgoingRequestQueue,
    server_notifier: ServerNotifier,
    #[cfg_attr(not(feature = "preview-engine"), allow(unused_mut))]
    mut preview_to_lsp_receiver: mpsc::UnboundedReceiver<crate::common::PreviewToLspMessage>,
    to_preview: Rc<dyn LspToPreview>,
) -> Result<()> {
    let mut rh = RequestHandler::default();
    register_request_handlers(&mut rh);

    let to_preview_clone = to_preview.clone();
    let compiler_config = CompilerConfiguration {
        style: Some(if cli_args.style.is_empty() { "fluent".into() } else { cli_args.style }),
        include_paths: cli_args.include_paths,
        library_paths: cli_args
            .library_paths
            .iter()
            .filter_map(|entry| entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into())))
            .collect(),
        open_import_callback: Some(Rc::new(move |path| {
            let to_preview = to_preview_clone.clone();
            // let server_notifier = server_notifier_.clone();
            Box::pin(async move {
                tracing::trace!("Importing file: {}", path);
                let contents = std::fs::read_to_string(&path);
                if let Ok(url) = Url::from_file_path(&path) {
                    if let Ok(contents) = &contents {
                        to_preview.send(&common::LspToPreviewMessage::SetContents {
                            url: common::VersionedUrl::new(url, None),
                            contents: contents.clone(),
                        });
                    } else {
                        to_preview.send(&common::LspToPreviewMessage::ForgetFile { url });
                    }
                }
                Some(contents.map(|c| (None, c)))
            })
        })),
        format: if init_param
            .capabilities
            .general
            .as_ref()
            .and_then(|x| x.position_encodings.as_ref())
            .is_some_and(|x| x.iter().any(|x| x == &lsp_types::PositionEncodingKind::UTF8))
        {
            common::ByteFormat::Utf8
        } else {
            common::ByteFormat::Utf16
        },
        resource_url_mapper: None,
        // The i_slint_compiler::CompilerConfiguration::default() will read the environment variable
        enable_experimental: false,
    };

    let mut ctx = Context {
        document_cache: crate::common::DocumentCache::new(compiler_config),
        preview_config: Default::default(),
        server_notifier,
        init_param,
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: Default::default(),
        to_preview,
        pending_recompile: Default::default(),
    };

    let connection = Arc::new(connection);
    let (from_lsp_sender, mut from_lsp_receiver) = mpsc::unbounded_channel();
    let inner_connection = connection.clone();
    let adapter_thread = std::thread::spawn(move || {
        crossbeam_tokio_adapter(inner_connection, from_lsp_sender, request_queue);
        tracing::debug!("crossbeam -> tokio adapter exited");
    });

    startup_lsp(&mut ctx).await?;

    loop {
        let recompile_idle_timeout =
            if ctx.pending_recompile.is_empty() { Duration::MAX } else { RECOMPILE_IDLE_TIMEOUT };
        tokio::select! {
            msg = from_lsp_receiver.recv() => {
                if let Some(msg) = msg {
                    if handle_lsp_message(
                        msg,
                        &connection,
                        &mut rh,
                        &mut ctx,
                    ).await? {
                        tracing::debug!("LSP shutdown requested");
                        adapter_thread.join().expect("Failed to join adapter thread");
                        return Ok(());
                    }
                } else {
                    adapter_thread.join().expect("Failed to join adapter thread");
                    return Err("LSP connection closed".into());
                }
            }
            _msg = preview_to_lsp_receiver.recv() => {
                // Messages from the native preview come in here:
                #[cfg(feature = "preview-engine")]
                {
                    if let Some(msg) = _msg && let Err(err) = handle_preview_to_lsp_message(msg, &ctx).await {
                        tracing::error!("handle_preview_to_lsp_message: {err}");
                    }
                }
            }
            _ = tokio::time::sleep(recompile_idle_timeout) => {
                tracing::debug!("LSP recompiling");
                let pending_recompile = std::mem::take(&mut ctx.pending_recompile);

                for url in pending_recompile {
                    if let Err(err) = language::reload_document(&mut ctx, url).await {
                        tracing::error!("Failed document reload: {err}");
                    }
                }
            }
        }
    }
}

async fn handle_lsp_message(
    msg: Message,
    connection: &Arc<Connection>,
    rh: &mut RequestHandler,
    ctx: &mut Context,
) -> Result<bool> {
    match msg {
        Message::Request(req) => {
            // ignore errors when shutdown
            if connection.handle_shutdown(&req).unwrap_or(false) {
                return Ok(true);
            }
            rh.handle_request(req, ctx)?;
        }
        Message::Response(_) => {
            // should not be receiving responses, since they're handled in the dedicated thread
        }
        Message::Notification(notification) => {
            handle_notification(notification, ctx).await?;
        }
    }
    Ok(false)
}

/// Crossbeam does not play well with async (it's always blocking), so we need to run a separate
/// thread just to relay messages between a Crossbeam channel and an async channel.
/// We need Crossbeam because we're using lsp-server, which does not support anything else.
fn crossbeam_tokio_adapter(
    connection: Arc<Connection>,
    from_lsp_sender: mpsc::UnboundedSender<Message>,
    request_queue: OutgoingRequestQueue,
) {
    loop {
        match connection.receiver.recv() {
            Ok(Message::Response(resp)) => {
                let Some(mut q) = request_queue.get_mut(&resp.id) else {
                    tracing::error!("Response to unknown request");
                    continue;
                };
                match &*q {
                    OutgoingRequest::Done(_) => {
                        tracing::error!("Response to unknown request");
                        continue;
                    }
                    OutgoingRequest::Start => { /* nothing to do */ }
                    OutgoingRequest::Pending(x) => x.wake_by_ref(),
                };
                *q = OutgoingRequest::Done(resp);
            }
            Ok(msg) => {
                if from_lsp_sender.send(msg.clone()).is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }
}

async fn handle_notification(req: lsp_server::Notification, ctx: &mut Context) -> Result<()> {
    match &*req.method {
        DidOpenTextDocument::METHOD => {
            let params: DidOpenTextDocumentParams = serde_json::from_value(req.params)?;
            open_document(
                ctx,
                params.text_document.text,
                params.text_document.uri,
                Some(params.text_document.version),
            )
            .await
        }
        DidCloseTextDocument::METHOD => {
            let params: DidCloseTextDocumentParams = serde_json::from_value(req.params)?;
            close_document(ctx, params.text_document.uri).await
        }
        DidChangeTextDocument::METHOD => {
            let mut params: DidChangeTextDocumentParams = serde_json::from_value(req.params)?;
            tracing::debug!(
                "Document changed: {} (version: {})",
                params.text_document.uri,
                params.text_document.version
            );
            load_document(
                ctx,
                params.content_changes.pop().unwrap().text,
                params.text_document.uri,
                Some(params.text_document.version),
            )
            .await
        }
        DidChangeConfiguration::METHOD => load_configuration(ctx).await,
        DidChangeWatchedFiles::METHOD => {
            let params: DidChangeWatchedFilesParams = serde_json::from_value(req.params)?;
            for fe in params.changes {
                tracing::debug!("Watched file changed: {} (type: {:?})", fe.uri, fe.typ);
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
    ctx: &Context,
) -> Result<()> {
    use crate::common::PreviewToLspMessage as M;
    match message {
        M::Diagnostics { uri, version, diagnostics } => {
            if diagnostics.is_empty() {
                // This is very common, so we log it at trace level
                tracing::trace!("Preview: Empty diagnostics {}", uri);
            } else {
                tracing::debug!("Preview: {} diagnostics for {}", diagnostics.len(), uri);
            }
            crate::common::lsp_to_editor::notify_lsp_diagnostics(
                &ctx.server_notifier,
                uri,
                version,
                diagnostics,
            );
        }
        M::ShowDocument { file, selection, take_focus } => {
            let sn = ctx.server_notifier.clone();
            crate::common::lsp_to_editor::send_show_document_to_editor(
                sn, file, selection, take_focus,
            )
            .await;
        }
        M::PreviewTypeChanged { is_external } => {
            tracing::debug!("Preview type changed: is_external={}", is_external);
            if is_external {
                ctx.to_preview.set_preview_target(common::PreviewTarget::EmbeddedWasm)?;
            } else {
                ctx.to_preview.set_preview_target(common::PreviewTarget::ChildProcess)?;
            }
        }
        M::RequestState { .. } => {
            tracing::debug!("Preview requested state");
            crate::language::send_state_to_preview(ctx);
        }
        M::SendWorkspaceEdit { label, edit } => {
            let sn = ctx.server_notifier.clone();
            let _ = send_workspace_edit(sn, label, Ok(edit)).await;
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
