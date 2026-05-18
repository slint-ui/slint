// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
use std::{
    pin::Pin,
    rc::Rc,
    sync::{Arc, atomic},
    task::{Poll, Waker},
    time::Duration,
};

use i_slint_preview_protocol::{
    LspToPreviewMessage, PreviewToLspMessage, SourceFileVersion, VersionedUrl,
};
use lsp_server::{Message, RequestId};
use lsp_types::{FileChangeType, MessageType, Url, notification::Notification};
use slint_interpreter::{FileChangeKind, FileWatcher, WatchEvent};

use crate::{
    common::{self, LspToPreview, Result, document_cache::OpenImportCallback},
    language, preview,
    preview::connector::EmbeddedLspToPreview,
};

pub fn editor_main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    use clap::Parser;

    let cli = Cli::parse();

    let (to_lsp, from_preview) = crossbeam_channel::unbounded();
    let (to_preview, from_lsp) = crossbeam_channel::unbounded();
    let request_queue = OutgoingRequestQueue::default();

    // TODO: Remove the ServerNotifier, we want to keep the "LSP" abstraction
    // as much out of the visual editor as possible.
    let notifier = ServerNotifier { sender: to_preview, queue: request_queue };

    let to_preview = EmbeddedLspToPreview::new(notifier.clone());

    let to_lsp =
        Rc::new(EmbeddedPreviewToLsp { sender: to_lsp }) as Rc<dyn common::PreviewToLsp + 'static>;

    start_lsp_thread(from_preview, to_preview, notifier, cli);

    start_processing_lsp_messages_thread(from_lsp);

    preview::run(to_lsp, false, true).unwrap();
}

// TODO: Deduplicate with main.rs
pub enum OutgoingRequest {
    Start,
    Pending(Waker),
    Done(lsp_server::Response),
}

// TODO: Deduplicate with main.rs
pub type OutgoingRequestQueue = Arc<dashmap::DashMap<RequestId, OutgoingRequest>>;

// TODO: Deduplicate with main.rs
/// A handle that can be used to communicate with the client
///
/// This type is duplicated, with the same interface, in main.rs and wasm_main.rs
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

struct EmbeddedPreviewToLsp {
    sender: crossbeam_channel::Sender<PreviewToLspMessage>,
}

impl common::PreviewToLsp for EmbeddedPreviewToLsp {
    fn send(&self, message: &i_slint_preview_protocol::PreviewToLspMessage) -> common::Result<()> {
        self.sender.send(message.clone())?;
        Ok(())
    }
}

#[derive(clap::Parser)]
struct Cli {
    file: String,
    component: Option<String>,
}

fn start_processing_lsp_messages_thread(from_lsp: crossbeam_channel::Receiver<Message>) {
    // Ensure the backend is set up before the reader thread starts. This fixes
    // bug #10274 on macOS where a race condition was causing the reader thread to already
    // process messages before the event loop was running.
    //
    // Use .ok() to ignore any errors, as the backend might already be set by the user and that's fine.
    slint::BackendSelector::new().select().ok();
    std::thread::spawn(move || {
        if let Err(err) = process_lsp_messages(from_lsp) {
            tracing::error!("LSP message processing thread exited with error: {err}");
        }
    });
}

fn process_lsp_messages(from_lsp: crossbeam_channel::Receiver<Message>) -> common::Result<()> {
    while let Ok(msg) = from_lsp.recv() {
        match msg {
            Message::Notification(notification) => {
                if notification.method == LspToPreviewMessage::METHOD {
                    // TODO: Error handling!
                    let message: LspToPreviewMessage = serde_json::from_value(notification.params)?;

                    slint::invoke_from_event_loop(move || {
                        preview::connector::lsp_to_preview(message);
                    })
                    .map_err(|err| {
                        let err = err.to_string();
                        tracing::error!("Failed to queue message onto event loop - reader thread will exit: {err}");
                        err
                    })?;
                } else {
                    tracing::debug!("Silently ignoring notification from LSP: {:?}", notification);
                }
            }
            msg => {
                tracing::debug!("Silently ignoring message from LSP: {:?}", msg);
            }
        }
    }
    tracing::debug!("LSP->Preview channel closed, quitting reader thread");
    Ok(())
}

fn start_lsp_thread(
    from_preview: crossbeam_channel::Receiver<PreviewToLspMessage>,
    to_preview: EmbeddedLspToPreview,
    notifier: ServerNotifier,
    cli: Cli,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set.block_on(&rt, lsp_main(from_preview, to_preview, notifier, cli))
        {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    });
}

fn bridge_crossbeam_to_tokio(
    from_preview: crossbeam_channel::Receiver<PreviewToLspMessage>,
) -> tokio::sync::mpsc::UnboundedReceiver<PreviewToLspMessage> {
    let (from_preview_tx, from_preview_rx) =
        tokio::sync::mpsc::unbounded_channel::<PreviewToLspMessage>();
    std::thread::spawn(move || {
        while let Ok(msg) = from_preview.recv() {
            if from_preview_tx.send(msg).is_err() {
                break;
            }
        }
        tracing::debug!("Preview->LSP crossbeam adapter thread exited");
    });
    from_preview_rx
}

