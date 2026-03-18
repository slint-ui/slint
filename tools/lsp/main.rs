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
use futures_util::FutureExt;
use language::*;

use lsp_types::notification::{
    DidChangeConfiguration, DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument,
    DidOpenTextDocument, Notification,
};
use lsp_types::{InitializeParams, Url, request::Initialize};

use clap::{Args, Parser, Subcommand};
use itertools::Itertools;
use std::collections::HashMap;
use std::io::Write as _;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tower::{Service, ServiceBuilder};

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
        let json = serde_json::to_string(&params).unwrap();
        tracing::debug!("Sending notification {json}");
        Ok(self.client.notify::<N>(params)?)
    }

    pub async fn send_request<R: lsp_types::request::Request>(
        &self,
        request: R::Params,
    ) -> Result<R::Result> {
        let json = serde_json::to_string(&request).unwrap();
        tracing::debug!("Sending request {json}");
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

        if let Err(error) = local_set.block_on(&rt, run_lsp_server(args)) {
            tracing::error!("Error running LSP server: {error}");
            std::process::exit(3);
        }
    }
}

async fn run_lsp_server(args: Cli) -> async_lsp::Result<()> {
    let (server, _) = async_lsp::MainLoop::new_server(move |client| {
        let mut router = Router::new(OnceLock::<Context>::new());
        register_notifications(&mut router);
        let server_notifier = ServerNotifier { client: client.clone() };

        let inner_sn = server_notifier.clone();
        router.request::<Initialize, _>(move |ctx, params| {
            let document_cache = if ctx
                .set(create_context(inner_sn.clone(), params.clone(), args.clone(), None))
                .is_err()
            {
                Err(async_lsp::ResponseError::new(
                    async_lsp::ErrorCode::INTERNAL_ERROR,
                    "Received Initialize request twice",
                ))
            } else {
                Ok(ctx.get().unwrap().document_cache.clone())
            };
            let server_notifier = inner_sn.clone();
            async move {
                let document_cache = document_cache?;
                let result = server_initialize_result(&params.capabilities);
                // Delay startup until the response to the Initialize request has been sent.
                tokio::task::spawn_local(async move {
                    match startup_lsp(&params, &server_notifier, document_cache).await {
                        Err(err) => {
                            server_notifier
                                .send_event(async_lsp::Error::Response(
                                    async_lsp::ResponseError::new(
                                        async_lsp::ErrorCode::INTERNAL_ERROR,
                                        err,
                                    ),
                                ))
                                .ok();
                        }
                        Ok(None) => {}
                        Ok(Some(config)) => {
                            server_notifier
                                .send_event(events::ConfigurePreviewEvent { config, doc_count: 0 })
                                .ok();
                        }
                    }
                });
                Ok(result)
            }
        });
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
            let ctx = ctx.get_mut().unwrap();
            ctx.preview_config = params.config.clone();
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
                    server_notifier.send_event(events::RecompileTimerEvent).ok();
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
        router.event::<events::RecompileTimerEvent>(move |ctx, _| {
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
        router.request::<lsp_types::request::Shutdown, _>(|ctx, _| {
            ctx.get_mut().unwrap().to_preview.clone().exec(|to_preview| {
                to_preview.send(&lsp_protocol::LspToPreviewMessage::Quit);
                Ok(())
            })
        });
        let mut rh = RequestHandler(router);
        register_request_handlers(&mut rh);

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .layer(ClientProcessMonitorLayer::new(client))
            .layer(tower::layer::layer_fn(LogService))
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

        LocalThreadWrapper::new(move || {
            let child_preview: Box<dyn common::LspToPreview> =
                Box::new(preview::connector::ChildProcessLspToPreview::new(sn.clone()));
            let embedded_preview: Box<dyn common::LspToPreview> =
                Box::new(preview::connector::EmbeddedLspToPreview::new(sn.clone()));
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

    let inner_init_param = init_param.clone();

    Context {
        document_cache: util::LocalThreadWrapper::new(move || {
            let compiler_config = CompilerConfiguration {
                style: Some(if cli_args.style.is_empty() {
                    "native".into()
                } else {
                    cli_args.style
                }),
                include_paths: cli_args.include_paths,
                library_paths: cli_args
                    .library_paths
                    .iter()
                    .filter_map(|entry| {
                        entry.split('=').collect_tuple().map(|(k, v)| (k.into(), v.into()))
                    })
                    .collect(),
                open_import_callback: Some(Arc::new(move |path| {
                    let to_preview = to_preview_clone.clone();
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
                                    to_preview
                                        .send(&common::LspToPreviewMessage::ForgetFile { url });
                                });
                            }
                        }
                        Some(contents.map(|c| (None, c)))
                    })
                })),
                format: if inner_init_param
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
        let server_notifier = ctx.server_notifier.clone();
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
                if let Ok(Some(config)) =
                    load_configuration(server_notifier.clone(), document_cache).await
                {
                    server_notifier
                        .send_event(events::ConfigurePreviewEvent { config, doc_count: 0 })
                        .ok();
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

// A middleware that logs requests before forwarding them to another service
pub struct LogService<S>(S);

impl<S> Service<async_lsp::AnyRequest> for LogService<S>
where
    S: Service<async_lsp::AnyRequest>,
    S::Response: serde::ser::Serialize,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = futures_util::future::Then<
        S::Future,
        std::future::Ready<std::result::Result<S::Response, S::Error>>,
        fn(
            std::result::Result<S::Response, S::Error>,
        ) -> std::future::Ready<std::result::Result<S::Response, S::Error>>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context,
    ) -> std::task::Poll<std::result::Result<(), Self::Error>> {
        self.0.poll_ready(cx)
    }

    fn call(&mut self, request: async_lsp::AnyRequest) -> Self::Future {
        let json = serde_json::to_string(&request).unwrap();
        // Log the request
        tracing::debug!("request = {json}");

        self.0.call(request).then(Self::log_response)
    }
}

impl<S> LogService<S>
where
    S: Service<async_lsp::AnyRequest>,
    S::Response: serde::ser::Serialize,
{
    fn log_response(
        response: std::result::Result<S::Response, S::Error>,
    ) -> std::future::Ready<std::result::Result<S::Response, S::Error>> {
        if let Ok(response) = &response {
            let json = serde_json::to_string(response).unwrap();
            tracing::debug!("response = {json}");
        }
        std::future::ready(response)
    }
}

impl<S> async_lsp::LspService for LogService<S>
where
    S: Service<async_lsp::AnyRequest>,
    S::Response: serde::ser::Serialize,
{
    fn notify(
        &mut self,
        notif: async_lsp::AnyNotification,
    ) -> std::ops::ControlFlow<async_lsp::Result<()>> {
        let json = serde_json::to_string(&notif).unwrap();
        tracing::debug!("Notify {json}");
        std::ops::ControlFlow::Continue(())
    }

    fn emit(&mut self, event: async_lsp::AnyEvent) -> std::ops::ControlFlow<async_lsp::Result<()>> {
        tracing::debug!("Emit {event:?}");
        std::ops::ControlFlow::Continue(())
    }
}
