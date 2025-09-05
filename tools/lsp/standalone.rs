// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common;
use crate::common::LspToPreview as _;
use crate::OutgoingRequestQueue;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::Poll;
use lsp_types::notification::Notification;
use slint::ComponentHandle as _;
use std::rc::Rc;
use std::sync::Arc;

pub fn open(
    args: super::Cli,
    path: std::path::PathBuf,
    component: Option<String>,
) -> crate::common::Result<()> {
    let (preview_to_lsp_sender, preview_to_lsp_receiver) =
        crossbeam_channel::unbounded::<crate::common::PreviewToLspMessage>();

    let to_lsp: Rc<dyn common::PreviewToLsp> =
        Rc::new(StandalonePreviewToLsp { sender: preview_to_lsp_sender });

    let experimental = std::env::var_os("SLINT_ENABLE_EXPERIMENTAL_FEATURES").is_some();
    let ui = crate::preview::ui::create_ui(&to_lsp, &"", experimental)?;

    to_lsp.send(&common::PreviewToLspMessage::RequestState { unused: true }).unwrap();

    let ui_clone = crate::preview::PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();
        *preview_state.to_lsp.borrow_mut() = Some(to_lsp);
        preview_state.ui = Some(ui.clone_strong());
        ui
    });

    let lsp_thread = std::thread::Builder::new()
        .name("LanguageServer".into())
        .spawn(move || {
            /// Make sure we quit the event loop even if we panic
            struct QuitEventLoop;
            impl Drop for QuitEventLoop {
                fn drop(&mut self) {
                    let _ = slint::quit_event_loop();
                }
            }
            let quit_ui_loop = QuitEventLoop;
            if let Err(e) = fake_lsp(args, path, component, preview_to_lsp_receiver) {
                eprintln!("{e}");
                std::process::exit(1);
            }
            drop(quit_ui_loop);
        })
        .unwrap();

    ui_clone.run()?;

    lsp_thread.join().unwrap();
    Ok(())
}

