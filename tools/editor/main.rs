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

const PRIMARY_PREVIEW_INDEX: usize = 0;
const RUN_PREVIEW_INDEX: usize = 1;

enum EditorToSessionMessage {
    RunPreview,
}

#[derive(Default)]
struct RunPreviewState {
    requested: bool,
    highlight: Option<(Url, u32)>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    use clap::Parser;

    let cli = Cli::parse();

    if cli.run_preview_child {
        return editor_preview::child_process::run();
    }

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
    #[arg(long, hide = true)]
    run_preview_child: bool,
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
    let (to_editor_session, from_editor_preview) = crossbeam_channel::unbounded();
    let preview_global = editor_ui.global::<preview::ui::Preview>();
    preview_global.on_run(move || {
        to_editor_session.send(EditorToSessionMessage::RunPreview).ok();
    });
    let preview_global =
        <preview::ui::Preview as slint::Global<'_, preview::ui::EditorUi>>::as_weak(
            &preview_global,
        );
    let to_lsp = Rc::new(EmbeddedPreviewToLsp { sender: to_lsp })
        as Rc<dyn editor_preview::PreviewToLsp + 'static>;
    preview::ui::initialize_editor(editor_ui, &to_lsp, "");
    preview::initialize(editor_ui, to_lsp, settings);
    start_lsp_thread(vec![from_preview], from_editor_preview, project, preview_global);
}

fn start_lsp_thread(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    from_editor_preview: crossbeam_channel::Receiver<EditorToSessionMessage>,
    project: Project,
    preview_global: slint::Weak<preview::ui::Preview<'static>>,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set
            .block_on(&rt, lsp_main(from_previews, from_editor_preview, project, preview_global))
        {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    });
}

fn bridge_crossbeam_to_tokio(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
) -> Vec<tokio::sync::mpsc::UnboundedReceiver<PreviewToLspMessage>> {
    from_previews.into_iter().map(bridge_crossbeam_receiver).collect()
}

