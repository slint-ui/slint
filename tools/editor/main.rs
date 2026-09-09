// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Keep Windows from opening a console behind the editor. Debug builds keep
// theirs, so that printing something still reaches somewhere visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    cell::Cell,
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
    time::Duration,
};

use i_slint_editor_preview as editor_preview;
use i_slint_editor_preview::{LspToPreviews, Result, document_cache::OpenImportCallback};
use i_slint_live_preview::file_watcher::{FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewTarget, PreviewToLspMessage, SourceFileVersion,
    VersionedUrl, WorkspaceEditOutcome,
};
use lsp_types::{MessageType, Url};
use slint::ComponentHandle;

#[cfg(target_os = "linux")]
mod flatpak;
mod preview;
#[cfg(target_os = "macos")]
mod sparkle;
mod startup;
#[cfg(target_os = "windows")]
mod windows;

use preview::settings::{Project, TOOL_NAME};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    use clap::Parser;

    let cli = Cli::parse();

    // Set up the Slint backend (installing the macOS unified-title-bar hook)
    select_backend()?;

    let editor_ui = preview::ui::create_ui()?;

    // The updater needs to stay in scope for as long as the window is up.
    #[cfg(target_os = "macos")]
    let _updater = setup_macos_chrome(&editor_ui);
    #[cfg(target_os = "linux")]
    let _updater = flatpak::connect(&editor_ui);
    #[cfg(target_os = "windows")]
    let _updater = windows::connect(&editor_ui);

    #[cfg(feature = "system-testing")]
    preview::test_sync::initialize();
    let settings = startup::load_settings();
    if let Some(file) = cli.file {
        let project = Project::from_file(file, cli.component)?;
        start_editor_session(&editor_ui, project, settings);
    } else {
        let session_started = Rc::new(Cell::new(false));
        let editor_ui_weak = editor_ui.as_weak();
        let session_settings = settings.clone();
        startup::setup(
            &editor_ui,
            &settings,
            Rc::new(move |project| {
                if session_started.get() {
                    return false;
                }
                let Some(editor_ui) = editor_ui_weak.upgrade() else {
                    return false;
                };
                session_started.set(true);
                start_editor_session(&editor_ui, project, session_settings.clone());
                true
            }),
        );
    }

    editor_ui.run()?;
    Ok(())
}

/// Set up the editor's macOS chrome: the unified title bar and the Sparkle
/// auto-updater driving the update section of the editor UI.
#[cfg(target_os = "macos")]
fn setup_macos_chrome(editor_ui: &preview::ui::EditorUi) -> Option<Rc<crate::sparkle::Sparkle>> {
    use slint::ComponentHandle;

    preview::macos_titlebar::setup(editor_ui.as_weak());
    crate::sparkle::connect(editor_ui)
}

/// Hands messages for the preview straight to the UI thread: the editor runs
/// the preview in-process, so there is nothing to serialize.
struct EditorLspToPreview;

impl editor_preview::LspToPreview for EditorLspToPreview {
    fn send(&self, message: &LspToPreviewMessage) {
        let message = message.clone();
        #[cfg(feature = "system-testing")]
        let work = preview::test_sync::Work::capture("UI message");
        #[cfg(feature = "system-testing")]
        let input = preview::test_sync::source_delivery(&message);
        if let Err(err) = slint::invoke_from_event_loop(move || {
            #[cfg(feature = "system-testing")]
            work.run(|| {
                preview::test_sync::install_source(input);
                preview::lsp_to_preview(message);
            });
            #[cfg(not(feature = "system-testing"))]
            preview::lsp_to_preview(message);
        }) {
            tracing::error!("Failed to queue message onto the event loop: {err}");
        }
    }

    // The variant `EmbeddedLspToPreview` used to report: despite its name it is not
    // wasm-specific, and here it only keys the single entry in `LspToPreviews`.
    fn preview_target(&self) -> PreviewTarget {
        PreviewTarget::EmbeddedWasm
    }
}

