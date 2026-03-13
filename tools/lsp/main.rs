// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::await_holding_refcell_ref)]

#[cfg(all(feature = "preview-engine", not(feature = "preview-builtin")))]
compile_error!(
    "Feature preview-engine and preview-builtin need to be enabled together when building native LSP"
);

mod common;
mod events;
mod fmt;
mod language;
#[cfg(feature = "preview-engine")]
mod preview;
mod request_handler;
pub mod util;

use async_lsp::ClientSocket;
use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router::Router;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use common::Result;
use i_slint_compiler::diagnostics::BuildDiagnostics;
use language::*;

use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, Notification,
};
use lsp_types::request::Initialize;
use lsp_types::{InitializeParams, InitializeResult, Url};

use clap::{Args, Parser, Subcommand};
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::io::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;
use tower::ServiceBuilder;

use crate::common::document_cache::CompilerConfiguration;
use crate::request_handler::RequestHandler;

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

const RECOMPILE_TIMEOUT: Duration = Duration::from_millis(50);

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

/// A handle that can be used to communicate with the client
///
/// This type is duplicated, with the same interface, in wasm_main.rs
#[derive(Clone)]
pub struct ServerNotifier {
    client: ClientSocket,
}

impl ServerNotifier {
    pub fn send_notification<N: Notification>(&self, params: N::Params) -> Result<()> {
        Ok(self.client.notify::<N>(params)?)
    }

    pub async fn send_request<R: lsp_types::request::Request>(
        &self,
        request: R::Params,
    ) -> Result<R::Result> {
        Ok(self.client.request::<R>(request).await?)
    }

    pub fn send_event<E: Send + 'static>(&self, event: E) -> Result<()> {
        Ok(self.client.emit(event)?)
    }

    #[cfg(test)]
    pub fn dummy() -> Self {
        Self { client: ClientSocket::new_closed() }
    }
}

// impl request_handler::RequestHandler {
// async fn handle_request(&self, request: lsp_server::Request, ctx: &Rc<Context>) -> Result<()> {
//     if let Some(x) = self.0.get(&request.method.as_str()) {
//         match x(request.params, ctx.clone()).await {
//             Ok(r) => ctx
//                 .server_notifier
//                 .sender
//                 .send(Message::Response(Response::new_ok(request.id, r)))?,
//             Err(e) => ctx.server_notifier.sender.send(Message::Response(Response::new_err(
//                 request.id,
//                 match e.code {
//                     LspErrorCode::InvalidParameter => ErrorCode::InvalidParams as i32,
//                     LspErrorCode::InternalError => ErrorCode::InternalError as i32,
//                     LspErrorCode::RequestFailed => ErrorCode::RequestFailed as i32,
//                     LspErrorCode::ContentModified => ErrorCode::ContentModified as i32,
//                 },
//                 e.message,
//             )))?,
//         };
//     } else {
//         tracing::error!("Unable to handle request {}", request.method);
//         ctx.server_notifier.sender.send(Message::Response(Response::new_err(
//             request.id,
//             ErrorCode::MethodNotFound as i32,
//             "Cannot handle request".into(),
//         )))?;
//     }
//     Ok(())
// }
// }

#[tokio::main(flavor = "current_thread")]
async fn main() {
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
    } else if let Err(error) = run_lsp_server(args).await {
        tracing::error!("Error running LSP server: {error}");
        std::process::exit(3);
    }
}