fn bridge_crossbeam_receiver<Message: Send + 'static>(
    receiver: crossbeam_channel::Receiver<Message>,
) -> tokio::sync::mpsc::UnboundedReceiver<Message> {
    let (sender, tokio_receiver) = tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(message) = receiver.recv() {
            if sender.send(message).is_err() {
                break;
            }
        }
    });
    tokio_receiver
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
    from_editor: crossbeam_channel::Receiver<EditorToSessionMessage>,
    project: Project,
    preview_global: slint::Weak<preview::ui::Preview<'static>>,
) -> Result<()> {
    use editor_preview::document_cache::CompilerConfiguration;

    let mut from_previews = bridge_crossbeam_to_tokio(from_previews);
    let mut from_editor = bridge_crossbeam_receiver(from_editor);
    let (from_run_preview_sender, from_run_preview) = tokio::sync::mpsc::unbounded_channel();
    from_previews.push(from_run_preview);
    let (file_watcher_tx, mut file_watcher_rx) = tokio::sync::mpsc::unbounded_channel();
    let mut file_watcher = FileWatcher::start(
        move |event| {
            if file_watcher_tx.send(event).is_err() {
                tracing::debug!("Ignoring file watcher event after editor shutdown");
            }
        },
        move |err| tracing::warn!("File watcher error: {err}"),
    )?;

    let to_previews = vec![
        LspToPreviews::with_one(EditorLspToPreview),
        LspToPreviews::with_one(editor_preview::child_process::ChildProcessLspToPreview::new(
            std::env::current_exe()?,
            vec![std::ffi::OsString::from("--run-preview-child")],
            from_run_preview_sender,
        )),
    ];

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
        preview_config: i_slint_live_preview::protocol::PreviewConfig {
            style: "fluent".into(),
            ..Default::default()
        },
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
    open_project(&session, PRIMARY_PREVIEW_INDEX, &project_root)?;
    open_preview(&mut session, PRIMARY_PREVIEW_INDEX, project.preview).await?;
    sync_file_watcher_if_needed(
        &mut file_watcher,
        &session,
        &project_root,
        &mut watch_paths_revision,
    )?;

    const RECOMPILE_IDLE_TIMEOUT: Duration = Duration::from_millis(50);
    let mut run_preview_state = RunPreviewState::default();
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
                            &mut run_preview_state,
                            &preview_global
                        ).await;
                    }
                    None => {
                        tracing::debug!("Preview->LSP channel closed, exiting");
                        break Ok(());
                    }
                }
            }
            editor_message = from_editor.recv() => {
                match editor_message {
                    Some(message) => handle_editor_message(
                        message,
                        &mut session,
                        &mut run_preview_state,
                    ),
                    None => break Ok(()),
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
    run_preview_state: &mut RunPreviewState,
    preview_global: &slint::Weak<preview::ui::Preview<'static>>,
) {
    use PreviewToLspMessage::*;
    if session.preview(preview_index).is_none() {
        return;
    }

    // any message we receive from the preview that is not "Exited" means the preview is alive.
    if preview_index == RUN_PREVIEW_INDEX {
        let is_running = !matches!(&message, PreviewToLspMessage::Exited);
        if let Err(error) = preview_global.upgrade_in_event_loop(move |preview_global| {
            preview_global.set_is_running(is_running);
        }) {
            tracing::error!("Failed to update Run preview state: {error}");
        }
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
                if preview_index == RUN_PREVIEW_INDEX {
                    send_run_preview_highlight(session, run_preview_state);
                }
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
        // If the editor preview requests to "showDocument" that should translate to a highlight in
        // the run preview.
        ShowDocument { file, selection, .. } if preview_index == PRIMARY_PREVIEW_INDEX => {
            let Some(document) = session
                .document_cache
                .get_document(file)
                .and_then(|document| document.node.as_ref())
            else {
                tracing::warn!("Cannot highlight a position in an unknown document: {file}");
                return;
            };
            let offset = editor_preview::util::lsp_position_to_text_size(
                &document.source_file,
                selection.start,
                session.document_cache.format,
            );
            run_preview_state.highlight = Some((file.clone(), offset.into()));
            send_run_preview_highlight(session, run_preview_state);
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
        | PairingRejected { .. }
        | Exited => {
            tracing::debug!("Ignoring message from preview: {message:?}");
        }
        SendWorkspaceEdit { label, edit } => {
            handle_workspace_edit(&session.document_cache, label.as_deref(), edit);
        }
    }
}

fn handle_editor_message(
    message: EditorToSessionMessage,
    session: &mut editor_preview::EditorSession,
    run_preview_state: &mut RunPreviewState,
) {
    match message {
        EditorToSessionMessage::RunPreview => {
            let Some(component) = session.primary_preview().to_show.clone() else {
                tracing::warn!("Cannot run a preview before a component is open");
                return;
            };
            run_preview_state.requested = true;
            session.show_preview(RUN_PREVIEW_INDEX, component);
            send_run_preview_highlight(session, run_preview_state);
        }
    }
}

fn send_run_preview_highlight(
    session: &editor_preview::EditorSession,
    run_preview_state: &RunPreviewState,
) {
    if !run_preview_state.requested {
        return;
    }
    let Some((url, offset)) = &run_preview_state.highlight else { return };
    session.send_to_preview(
        RUN_PREVIEW_INDEX,
        &LspToPreviewMessage::HighlightFromEditor { url: Some(url.clone()), offset: *offset },
    );
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

    fn clear_messages(messages: &[editor_preview::test::CapturedPreviewMessages]) {
        for messages in messages {
            messages.borrow_mut().clear();
        }
    }

    fn component(url: &str, name: &str) -> PreviewComponent {
        PreviewComponent { url: Url::parse(url).unwrap(), component: Some(name.into()) }
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
        let mut run_preview_state = RunPreviewState::default();
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
            &mut run_preview_state,
            &Default::default(),
        ));

        assert!(messages[0].borrow().is_empty());
        assert!(
            messages[1]
                .borrow()
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::OpenProject { .. }))
        );

        clear_messages(&messages);

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestPreview { component: component.clone() },
            1,
            &mut session,
            project.path(),
            &mut run_preview_state,
            &Default::default(),
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

    #[test]
    fn run_preview_uses_the_primary_preview_target() {
        let (mut session, messages) = session_with_recording_previews();
        let primary_component = component("file:///primary.slint", "Primary");
        session.show_preview(PRIMARY_PREVIEW_INDEX, primary_component.clone());
        clear_messages(&messages);

        let mut run_preview_state = RunPreviewState::default();
        handle_editor_message(
            EditorToSessionMessage::RunPreview,
            &mut session,
            &mut run_preview_state,
        );

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component) if component == &primary_component)
        }));
        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(primary_component));
    }

    #[test]
    fn run_preview_target_changes_only_when_run_is_requested() {
        let (mut session, messages) = session_with_recording_previews();
        let first_component = component("file:///first.slint", "First");
        let second_component = component("file:///second.slint", "Second");
        session.show_preview(PRIMARY_PREVIEW_INDEX, first_component.clone());
        let mut run_preview_state = RunPreviewState::default();
        handle_editor_message(
            EditorToSessionMessage::RunPreview,
            &mut session,
            &mut run_preview_state,
        );
        clear_messages(&messages);

        session.show_preview(PRIMARY_PREVIEW_INDEX, second_component.clone());

        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(first_component));
        assert!(messages[RUN_PREVIEW_INDEX].borrow().is_empty());

        handle_editor_message(
            EditorToSessionMessage::RunPreview,
            &mut session,
            &mut run_preview_state,
        );
        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(second_component));
    }

    #[test]
    fn primary_show_document_highlights_the_run_preview() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("main.slint");
        let url = Url::from_file_path(&path).unwrap();
        let source = "export component Main {\n    Text {}\n}";
        spin_on::spin_on(session.load_document_impl(source.into(), url.clone(), None));
        session.show_preview(
            PRIMARY_PREVIEW_INDEX,
            PreviewComponent { url: url.clone(), component: Some("Main".into()) },
        );
        let mut run_preview_state = RunPreviewState::default();
        handle_editor_message(
            EditorToSessionMessage::RunPreview,
            &mut session,
            &mut run_preview_state,
        );
        clear_messages(&messages);

        let expected_offset = u32::try_from(source.find("Text").unwrap()).unwrap();
        let document = session.document_cache.get_document(&url).unwrap().node.as_ref().unwrap();
        let position = editor_preview::util::text_size_to_lsp_position(
            &document.source_file,
            expected_offset.into(),
            session.document_cache.format,
        );
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::ShowDocument {
                file: url.clone(),
                selection: lsp_types::Range::new(position, position),
                take_focus: false,
            },
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            project.path(),
            &mut run_preview_state,
            &Default::default(),
        ));

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: Some(current_url), offset }
                if current_url == &url && *offset == expected_offset)
        }));

        clear_messages(&messages);
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
            &mut run_preview_state,
            &Default::default(),
        ));
        let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
        let show_position = run_messages
            .iter()
            .position(|message| matches!(message, LspToPreviewMessage::ShowPreview(_)))
            .unwrap();
        let highlight_position = run_messages.iter().position(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: Some(current_url), offset }
                if current_url == &url && *offset == expected_offset)
        });
        assert!(highlight_position.is_some_and(|position| position > show_position));
    }

    #[test]
    fn show_document_from_the_run_preview_is_ignored() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let mut run_preview_state = RunPreviewState { requested: true, highlight: None };

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::ShowDocument {
                file: Url::parse("file:///run.slint").unwrap(),
                selection: Default::default(),
                take_focus: false,
            },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
            &mut run_preview_state,
            &Default::default(),
        ));

        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
        assert!(run_preview_state.highlight.is_none());
    }
}