struct PreviewMessage {
    message: PreviewToLspMessage,
    #[cfg(feature = "system-testing")]
    work: preview::test_sync::Work,
}

struct EmbeddedPreviewToLsp {
    sender: crossbeam_channel::Sender<PreviewMessage>,
}

impl editor_preview::PreviewToLsp for EmbeddedPreviewToLsp {
    fn send(&self, message: &PreviewToLspMessage) -> editor_preview::Result<()> {
        self.sender.send(PreviewMessage {
            message: message.clone(),
            #[cfg(feature = "system-testing")]
            work: preview::test_sync::Work::capture("LSP message"),
        })?;
        Ok(())
    }
}

#[derive(clap::Parser)]
struct Cli {
    file: Option<String>,
    component: Option<String>,
}

fn select_backend() -> std::result::Result<(), slint::PlatformError> {
    let headless_requested = std::env::var("SLINT_BACKEND").is_ok_and(|backend| {
        i_slint_backend_selector::parse_backend_env_var(&backend.to_ascii_lowercase()).0
            == "headless"
    });
    if headless_requested {
        return i_slint_backend_selector::with_platform(|_| Ok(()));
    }

    // See bug #10274 on macOS.
    let selector = slint::BackendSelector::new();
    // On macOS, request a unified title bar: the editor content extends underneath
    // a transparent title bar (see `preview::macos_titlebar`).
    #[cfg(target_os = "macos")]
    let selector =
        selector.with_winit_window_attributes_hook(preview::macos_titlebar::apply_unified_titlebar);
    selector.select()
}

fn start_editor_session(
    editor_ui: &preview::ui::EditorUi,
    project: Project,
    settings: preview::settings::VisualEditorSettings,
) {
    let (to_lsp, from_preview) = crossbeam_channel::unbounded();
    let to_lsp = Rc::new(EmbeddedPreviewToLsp { sender: to_lsp })
        as Rc<dyn editor_preview::PreviewToLsp + 'static>;
    preview::ui::initialize_editor(editor_ui, &to_lsp, "");
    preview::initialize(editor_ui, to_lsp, settings);
    start_lsp_thread(from_preview, project);
}

fn start_lsp_thread(from_preview: crossbeam_channel::Receiver<PreviewMessage>, project: Project) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set.block_on(&rt, lsp_main(from_preview, project)) {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    });
}

