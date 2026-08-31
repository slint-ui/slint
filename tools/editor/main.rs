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
    VersionedUrl,
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
        if let Err(err) = slint::invoke_from_event_loop(move || {
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

struct EmbeddedPreviewToLsp {
    sender: crossbeam_channel::Sender<PreviewToLspMessage>,
}

impl editor_preview::PreviewToLsp for EmbeddedPreviewToLsp {
    fn send(&self, message: &PreviewToLspMessage) -> editor_preview::Result<()> {
        self.sender.send(message.clone())?;
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
    start_lsp_thread(vec![from_preview], project);
}

fn start_lsp_thread(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    project: Project,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set.block_on(&rt, lsp_main(from_previews, project)) {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    });
}

fn bridge_crossbeam_to_tokio(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
) -> Vec<tokio::sync::mpsc::UnboundedReceiver<PreviewToLspMessage>> {
    from_previews
        .into_iter()
        .map(|from_preview| {
            let (from_preview_tx, from_preview_rx) =
                tokio::sync::mpsc::unbounded_channel::<PreviewToLspMessage>();
            std::thread::spawn(move || {
                while let Ok(message) = from_preview.recv() {
                    if from_preview_tx.send(message).is_err() {
                        break;
                    }
                }
                tracing::debug!("Preview->LSP crossbeam adapter thread exited");
            });
            from_preview_rx
        })
        .collect()
}

async fn receive_preview_message(
    from_previews: &mut [tokio::sync::mpsc::UnboundedReceiver<PreviewToLspMessage>],
) -> (usize, Option<PreviewToLspMessage>) {
    let receives = from_previews.iter_mut().map(|from_preview| Box::pin(from_preview.recv()));
    let (message, preview_index, _) = futures_util::future::select_all(receives).await;
    (preview_index, message)
}

async fn lsp_main(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    project: Project,
) -> Result<()> {
    use editor_preview::document_cache::CompilerConfiguration;

    let mut from_previews = bridge_crossbeam_to_tokio(from_previews);
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
    let to_previews = vec![LspToPreviews::with_one(EditorLspToPreview)];

    let open_import_callback = {
        let to_previews = to_previews.clone();
        Rc::new(move |path: String| {
            let to_previews = to_previews.clone();
            Box::pin(async move {
                tracing::trace!("Importing file: {}", path);
                let contents = std::fs::read(&path);
                if let Ok(url) = Url::from_file_path(&path) {
                    for to_preview in &to_previews {
                        if let Ok(contents) = &contents {
                            to_preview.send(&LspToPreviewMessage::SetContents {
                                url: VersionedUrl::new(url.clone(), None),
                                contents: contents.clone(),
                            });
                        } else {
                            to_preview.send(&LspToPreviewMessage::ForgetFile { url: url.clone() });
                        }
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
        open_urls: Default::default(),
        previews: to_previews
            .into_iter()
            .map(|to_preview| editor_preview::PreviewConnection {
                to_preview,
                to_show: Default::default(),
            })
            .collect(),
        pending_recompile: Default::default(),
    };

    assert_eq!(session.previews.len(), from_previews.len());

    let mut watch_paths_revision = None;
    let project_root = project.root;
    open_project(&session, 0, &project_root)?;
    open_preview(&mut session, 0, project.preview).await?;
    sync_file_watcher_if_needed(
        &mut file_watcher,
        &session,
        &project_root,
        &mut watch_paths_revision,
    )?;

    const RECOMPILE_IDLE_TIMEOUT: Duration = Duration::from_millis(50);
    loop {
        let recompile_idle_timeout = if session.pending_recompile.is_empty() {
            Duration::MAX
        } else {
            RECOMPILE_IDLE_TIMEOUT
        };
        tokio::select! {
            watcher_event = file_watcher_rx.recv() => {
                match watcher_event {
                    Some(event) => trigger_editor_file_watcher(&mut session, event).await?,
                    None => break Err("File watcher channel closed".into()),
                }
            }
            preview_message = receive_preview_message(&mut from_previews) => {
                let (preview_index, message) = preview_message;
                match message {
                    Some(message) => {
                        handle_preview_message(
                            message,
                            preview_index,
                            &mut session,
                            &project_root,
                        ).await;
                    }
                    None => {
                        tracing::debug!("Preview->LSP channel closed, exiting");
                        break Ok(());
                    }
                }
            }
            _ = tokio::time::sleep(recompile_idle_timeout) => {
                tracing::debug!("LSP recompiling");
                let pending_recompile = std::mem::take(&mut session.pending_recompile);

                for url in pending_recompile {
                    if let Err(err) = session.reload_document(url).await {
                        tracing::error!("Failed document reload: {err}");
                    }
                }
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
    message: PreviewToLspMessage,
    preview_index: usize,
    session: &mut editor_preview::EditorSession,
    project_root: &Path,
) {
    use PreviewToLspMessage::*;
    if session.preview(preview_index).is_none() {
        return;
    }
    match &message {
        RequestState { files, settings } => {
            tracing::debug!("Preview requested state");
            if files.is_empty() {
                if let Ok(root) = Url::from_directory_path(project_root) {
                    session
                        .send_to_preview(preview_index, &LspToPreviewMessage::OpenProject { root });
                }
                session.send_state_to_preview(preview_index);
            } else {
                session.send_files_to_preview(preview_index, files, |_| true);
            }
            for name in settings {
                if let Some(contents) =
                    i_slint_editor_preview::settings_store::load(TOOL_NAME, name)
                {
                    session.send_to_preview(
                        preview_index,
                        &LspToPreviewMessage::SetUserSettings { name: name.clone(), contents },
                    );
                }
            }
        }
        RequestPreview { component } => {
            let Some((component, path)) = canonical_preview_component(component) else {
                tracing::warn!("Ignoring preview request with an invalid path: {}", component.url);
                return;
            };
            if let Err(err) = open_preview(session, preview_index, component).await {
                tracing::error!("Failed to open preview for {}: {err}", path.display());
            }
        }
        UpdateUserSettings { name, contents } => {
            if let Err(error) =
                i_slint_editor_preview::settings_store::save(TOOL_NAME, name, contents)
            {
                tracing::warn!("Failed to save preview user settings: {error}");
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
            tracing::debug!("Ignoring message from preview: {message:?}");
        }
        SendWorkspaceEdit { label, edit } => {
            handle_workspace_edit(&session.document_cache, label.as_deref(), edit);
        }
    }
}

async fn open_preview(
    session: &mut editor_preview::EditorSession,
    preview_index: usize,
    component: PreviewComponent,
) -> Result<()> {
    let _diagnostics = session.reload_document(component.url.clone()).await?;
    session.show_preview(preview_index, component);
    Ok(())
}

fn open_project(
    session: &editor_preview::EditorSession,
    preview_index: usize,
    root: &Path,
) -> Result<()> {
    let root = std::fs::canonicalize(root)?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", root.display()).into());
    }
    let url = Url::from_directory_path(&root)
        .map_err(|_| format!("Failed to convert {} to URL", root.display()))?;
    session.send_to_preview(preview_index, &LspToPreviewMessage::OpenProject { root: url });
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
    label: Option<&str>,
    edit: &lsp_types::WorkspaceEdit,
) {
    match editor_preview::editing::text_edit::apply_workspace_edit(document_cache, edit) {
        Ok(edited_texts) => {
            for editor_preview::editing::text_edit::EditedText { url, contents } in edited_texts {
                match editor_preview::uri_to_file(&url) {
                    Some(path) => {
                        if let Err(err) = std::fs::write(&path, &contents) {
                            tracing::error!(
                                "Failed to apply workspace edit '{}' to {}: {err}",
                                label.unwrap_or("(unnamed)"),
                                path.display()
                            );
                        }
                    }
                    None => {
                        tracing::warn!("Cannot apply workspace edit to non-file URL: {url}");
                    }
                }
            }
        }
        Err(err) => {
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

    fn session_with_recording_previews()
    -> (editor_preview::EditorSession, [editor_preview::test::CapturedPreviewMessages; 2]) {
        let captures = std::array::from_fn(|_| editor_preview::test::preview_capture());
        let previews = captures
            .iter()
            .map(|(to_preview, _)| editor_preview::PreviewConnection {
                to_preview: to_preview.clone(),
                to_show: None,
            })
            .collect();
        let messages = captures.map(|(_, messages)| messages);
        let session = editor_preview::EditorSession {
            document_cache: editor_preview::test::empty_document_cache(),
            preview_config: Default::default(),
            open_urls: Default::default(),
            previews,
            pending_recompile: Default::default(),
        };
        (session, messages)
    }

    #[test]
    fn separate_preview_channels_report_the_ready_receiver() {
        let (primary_sender, primary_receiver) = tokio::sync::mpsc::unbounded_channel();
        let (secondary_sender, secondary_receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut receivers = vec![primary_receiver, secondary_receiver];
        secondary_sender.send(PreviewToLspMessage::Pong).unwrap();

        let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
        let (preview_index, message) = runtime.block_on(receive_preview_message(&mut receivers));

        assert_eq!(preview_index, 1);
        assert!(matches!(message, Some(PreviewToLspMessage::Pong)));
        drop(primary_sender);
    }

    #[test]
    fn preview_requests_are_answered_through_the_originating_connection() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("secondary.slint");
        std::fs::write(&path, "export component Secondary {}").unwrap();
        let component = PreviewComponent {
            url: Url::from_file_path(&path).unwrap(),
            component: Some("Secondary".into()),
        };

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            1,
            &mut session,
            project.path(),
        ));

        assert!(messages[0].borrow().is_empty());
        assert!(
            messages[1]
                .borrow()
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::OpenProject { .. }))
        );

        for recorded_messages in &messages {
            recorded_messages.borrow_mut().clear();
        }

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestPreview { component: component.clone() },
            1,
            &mut session,
            project.path(),
        ));

        assert!(
            !messages[0]
                .borrow()
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::ShowPreview(_)))
        );
        assert!(messages[1].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(current) if current == &component)
        }));
        assert!(session.primary_preview().to_show.is_none());
        assert_eq!(session.preview(1).unwrap().to_show, Some(component));
    }
}