async fn run_lsp_server(args: Cli) -> async_lsp::Result<()> {
    let (server, _) = async_lsp::MainLoop::new_server(move |client| {
        let mut router = Router::new(OnceLock::<Context>::new());
        register_notifications(&mut router);
        let server_notifier = ServerNotifier { client };

        router.request::<Initialize, _>(move |ctx, params| {
            let ctx = ctx.get().unwrap();
            let document_cache = ctx.document_cache.clone();
            let server_notifier = ctx.server_notifier.clone();
            async move {
                let preview_config =
                    startup_lsp(&params, server_notifier, document_cache).await.map_err(|err| {
                        async_lsp::ResponseError::new(async_lsp::ErrorCode::INTERNAL_ERROR, err)
                    })?;
                client.emit(SetContextEvent(params, preview_config)).ok();
                Ok(InitializeResult::default())
            }
        });
        router.event::<events::SetContextEvent>(move |ctx, params| {
            let new_ctx = create_context(server_notifier, params.0, args.clone(), params.1);
            if ctx.set(new_ctx).is_err() {
                std::ops::ControlFlow::Break(Err(async_lsp::Error::Response(
                    async_lsp::ResponseError::new(
                        async_lsp::ErrorCode::INTERNAL_ERROR,
                        "Received SetContextEvent twice",
                    ),
                )))
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        let inner_sn = server_notifier.clone();
        router.event::<events::SendDiagnosticsEvent>(move |ctx, params| {
            let ctx = ctx.get().unwrap();
            let server_notifier = ctx.server_notifier.clone();
            tokio::task::spawn_local(ctx.document_cache.clone().exec(move |document_cache| {
                send_diagnostics(server_notifier, document_cache, &params.extra_files, params.diag);
            }));
            std::ops::ControlFlow::Continue(())
        });
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        router.event::<events::ConfigurePreviewEvent>(move |ctx, params| {
            let ctx = ctx.get().unwrap();
            ctx.to_preview.oneway(move |to_preview| {
                to_preview
                    .send(&common::LspToPreviewMessage::SetConfiguration { config: params.config });
            });
            if let Some(c) = ctx.to_show.clone() {
                tracing::debug!(
                    "Sending state to preview: {} documents, showing {}",
                    params.doc_count,
                    c.url
                );
                ctx.to_preview.oneway(move |to_preview| {
                    to_preview.send(&common::LspToPreviewMessage::ShowPreview(c));
                });
            } else {
                tracing::debug!(
                    "Sending state to preview: {} documents, showing default component",
                    params.doc_count
                );
            }

            std::ops::ControlFlow::Continue(())
        });
        router.event::<events::LoadDocumentEvent>(move |ctx, params| {
            tokio::task::spawn_local(language::load_document(
                ctx.get_mut().unwrap(),
                params.content,
                params.url,
                params.version,
            ));
            std::ops::ControlFlow::Continue(())
        });
        router.event::<events::AddRecompile>(move |ctx, params| {
            let ctx = ctx.get_mut().unwrap();
            let open_dependencies = ctx.open_urls.intersection(&params.0).cloned();
            ctx.pending_recompile.extend(open_dependencies);

            #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
            if let Some(preview_url) = ctx.to_show.as_ref().map(|c| c.url.clone()) {
                // The external preview only has access to the files the LSP recompiles, so we need to
                // ensure the preview file is recompiled if anything it depends on changes, even if it's
                // not in the open_urls.
                if params.0.contains(&preview_url) {
                    ctx.pending_recompile.insert(preview_url);
                }
            }
            let server_notifier = ctx.server_notifier.clone();

            if let Some(old_timer) =
                ctx.recompile_timer.replace(tokio::task::spawn_local(async move {
                    tokio::time::sleep(RECOMPILE_TIMEOUT).await;
                    server_notifier.send_event(RecompileTimerEvent).ok();
                }))
            {
                old_timer.abort();
            }

            std::ops::ControlFlow::Continue(())
        });
        #[cfg(feature = "preview-engine")]
        router.event::<lsp_protocol::PreviewToLspMessage>(|ctx, msg| {
            if let Err(err) = handle_preview_to_lsp_message(msg, ctx.get_mut().unwrap()) {
                std::ops::ControlFlow::Break(Err(async_lsp::Error::Response(
                    async_lsp::ResponseError::new(async_lsp::ErrorCode::INTERNAL_ERROR, err),
                )))
            } else {
                std::ops::ControlFlow::Continue(())
            }
        });
        router.event::<RecompileTimerEvent>(move |ctx, _| {
            let ctx = ctx.get_mut().unwrap();
            let server_notifier = ctx.server_notifier.clone();
            let mut futures = Vec::with_capacity(ctx.pending_recompile.len());
            // can't use Iterator::map here due to mutable borrow rules for &mut Context
            for url in ctx.pending_recompile.drain().collect::<Vec<_>>() {
                futures.push(language::reload_document(ctx, url));
            }
            let joined = futures_util::future::try_join_all(futures);
            tokio::task::spawn_local(async move {
                if let Err(err) = joined.await {
                    server_notifier.send_event(err).ok();
                }
            });
            std::ops::ControlFlow::Continue(())
        });
        router.event::<async_lsp::Error>(|_ctx, error| std::ops::ControlFlow::Break(Err(error)));
        let mut rh = RequestHandler(router);
        register_request_handlers(&mut rh);
        // TODO: register notification handlers

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .layer(ClientProcessMonitorLayer::new(client))
            .service(rh.0)
    });

    // Prefer truly asynchronous piped stdin/stdout without blocking tasks.
    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap(),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap(),
    );
    // Fallback to spawn blocking read/write otherwise.
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    server.run_buffered(stdin, stdout).await

    // let (connection, io_threads) = Connection::stdio();
    // let (id, params) = connection.initialize_start()?;

    // let init_param: InitializeParams = serde_json::from_value(params).unwrap();
    // let initialize_result =
    //     serde_json::to_value(language::server_initialize_result(&init_param.capabilities))?;
    // connection.initialize_finish(id, initialize_result)?;

    // main_loop(connection, init_param, args)?;

    // Ok(io_threads)
}

fn create_context(
    server_notifier: ServerNotifier,
    init_param: InitializeParams,
    cli_args: Cli,
    preview_config: Option<common::PreviewConfig>,
) -> Context {
    #[cfg(not(feature = "preview-engine"))]
    let to_preview = {
        LocalThreadWrapper::new(|| {
            preview::connector::SwitchableLspToPreview::with_one(common::DummyLspToPreview {})
        })
    };
    #[cfg(feature = "preview-engine")]
    let to_preview = {
        use crate::util::LocalThreadWrapper;

        let sn = server_notifier.clone();

        LocalThreadWrapper::new(|| {
            let child_preview: Box<dyn common::LspToPreview> = Box::new(
                preview::connector::ChildProcessLspToPreview::new(server_notifier.clone()),
            );
            let embedded_preview: Box<dyn common::LspToPreview> =
                Box::new(preview::connector::EmbeddedLspToPreview::new(sn.clone()));
            #[cfg(feature = "preview-remote")]
            let remote_preview: Box<dyn common::LspToPreview> =
                Box::new(preview::connector::RemoteLspToPreview::new(sn));
            preview::connector::SwitchableLspToPreview::new(
                HashMap::from([
                    (common::PreviewTarget::ChildProcess, child_preview),
                    (common::PreviewTarget::EmbeddedWasm, embedded_preview),
                ]),
                common::PreviewTarget::ChildProcess,
            )
            .unwrap()
        })
    };

    let to_preview_clone = to_preview.clone();
    let compiler_config = CompilerConfiguration {
        style: Some(if cli_args.style.is_empty() { "native".into() } else { cli_args.style }),
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
                        let contents = contents.clone().into();
                        to_preview.oneway(move |to_preview| {
                            to_preview.send(&common::LspToPreviewMessage::SetContents {
                                url: common::VersionedUrl::new(url, None),
                                contents,
                            });
                        });
                    } else {
                        to_preview.oneway(move |to_preview| {
                            to_preview.send(&common::LspToPreviewMessage::ForgetFile { url });
                        });
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

    Context {
        document_cache: util::LocalThreadWrapper::new(|| {
            crate::common::DocumentCache::new(compiler_config)
        }),
        preview_config: preview_config.unwrap_or_default(),
        server_notifier,
        init_param,
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: Default::default(),
        to_preview,
        pending_recompile: Default::default(),
        recompile_timer: None,
    }
}

// fn main_loop(connection: Connection, init_param: InitializeParams, cli_args: Cli) -> Result<()> {
//     let runtime =
//         tokio::runtime::Builder::new_current_thread().enable_io().enable_time().build()?;

//     runtime.block_on(async move {
//         let mut rh = RequestHandler::default();
//         register_request_handlers(&mut rh);

//         let request_queue = OutgoingRequestQueue::default();
//         #[cfg_attr(not(feature = "preview-engine"), allow(unused))]
//         let (preview_to_lsp_sender, preview_to_lsp_receiver) =
//             crossbeam_channel::unbounded::<crate::common::PreviewToLspMessage>();

//         let server_notifier =
//             ServerNotifier { sender: connection.sender.clone(), queue: request_queue.clone() };

//         #[cfg(not(feature = "preview-engine"))]
//         let to_preview = {
//             Rc::new(preview::connector::SwitchableLspToPreview::with_one(common::DummyLspToPreview {}))
//         };
//         #[cfg(feature = "preview-engine")]
//         let to_preview = {
//             let sn = server_notifier.clone();

//             let child_preview: Box<dyn common::LspToPreview> = Box::new(
//                 preview::connector::ChildProcessLspToPreview::new(preview_to_lsp_sender.clone()),
//             );
//             let embedded_preview: Box<dyn common::LspToPreview> =
//                 Box::new(preview::connector::EmbeddedLspToPreview::new(sn.clone()));
//             #[cfg(feature = "preview-remote")]
//             let remote_preview: Box<dyn common::LspToPreview> =
//                 Box::new(preview::connector::RemoteLspToPreview::new(preview_to_lsp_sender, sn));
//             Rc::new(
//                 preview::connector::SwitchableLspToPreview::new(
//                     HashMap::from([
//                         (common::PreviewTarget::ChildProcess, child_preview),
//                         (common::PreviewTarget::EmbeddedWasm, embedded_preview),
//                         #[cfg(feature = "preview-remote")]
//                         (common::PreviewTarget::Remote, remote_preview),
//                     ]),
//                     common::PreviewTarget::ChildProcess,
//                 )
//                 .unwrap(),
//             )
//         };

//         let to_preview_clone = to_preview.clone();
//         let compiler_config = CompilerConfiguration {
//             style: Some(if cli_args.style.is_empty() { "native".into() } else { cli_args.style }),
//             include_paths: cli_args.include_paths,
//             library_paths: cli_args
//                 .library_paths
//                 .iter()
//                 .filter_map(|entry| entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into())))
//                 .collect(),
//             open_import_callback: Some(Rc::new(move |path| {
//                 let to_preview = to_preview_clone.clone();
//                 // let server_notifier = server_notifier_.clone();
//                 Box::pin(async move {
//                     tracing::trace!("Importing file: {}", path);
//                     let contents = std::fs::read_to_string(&path);
//                     if let Ok(url) = Url::from_file_path(&path) {
//                         if let Ok(contents) = &contents {
//                             to_preview.send(&common::LspToPreviewMessage::SetContents {
//                                 url: common::VersionedUrl::new(url, None),
//                                 contents: contents.clone().into(),
//                             });
//                         } else {
//                             to_preview.send(&common::LspToPreviewMessage::ForgetFile { url });
//                         }
//                     }
//                     Some(contents.map(|c| (None, c)))
//                 })
//             })),
//             format: if init_param
//                 .capabilities
//                 .general
//                 .as_ref()
//                 .and_then(|x| x.position_encodings.as_ref())
//                 .is_some_and(|x| x.iter().any(|x| x == &lsp_types::PositionEncodingKind::UTF8))
//             {
//                 common::ByteFormat::Utf8
//             } else {
//                 common::ByteFormat::Utf16
//             },
//             resource_url_mapper: None,
//             // The i_slint_compiler::CompilerConfiguration::default() will read the environment variable
//             enable_experimental: false,
//         };

//         let ctx = Rc::new(Context {
//             document_cache: crate::common::DocumentCache::new(compiler_config),
//             preview_config: Default::default(),
//             server_notifier,
//             init_param,
//             #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
//             to_show: Default::default(),
//             open_urls: Default::default(),
//             to_preview,
//             pending_recompile: Default::default(),
//         });

//         let mut futures = Vec::<Pin<Box<dyn Future<Output = Result<()>>>>>::new();
//         let mut first_future = Box::pin(startup_lsp(&ctx));

//         // We are waiting in this loop for two kind of futures:
//         //  - The compiler future should always be ready immediately because we do not set a callback to load files
//         //  - the future from `send_request` are blocked waiting for a response from the client.
//         //    Responses are sent on the `connection.receiver` which will wake the loop, so there
//         //    is no need to do anything in the waker.
//         struct DummyWaker;
//         impl std::task::Wake for DummyWaker {
//             fn wake(self: Arc<Self>) {}
//         }
//         let waker = Arc::new(DummyWaker).into();
//         match first_future.as_mut().poll(&mut std::task::Context::from_waker(&waker)) {
//             Poll::Ready(x) => x?,
//             Poll::Pending => futures.push(first_future),
//         };

//         loop {
//             let recompile_timeout = if ctx.pending_recompile.borrow().is_empty() {
//                 crossbeam_channel::never()
//             } else {
//                 crossbeam_channel::after(std::time::Duration::from_millis(50))
//             };
//             crossbeam_channel::select! {
//                 recv(connection.receiver) -> msg => {
//                     match msg? {
//                         Message::Request(req) => {
//                             // ignore errors when shutdown
//                             if connection.handle_shutdown(&req).unwrap_or(false) {
//                                 return Ok(());
//                             }
//                             futures.push(Box::pin(rh.handle_request(req, &ctx)));
//                         }
//                         Message::Response(resp) => {
//                             if let Some(q) = request_queue.lock().unwrap().get_mut(&resp.id) {
//                                 match q {
//                                     OutgoingRequest::Done(_) => {
//                                         return Err("Response to unknown request".into())
//                                     }
//                                     OutgoingRequest::Start => { /* nothing to do */ }
//                                     OutgoingRequest::Pending(x) => x.wake_by_ref(),
//                                 };
//                                 *q = OutgoingRequest::Done(resp)
//                             } else {
//                                 return Err("Response to unknown request".into());
//                             }
//                         }
//                         Message::Notification(notification) => {
//                             futures.push(Box::pin(handle_notification(notification, &ctx)))
//                         }
//                     }
//                  },
//                  recv(preview_to_lsp_receiver) -> _msg => {
//                     // Messages from the native preview come in here:
//                     #[cfg(feature = "preview-engine")]
//                     futures.push(Box::pin(handle_preview_to_lsp_message(_msg?, &ctx)))
//                  },
//                  recv(recompile_timeout) -> _ => {
//                      let pending_recompile = std::mem::take(&mut *ctx.pending_recompile.borrow_mut());

//                      for url in pending_recompile {
//                          futures.push(Box::pin(language::reload_document(&ctx, url)));
//                      }
//                  }
//             };

//             let mut result = Ok(());
//             futures.retain_mut(|f| {
//                 if result.is_err() {
//                     return true;
//                 }
//                 match f.as_mut().poll(&mut std::task::Context::from_waker(&waker)) {
//                     Poll::Ready(x) => {
//                         result = x;
//                         false
//                     }
//                     Poll::Pending => true,
//                 }
//             });
//             result?;
//         }
//     })
// }

#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
struct ShowPreviewCommandNotification;

#[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
impl lsp_types::notification::Notification for ShowPreviewCommandNotification {
    const METHOD: &'static str = language::SHOW_PREVIEW_COMMAND;
    type Params = common::PreviewComponent;
}

fn register_notifications(router: &mut Router<OnceLock<Context>>) {
    router.notification::<DidOpenTextDocument>(|ctx, params| {
        tokio::task::spawn_local(open_document(
            ctx.get_mut().unwrap(),
            params.text_document.text,
            params.text_document.uri,
            Some(params.text_document.version),
        ));
        std::ops::ControlFlow::Continue(())
    });
    router.notification::<DidCloseTextDocument>(|ctx, params| {
        let future = close_document(ctx.get_mut().unwrap(), params.text_document.uri);
        let server_notifier = ctx.get().unwrap().server_notifier.clone();
        tokio::task::spawn_local(async move {
            if let Err(err) = future.await {
                server_notifier
                    .send_event(async_lsp::Error::Response(async_lsp::ResponseError::new(
                        async_lsp::ErrorCode::INTERNAL_ERROR,
                        err,
                    )))
                    .ok();
            }
        });
        std::ops::ControlFlow::Continue(())
    });
    router.notification::<DidChangeTextDocument>(|ctx, mut params| {
        tracing::debug!(
            "Document changed: {} (version: {})",
            params.text_document.uri,
            params.text_document.version
        );
        tokio::task::spawn_local(load_document(
            ctx.get_mut().unwrap(),
            params.content_changes.pop().unwrap().text,
            params.text_document.uri,
            Some(params.text_document.version),
        ));
        std::ops::ControlFlow::Continue(())
    });
    router.notification::<DidChangeConfiguration>(|ctx, _params| {
        let ctx = ctx.get_mut().unwrap();
        let client = ctx.server_notifier.client.clone();
        let document_cache = ctx.document_cache.clone();
        if ctx
            .init_param
            .capabilities
            .workspace
            .as_ref()
            .and_then(|w| w.configuration)
            .unwrap_or(false)
        {
            tokio::task::spawn_local(async move {
                if let Ok(Some(config)) = load_configuration(client.clone(), document_cache).await {
                    client.emit(ConfigurePreviewEvent { config, doc_count: 0 }).ok();
                }
            });
        }
        std::ops::ControlFlow::Continue(())
    });
    router.notification::<DidChangeWatchedFiles>(|ctx, params| {
        let ctx = ctx.get_mut().unwrap();
        for fe in params.changes {
            tracing::debug!("Watched file changed: {} (type: {:?})", fe.uri, fe.typ);
            let future = trigger_file_watcher(ctx, fe.uri, fe.typ);
            let server_notifier = ctx.server_notifier.clone();
            tokio::task::spawn_local(async move {
                if let Err(err) = future.await {
                    server_notifier
                        .send_event(async_lsp::Error::Response(async_lsp::ResponseError::new(
                            async_lsp::ErrorCode::INTERNAL_ERROR,
                            err,
                        )))
                        .ok();
                }
            });
        }
        std::ops::ControlFlow::Continue(())
    });
    #[cfg(any(feature = "preview-builtin", feature = "preview-external"))]
    router.notification::<ShowPreviewCommandNotification>(|ctx, params| {
        match language::show_preview_command(params, ctx.get_mut().unwrap()) {
            Ok(()) => std::ops::ControlFlow::Continue(()),
            Err(e) => match e.code {
                LspErrorCode::RequestFailed => match ctx
                    .get()
                    .unwrap()
                    .server_notifier
                    .send_notification::<lsp_types::notification::ShowMessage>(
                    lsp_types::ShowMessageParams {
                        typ: lsp_types::MessageType::ERROR,
                        message: e.message,
                    },
                ) {
                    Ok(()) => std::ops::ControlFlow::Continue(()),
                    Err(err) => std::ops::ControlFlow::Break(Err(async_lsp::Error::Response(
                        async_lsp::ResponseError::new(async_lsp::ErrorCode::INTERNAL_ERROR, err),
                    ))),
                },
                _ => std::ops::ControlFlow::Break(Err(async_lsp::Error::Response(
                    async_lsp::ResponseError::new(e.code.into(), e.message),
                ))),
            },
        }
    });

    // Messages from the WASM preview come in as notifications sent by the "editor":
    #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
    router.notification::<common::PreviewToLspMessage>(|ctx, params| {
        if let Err(err) = handle_preview_to_lsp_message(params, ctx.get_mut().unwrap()) {
            std::ops::ControlFlow::Break(Err(async_lsp::Error::Response(
                async_lsp::ResponseError::new(async_lsp::ErrorCode::INTERNAL_ERROR, err),
            )))
        } else {
            std::ops::ControlFlow::Continue(())
        }
    });
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
        )
        .await?;
    if !response.applied {
        anyhow::bail!(
            response.failure_reason.unwrap_or("Operation failed, no specific reason given".into())
        );
    }
    Ok(())
}

#[cfg(any(feature = "preview-external", feature = "preview-engine"))]
fn handle_preview_to_lsp_message(
    message: crate::common::PreviewToLspMessage,
    ctx: &mut Context,
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
            tokio::task::spawn_local(crate::common::lsp_to_editor::send_show_document_to_editor(
                ctx.server_notifier.clone(),
                file,
                selection,
                take_focus,
            ));
        }
        M::PreviewTypeChanged { target } => {
            tracing::debug!("Preview type changed: target={target:?}");
            ctx.to_preview.oneway(move |to_preview| {
                if let Err(err) = to_preview.set_preview_target(target) {
                    tracing::error!("Failed setting preview target: {err}");
                }
            });
        }
        M::RequestState { .. } => {
            tracing::debug!("Preview requested state");
            crate::language::send_state_to_preview(ctx);
        }
        M::SendWorkspaceEdit { label, edit } => {
            tokio::task::spawn_local(send_workspace_edit(
                ctx.server_notifier.clone(),
                label,
                Ok(edit),
            ));
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

pub(crate) use tokio::task::JoinHandle;

pub(crate) fn spawn_local<R: 'static>(
    future: impl std::future::Future<Output = R> + 'static,
) -> JoinHandle<R> {
    tokio::task::spawn_local(future)
}