fn bridge_crossbeam_to_tokio(
    from_preview: crossbeam_channel::Receiver<PreviewMessage>,
) -> tokio::sync::mpsc::UnboundedReceiver<PreviewMessage> {
    let (from_preview_tx, from_preview_rx) =
        tokio::sync::mpsc::unbounded_channel::<PreviewMessage>();
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
    from_preview: crossbeam_channel::Receiver<PreviewMessage>,
    project: Project,
) -> Result<()> {
    use editor_preview::document_cache::CompilerConfiguration;

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

    // Wrap to_preview in Rc for sharing with the import callback and the session
    let to_preview = LspToPreviews::with_one(EditorLspToPreview);
    #[cfg(feature = "system-testing")]
    to_preview.set_source_observer(Rc::new(preview::test_sync::observed_read));

    let open_import_callback = {
        let to_preview = Rc::clone(&to_preview);
        Rc::new(move |path: String| {
            let to_preview = Rc::clone(&to_preview);
            Box::pin(async move {
                tracing::trace!("Importing file: {}", path);
                let contents = std::fs::read(&path);
                if let Ok(url) = Url::from_file_path(&path) {
                    #[cfg(feature = "system-testing")]
                    preview::test_sync::observed_read(
                        &url,
                        contents.as_ref().ok().and_then(|c| std::str::from_utf8(c).ok()),
                    );
                    if let Ok(contents) = &contents {
                        to_preview.send(&LspToPreviewMessage::SetContents {
                            url: VersionedUrl::new(url, None),
                            contents: contents.clone(),
                        });
                    } else {
                        to_preview.send(&LspToPreviewMessage::ForgetFile { url });
                    }
                }
                Some(
                    contents
                        .and_then(|c| String::from_utf8(c).map_err(std::io::Error::other))
                        .map(|c| (None, c)),
                )
            })
                as Pin<
                    Box<dyn Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>>,
                >
        }) as OpenImportCallback
    };
    let compiler_config = CompilerConfiguration {
        style: Some("fluent".into()),
        open_import_callback: Some(open_import_callback),
        format: editor_preview::ByteFormat::Utf8,
        ..Default::default()
    };

    let mut session = editor_preview::EditorSession {
        document_cache: editor_preview::DocumentCache::new(compiler_config),
        preview_config: Default::default(),
        to_show: Default::default(),
        open_urls: Default::default(),
        to_preview,
        pending_recompile: Default::default(),
    };

    let mut watch_paths_revision = None;
    let project_root = project.root;
    open_initial_preview(&mut session, &mut file_watcher, &project_root, project.preview).await?;
    sync_file_watcher_if_needed(
        &mut file_watcher,
        &session,
        &project_root,
        &mut watch_paths_revision,
    )?;

    const RECOMPILE_DELAY: Duration = Duration::from_millis(50);
    let mut recompile_deadline = None;
    #[cfg(feature = "system-testing")]
    let mut pending_work = Vec::new();
    #[cfg(feature = "system-testing")]
    let mut held_events: Vec<(u64, WatchEvent)> = Vec::new();
    loop {
        if session.pending_recompile.is_empty() {
            recompile_deadline = None;
        } else {
            // Preview messages must not postpone a pending source update.
            recompile_deadline.get_or_insert_with(|| tokio::time::Instant::now() + RECOMPILE_DELAY);
        }
        tokio::select! {
            _ = source_gate_changed() => {},
            watcher_event = file_watcher_rx.recv() => {
                match watcher_event {
                    Some(event) => {
                        #[cfg(feature = "system-testing")]
                        {
                            if let Ok(url) = Url::from_file_path(&event.path) {
                                if let Some(gate) = preview::test_sync::hold_source(&url) {
                                    held_events.push((gate, event));
                                    continue;
                                }
                                let work = preview::test_sync::observed_write(&url);
                                work.during(trigger_editor_file_watcher(&mut session, event)).await?;
                                pending_work.push(work);
                            }
                        }
                        #[cfg(not(feature = "system-testing"))]
                        trigger_editor_file_watcher(&mut session, event).await?;
                    },
                    None => break Err("File watcher channel closed".into()),
                }
            }
            msg = from_preview_rx.recv() => {
                match msg {
                    Some(msg) => {
                        #[cfg(feature = "system-testing")]
                        {
                            msg.work.during(handle_preview_message(msg.message, &mut session, &project_root)).await;
                            pending_work.push(msg.work);
                        }
                        #[cfg(not(feature = "system-testing"))]
                        handle_preview_message(msg.message, &mut session, &project_root).await;
                    }
                    None => {
                        tracing::debug!("Preview->LSP channel closed, exiting");
                        break Ok(());
                    }
                }
            }
            _ = async {
                match recompile_deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending().await,
                }
            } => {
                recompile_deadline = None;
                tracing::debug!("LSP recompiling");
                let pending_recompile = std::mem::take(&mut session.pending_recompile);

                #[cfg(feature = "system-testing")]
                let work = preview::test_sync::Work::combine(std::mem::take(&mut pending_work));
                for url in pending_recompile {
                    #[cfg(feature = "system-testing")]
                    let result = work.during(session.reload_document(url)).await;
                    #[cfg(not(feature = "system-testing"))]
                    let result = session.reload_document(url).await;
                    if let Err(err) = result {
                        tracing::error!("Failed document reload: {err}");
                    }
                }
            }
        }

        #[cfg(feature = "system-testing")]
        {
            let mut still_held = Vec::new();
            for (gate, event) in held_events.drain(..) {
                if preview::test_sync::gate_released(gate) {
                    let work = Url::from_file_path(&event.path)
                        .ok()
                        .map(|url| preview::test_sync::observed_write(&url))
                        .unwrap_or_default();
                    work.during(trigger_editor_file_watcher(&mut session, event)).await?;
                    pending_work.push(work);
                } else {
                    still_held.push((gate, event));
                }
            }
            held_events = still_held;
            if session.pending_recompile.is_empty() {
                pending_work.clear();
            }
        }

        sync_file_watcher_if_needed(
            &mut file_watcher,
            &session,
            &project_root,
            &mut watch_paths_revision,
        )?;
    }
}