async fn lsp_main(
    from_preview: crossbeam_channel::Receiver<PreviewToLspMessage>,
    to_preview: EmbeddedLspToPreview,
    notifier: ServerNotifier,
    cli: Cli,
) -> Result<()> {
    use crate::common::document_cache::CompilerConfiguration;

    let mut from_preview_rx = bridge_crossbeam_to_tokio(from_preview);
    let (file_watcher_tx, mut file_watcher_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut file_watcher = FileWatcher::start(
        move |event| {
            if file_watcher_tx.send(event).is_err() {
                tracing::debug!("Ignoring file watcher event after editor shutdown");
            }
        },
        move |err| tracing::warn!("File watcher error: {err}"),
    )?;

    // Wrap to_preview in Rc for sharing with the import callback and Context
    let to_preview: Rc<dyn LspToPreview> = Rc::new(to_preview);

    let open_import_callback = {
        let to_preview = Rc::clone(&to_preview);
        Rc::new(move |path: String| {
            let to_preview = Rc::clone(&to_preview);
            Box::pin(async move {
                tracing::trace!("Importing file: {}", path);
                let contents = std::fs::read_to_string(&path);
                if let Ok(url) = Url::from_file_path(&path) {
                    if let Ok(contents) = &contents {
                        to_preview.send(&LspToPreviewMessage::SetContents {
                            url: VersionedUrl::new(url, None),
                            contents: contents.clone(),
                        });
                    } else {
                        to_preview.send(&LspToPreviewMessage::ForgetFile { url });
                    }
                }
                Some(contents.map(|c| (None, c)))
            })
                as Pin<
                    Box<dyn Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>>,
                >
        }) as OpenImportCallback
    };
    let compiler_config = CompilerConfiguration {
        style: Some("fluent".into()),
        open_import_callback: Some(open_import_callback),
        format: common::ByteFormat::Utf8,
        ..Default::default()
    };

    let mut ctx = language::Context {
        document_cache: common::DocumentCache::new(compiler_config),
        preview_config: Default::default(),
        server_notifier: notifier,
        init_param: Default::default(),
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: Default::default(),
        to_preview,
        pending_recompile: Default::default(),
    };

    // Load the initial document through the compiler. This triggers the import
    // callback for all transitive dependencies, sending their contents to the preview.
    let full_path = std::fs::canonicalize(&cli.file)
        .map_err(|err| format!("Failed to determine full path for {}: {err}", cli.file))?;
    let root_path = full_path.clone();
    let url = Url::from_file_path(full_path)
        .map_err(|_| format!("Failed to convert {} to URL!", cli.file))?;
    file_watcher.update_watched_paths(std::iter::once(root_path.clone()))?;
    language::show_preview(
        i_slint_preview_protocol::PreviewComponent { url: url.clone(), component: cli.component },
        &mut ctx,
    );

    // Make sure the document is loaded before we start processing messages from the preview, so we
    // have the correct state already loaded.
    language::reload_document(&mut ctx, url)
        .await
        .map_err(|err| format!("Failed to load file: {}: {err}", cli.file))?;
    sync_file_watcher(&mut file_watcher, &ctx, &root_path)?;

    const RECOMPILE_IDLE_TIMEOUT: Duration = Duration::from_millis(50);
    loop {
        let recompile_idle_timeout =
            if ctx.pending_recompile.is_empty() { Duration::MAX } else { RECOMPILE_IDLE_TIMEOUT };
        tokio::select! {
            watcher_event = file_watcher_rx.recv() => {
                match watcher_event {
                    Some(event) => trigger_editor_file_watcher(&mut ctx, event).await?,
                    None => break Err("File watcher channel closed".into()),
                }
            }
            msg = from_preview_rx.recv() => {
                match msg {
                    Some(msg) => handle_preview_message(msg, &ctx),
                    None => {
                        tracing::debug!("Preview->LSP channel closed, exiting");
                        break Ok(());
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
                sync_file_watcher(&mut file_watcher, &ctx, &root_path)?;
            }
        }
    }
}

async fn trigger_editor_file_watcher(
    ctx: &mut language::Context,
    WatchEvent { path, kind }: WatchEvent,
) -> Result<()> {
    let Ok(url) = Url::from_file_path(&path) else {
        tracing::debug!("Ignoring file watcher event for non-file path: {}", path.display());
        return Ok(());
    };

    language::trigger_file_watcher(ctx, url, lsp_file_change_type(kind)).await
}

fn lsp_file_change_type(kind: FileChangeKind) -> FileChangeType {
    match kind {
        FileChangeKind::Created => FileChangeType::CREATED,
        FileChangeKind::Changed => FileChangeType::CHANGED,
        FileChangeKind::Deleted => FileChangeType::DELETED,
    }
}

fn sync_file_watcher(
    watcher: &mut FileWatcher,
    ctx: &language::Context,
    root_path: &std::path::Path,
) -> Result<()> {
    watcher.update_watched_paths(
        std::iter::once(root_path.to_path_buf()).chain(
            ctx.document_cache
                .all_urls_to_watch()
                .into_iter()
                // filter out builtins
                .filter(|url| url.scheme() == "file")
                .filter_map(|url| common::uri_to_file(&url)),
        ),
    )?;
    Ok(())
}

fn handle_preview_message(msg: PreviewToLspMessage, ctx: &language::Context) {
    use PreviewToLspMessage::*;
    match &msg {
        RequestState { .. } => {
            tracing::debug!("Preview requested state, re-sending all documents");
            language::send_state_to_preview(ctx);
        }
        SendShowMessage { message } => {
            match message.typ {
                MessageType::ERROR => tracing::error!("Preview: {}", message.message),
                MessageType::WARNING => tracing::warn!("Preview: {}", message.message),
                MessageType::LOG => tracing::debug!("Preview: {}", message.message),
                _ => tracing::info!("Preview: {}", message.message),
            };
        }
        Diagnostics { .. }
        | ShowDocument { .. }
        | PreviewTypeChanged { .. }
        | TelemetryEvent(..) => {
            tracing::debug!("Ignoring message from preview: {msg:?}");
        }
        SendWorkspaceEdit { .. } => {
            tracing::warn!("Workspace edits not yet implemented in visual editor");
        }
    }
}