fn fake_lsp(
    args: super::Cli,
    path: std::path::PathBuf,
    component: Option<String>,
    preview_to_lsp_receiver: crossbeam_channel::Receiver<crate::common::PreviewToLspMessage>,
) -> crate::common::Result<()> {
    let request_queue = OutgoingRequestQueue::default();
    let (preview_to_client_sender, preview_to_client_reciever) =
        crossbeam_channel::unbounded::<lsp_server::Message>();
    let server_notifier =
        crate::ServerNotifier { sender: preview_to_client_sender, queue: request_queue.clone() };

    let compiler_config = args.into_compiler_config(|m| LspToStandalonePreview.send(&m).unwrap());

    let init_param = lsp_types::InitializeParams {
        capabilities: lsp_types::ClientCapabilities {
            workspace: Some(lsp_types::WorkspaceClientCapabilities {
                did_change_watched_files: Some(
                    lsp_types::DidChangeWatchedFilesClientCapabilities {
                        dynamic_registration: Some(true),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    let ctx = Rc::new(crate::Context {
        document_cache: RefCell::new(crate::common::DocumentCache::new(compiler_config)),
        preview_config: RefCell::new(Default::default()),
        server_notifier,
        init_param,
        #[cfg(any(feature = "preview-external", feature = "preview-engine"))]
        to_show: Default::default(),
        open_urls: Default::default(),
        to_preview: Rc::new(LspToStandalonePreview),
    });

    let ctx_ = ctx.clone();
    let contents = std::fs::read_to_string(&path)?;
    let absolute_path = std::fs::canonicalize(path)?;
    let future = Box::pin(async move {
        let url = lsp_types::Url::from_file_path(&absolute_path).unwrap();
        crate::open_document(
            &ctx_,
            contents,
            url.clone(),
            None,
            &mut ctx_.document_cache.borrow_mut(),
        )
        .await?;
        let mut args = vec![serde_json::to_value(url).unwrap()];
        if let Some(component) = component {
            args.push(serde_json::Value::String(component));
        }
        crate::language::show_preview_command(&args, &ctx_).map_err(|e| e.message)?;
        Ok(())
    });

    // We are waiting in this loop for two kind of futures:
    //  - The compiler future should always be ready immediately because we do not set a callback to load files
    //  - the future from `send_request` are blocked waiting for a response from the client (us) and we make sure
    //    that they are available immediately.
    struct DummyWaker;
    impl std::task::Wake for DummyWaker {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Arc::new(DummyWaker).into();
    let mut futures = Vec::<Pin<Box<dyn Future<Output = crate::common::Result<()>>>>>::new();
    futures.push(future);

    loop {
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
        crossbeam_channel::select! {
            recv(preview_to_client_reciever) -> msg => {
                match msg? {
                    lsp_server::Message::Notification(n) if n.method == lsp_types::notification::PublishDiagnostics::METHOD => (),
                    msg => eprintln!("Got client message from preview: {msg:?}")
                };
             },
             recv(preview_to_lsp_receiver) -> msg => {
                use crate::common::PreviewToLspMessage as M;
                match msg? {
                    M::TelemetryEvent{ .. } => (),
                    M::Diagnostics { uri, version: _, diagnostics } => {
                        // print to stdout, what else can we do?
                        for d in diagnostics {
                            let severity = match d.severity {
                                Some(lsp_types::DiagnosticSeverity::ERROR) => "Error: ",
                                Some(lsp_types::DiagnosticSeverity::WARNING) => "Warning: ",
                                Some(lsp_types::DiagnosticSeverity::INFORMATION) => "Info: ",
                                Some(lsp_types::DiagnosticSeverity::HINT) => "Hint: ",
                                _ => "",
                            };
                            println!("{uri:?}:{} {severity}{}", d.range.start.line, d.message);
                        }
                    },
                    M::ShowDocument { .. } => (),
                    M::PreviewTypeChanged { .. } => unreachable!("can't change type to external"),
                    M::RequestState { unused: _ } => {
                        crate::language::request_state(&ctx);
                    },
                    M::SendWorkspaceEdit { label:_, edit } => {
                        let edits = crate::common::text_edit::apply_workspace_edit(&ctx.document_cache.borrow(), &edit)?;
                        for e in edits {
                            std::fs::write(e.url.to_file_path().unwrap(), &e.contents)?;
                            // FIXME: fs watcher should take care of this automatically
                            let ctx = ctx.clone();
                            futures.push(Box::pin(async move {
                                crate::language::reload_document(&ctx, e.contents, e.url, None, &mut ctx.document_cache.borrow_mut()).await
                            }));
                        }

                    },
                    M::SendShowMessage { message } => {
                        eprint!("{}", message.message);
                    },
                };
             },
        };
    }
}

struct StandalonePreviewToLsp {
    sender: crossbeam_channel::Sender<common::PreviewToLspMessage>,
}

impl common::PreviewToLsp for StandalonePreviewToLsp {
    fn send(&self, message: &common::PreviewToLspMessage) -> common::Result<()> {
        self.sender
            .send(message.clone())
            .map_err(|e| format!("Failed to send message to LSP: {e}").into())
    }
}

struct LspToStandalonePreview;

impl common::LspToPreview for LspToStandalonePreview {
    fn preview_target(&self) -> common::PreviewTarget {
        common::PreviewTarget::Dummy
    }

    fn set_preview_target(&self, _: common::PreviewTarget) -> common::Result<()> {
        Err("Standalone Preview cannot not change the preview target".into())
    }

    fn send(&self, message: &crate::common::LspToPreviewMessage) -> crate::common::Result<()> {
        let message = message.clone();
        slint::invoke_from_event_loop(move || {
            crate::preview::connector::lsp_to_preview(message);
        })
        .map_err(|e| format!("Failed to send message to preview: {e}").into())
    }
}