async fn source_gate_changed() {
    #[cfg(feature = "system-testing")]
    preview::test_sync::gate_changed().await;
    #[cfg(not(feature = "system-testing"))]
    std::future::pending::<()>().await;
}

async fn trigger_editor_file_watcher(
    session: &mut editor_preview::EditorSession,
    WatchEvent { path, kind }: WatchEvent,
) -> Result<()> {
    let Ok(url) = Url::from_file_path(&path) else {
        tracing::debug!("Ignoring file watcher event for non-file path: {}", path.display());
        return Ok(());
    };

    let _diagnostics = session.trigger_file_watcher(url, kind).await?;
    Ok(())
}

fn sync_file_watcher_if_needed(
    watcher: &mut FileWatcher,
    session: &editor_preview::EditorSession,
    root_path: &Path,
    watch_paths_revision: &mut Option<u64>,
) -> Result<()> {
    let current_revision = session.document_cache.revision();
    if watch_paths_revision.is_some_and(|rev| rev == current_revision) {
        return Ok(());
    }

    watcher.update_watched_paths(
        std::iter::once(root_path.to_path_buf()).chain(
            session
                .document_cache
                .all_urls_to_watch()
                .into_iter()
                // filter out builtins
                .filter(|url| url.scheme() == "file")
                .filter_map(|url| editor_preview::uri_to_file(&url)),
        ),
    )?;
    *watch_paths_revision = Some(current_revision);
    Ok(())
}

async fn handle_preview_message(
    msg: PreviewToLspMessage,
    session: &mut editor_preview::EditorSession,
    project_root: &Path,
) {
    use PreviewToLspMessage::*;
    match &msg {
        RequestState { files, settings } => {
            tracing::debug!("Preview requested state");
            if files.is_empty() {
                if let Ok(root) = Url::from_directory_path(project_root) {
                    session.to_preview.send(&LspToPreviewMessage::OpenProject { root });
                }
                session.send_state_to_preview();
            } else {
                session.send_files_to_preview(files, |_| true);
            }
            for name in settings {
                if let Some(contents) =
                    i_slint_editor_preview::settings_store::load(TOOL_NAME, name)
                {
                    session.to_preview.send(&LspToPreviewMessage::SetUserSettings {
                        name: name.clone(),
                        contents,
                    });
                }
            }
        }
        RequestPreview { component } => {
            let Some((component, path)) = canonical_preview_component(component) else {
                tracing::warn!("Ignoring preview request with an invalid path: {}", component.url);
                return;
            };
            if let Err(err) = open_preview(session, component).await {
                tracing::error!("Failed to open preview for {}: {err}", path.display());
            }
        }
        UpdateUserSettings { name, contents } => {
            if let Err(error) =
                i_slint_editor_preview::settings_store::save(TOOL_NAME, name, contents)
            {
                #[cfg(feature = "system-testing")]
                preview::test_sync::effect("failed");
                tracing::warn!("Failed to save preview user settings: {error}");
            } else {
                #[cfg(feature = "system-testing")]
                preview::test_sync::effect("completed");
            }
        }
        SendShowMessage { message } => {
            match message.typ {
                MessageType::ERROR => tracing::error!("Preview: {}", message.message),
                MessageType::WARNING => tracing::warn!("Preview: {}", message.message),
                MessageType::LOG => tracing::debug!("Preview: {}", message.message),
                _ => tracing::info!("Preview: {}", message.message),
            };
        }
        DebugMessage { location, message } => {
            eprintln!("{}", editor_preview::preview_log_message_to_string(location, message));
        }

        Diagnostics { .. }
        | ShowDocument { .. }
        | PreviewTypeChanged { .. }
        | TelemetryEvent(..)
        | ConnectRemote { .. }
        | DisconnectRemote
        | SubmitPairingCode { .. }
        | CancelPairing
        | AcceptUnpairedConnection
        | Pong
        | PairingReady
        | PairingRequired { .. }
        | PairingTokenChallenge { .. }
        | PairingConfirm { .. }
        | PairingAccepted
        | PairingRejected { .. } => {
            tracing::debug!("Ignoring message from preview: {msg:?}");
        }
        SendWorkspaceEdit { label, edit } => {
            handle_workspace_edit(
                &session.document_cache,
                session.to_preview.as_ref(),
                label.as_deref(),
                edit,
            );
        }
    }
}

async fn open_initial_preview(
    session: &mut editor_preview::EditorSession,
    watcher: &mut FileWatcher,
    project_root: &Path,
    component: PreviewComponent,
) -> Result<()> {
    watcher.update_watched_paths(
        std::iter::once(project_root.to_path_buf())
            .chain(editor_preview::uri_to_file(&component.url)),
    )?;
    open_project(session, project_root)?;
    open_preview(session, component).await
}

async fn open_preview(
    session: &mut editor_preview::EditorSession,
    component: PreviewComponent,
) -> Result<()> {
    let _diagnostics = session.reload_document(component.url.clone()).await?;
    session.to_show = Some(component.clone());
    session.to_preview.send(&LspToPreviewMessage::ShowPreview(component));
    Ok(())
}

fn open_project(session: &editor_preview::EditorSession, root: &Path) -> Result<()> {
    let root = std::fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()).into());
    }
    let url = Url::from_directory_path(&root)
        .map_err(|_| format!("Failed to convert {} to URL", root.display()))?;
    session.to_preview.send(&LspToPreviewMessage::OpenProject { root: url });
    Ok(())
}

fn canonical_preview_component(
    component: &PreviewComponent,
) -> Option<(PreviewComponent, PathBuf)> {
    let path = editor_preview::uri_to_file(&component.url)?;
    let path = std::fs::canonicalize(path).ok()?;
    let url = Url::from_file_path(&path).ok()?;
    Some((PreviewComponent { url, component: component.component.clone() }, path))
}

fn handle_workspace_edit(
    document_cache: &editor_preview::DocumentCache,
    to_preview: &editor_preview::LspToPreviews,
    label: Option<&str>,
    edit: &lsp_types::WorkspaceEdit,
) {
    match editor_preview::editing::text_edit::apply_workspace_edit(document_cache, edit) {
        Ok(edited_texts) if edited_texts.is_empty() => {
            #[cfg(feature = "system-testing")]
            crate::preview::test_sync::effect("rejected");
            to_preview.send(&LspToPreviewMessage::WorkspaceEditResult {
                outcome: WorkspaceEditOutcome::Rejected,
            });
            tracing::warn!(
                "Workspace edit '{}' did not address any loaded document",
                label.unwrap_or("(unnamed)")
            );
        }
        Ok(edited_texts) => {
            let files = u32::try_from(edited_texts.len()).unwrap_or(u32::MAX);
            #[cfg(feature = "system-testing")]
            crate::preview::test_sync::accepted_edit();
            let mut written = 0u32;
            for editor_preview::editing::text_edit::EditedText { url, contents } in edited_texts {
                match editor_preview::uri_to_file(&url) {
                    Some(path) => {
                        if let Err(err) = std::fs::write(&path, &contents) {
                            #[cfg(feature = "system-testing")]
                            crate::preview::test_sync::effect("failed");
                            tracing::error!(
                                "Failed to apply workspace edit '{}' to {}: {err}",
                                label.unwrap_or("(unnamed)"),
                                path.display()
                            );
                        } else {
                            written += 1;
                            #[cfg(feature = "system-testing")]
                            {
                                crate::preview::test_sync::written(&url);
                                crate::preview::test_sync::expect_watch(&url);
                            }
                        }
                    }
                    None => {
                        tracing::warn!("Cannot apply workspace edit to non-file URL: {url}");
                    }
                }
            }
            let outcome = if written == files {
                WorkspaceEditOutcome::Applied { files }
            } else {
                #[cfg(feature = "system-testing")]
                crate::preview::test_sync::effect("failed");
                WorkspaceEditOutcome::Failed { files, written }
            };
            to_preview.send(&LspToPreviewMessage::WorkspaceEditResult { outcome });
        }
        Err(err) => {
            #[cfg(feature = "system-testing")]
            crate::preview::test_sync::effect("rejected");
            to_preview.send(&LspToPreviewMessage::WorkspaceEditResult {
                outcome: WorkspaceEditOutcome::Rejected,
            });
            tracing::error!(
                "Failed to compute workspace edit '{}': {err}",
                label.unwrap_or("(unnamed)")
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_source_repair_is_watched_before_preview_is_published() {
        const REPAIRED: &str = "export component Initial inherits Window { width: 320px; }";
        struct RepairOnPreview(PathBuf);
        impl editor_preview::LspToPreview for RepairOnPreview {
            fn send(&self, message: &LspToPreviewMessage) {
                if matches!(message, LspToPreviewMessage::ShowPreview(_)) {
                    std::fs::write(&self.0, REPAIRED).unwrap();
                }
            }
            fn preview_target(&self) -> PreviewTarget {
                PreviewTarget::Dummy
            }
        }

        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = root.join("Initial.slint");
        std::fs::write(&source, "export component Initial inherits Window { broken }").unwrap();
        let url = Url::from_file_path(&source).unwrap();
        let mut session = editor_preview::EditorSession {
            document_cache: editor_preview::DocumentCache::new(Default::default()),
            preview_config: Default::default(),
            to_show: None,
            open_urls: Default::default(),
            to_preview: LspToPreviews::with_one(RepairOnPreview(source.clone())),
            pending_recompile: Default::default(),
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (error_tx, error_rx) = std::sync::mpsc::channel();
        let mut watcher = FileWatcher::start(
            move |event| {
                let _ = tx.send(event);
            },
            move |error| {
                let _ = error_tx.send(error);
            },
        )
        .unwrap();
        spin_on::spin_on(open_initial_preview(
            &mut session,
            &mut watcher,
            &root,
            PreviewComponent { url: url.clone(), component: None },
        ))
        .unwrap();
        sync_file_watcher_if_needed(&mut watcher, &session, &root, &mut None).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut seen = Vec::new();
        loop {
            let event = rx
                .recv_timeout(deadline.saturating_duration_since(std::time::Instant::now()))
                .unwrap_or_else(|error| {
                    let errors = error_rx.try_iter().collect::<Vec<_>>();
                    panic!(
                        "repair made while publishing the initial preview must produce a watch event for {url}: {error}; received {seen:?}; watcher errors {errors:?}"
                    );
                });
            if Url::from_file_path(&event.path).ok().as_ref() == Some(&url) {
                spin_on::spin_on(trigger_editor_file_watcher(&mut session, event)).unwrap();
                break;
            }
            seen.push(event);
        }
        assert!(session.pending_recompile.remove(&url));
        spin_on::spin_on(session.reload_document(url.clone())).unwrap();
        let (_, node) =
            session.document_cache.all_url_documents().find(|(u, _)| u == &url).unwrap();
        assert_eq!(node.text().to_string(), REPAIRED);
    }
}
