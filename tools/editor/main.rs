// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Keep Windows from opening a console behind the editor. Debug builds keep
// theirs, so that printing something still reaches somewhere visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    cell::{Cell, RefCell},
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
    time::Duration,
};

use i_slint_compiler::source_path::SourcePath;
use i_slint_editor_preview as editor_preview;
use i_slint_editor_preview::{LspToPreviews, Result, document_cache::OpenImportCallback};
use i_slint_live_preview::file_watcher::{FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewTarget, PreviewToLspMessage, SourceFileVersion,
    VersionedUrl,
};
use lsp_types::{MessageType, Url};
use slint::ComponentHandle;

#[cfg(not(target_arch = "wasm32"))]
mod file_dialog;
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
    SwitchProject(Project),
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
    let active_session = RefCell::new(None::<crossbeam_channel::Sender<EditorToSessionMessage>>);
    let editor_ui_weak = editor_ui.as_weak();
    let start_project = Rc::new(move |project| {
        let mut session_slot = active_session.borrow_mut();
        if let Some(session_sender) = session_slot.as_ref() {
            return session_sender.send(EditorToSessionMessage::SwitchProject(project)).is_ok();
        }
        let Some(editor_ui) = editor_ui_weak.upgrade() else {
            return false;
        };
        *session_slot = Some(start_editor_session(&editor_ui, project, startup::load_settings()));
        true
    });
    startup::setup(&editor_ui, &settings, start_project.clone());
    if let Some(file) = cli.file {
        let project = Project::from_file(file, cli.component)?;
        start_project(project);
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
) -> crossbeam_channel::Sender<EditorToSessionMessage> {
    let (to_lsp, from_preview) = crossbeam_channel::unbounded();
    let (to_editor_session, from_editor_preview) = crossbeam_channel::unbounded();
    let preview_global = editor_ui.global::<preview::ui::Preview>();
    let run_preview_sender = to_editor_session.clone();
    preview_global.on_run(move || {
        run_preview_sender.send(EditorToSessionMessage::RunPreview).ok();
    });
    let springboard_global = editor_ui.global::<editor_preview::springboard_ui::Springboard>();
    let springboard_global = <editor_preview::springboard_ui::Springboard as slint::Global<
        '_,
        preview::ui::EditorUi,
    >>::as_weak(&springboard_global);
    let to_lsp = Rc::new(EmbeddedPreviewToLsp { sender: to_lsp })
        as Rc<dyn editor_preview::PreviewToLsp + 'static>;
    preview::ui::initialize_editor(editor_ui, &to_lsp, "");
    preview::initialize(editor_ui, to_lsp, settings);
    start_lsp_thread(vec![from_preview], from_editor_preview, project, springboard_global);
    to_editor_session
}

fn start_lsp_thread(
    from_previews: Vec<crossbeam_channel::Receiver<PreviewToLspMessage>>,
    from_editor_preview: crossbeam_channel::Receiver<EditorToSessionMessage>,
    project: Project,
    springboard_global: slint::Weak<editor_preview::springboard_ui::Springboard<'static>>,
) {
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) = local_set.block_on(
            &rt,
            lsp_main(from_previews, from_editor_preview, project, springboard_global),
        ) {
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
    springboard_global: slint::Weak<editor_preview::springboard_ui::Springboard<'static>>,
) -> Result<()> {
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

    let local_preview = editor_preview::springboard::LocalPreviewConfig {
        executable: std::env::current_exe()?,
        arguments: vec![std::ffi::OsString::from("--run-preview-child")],
    };
    let springboard = editor_preview::springboard::Springboard::new(
        local_preview,
        from_run_preview_sender,
        springboard_global,
    );
    let to_previews = vec![
        LspToPreviews::with_one(EditorLspToPreview),
        LspToPreviews::with_one(springboard.clone()),
    ];

    let (mut session, publish_imports) = new_editor_session(to_previews);

    assert_eq!(session.previews.len(), from_previews.len());

    let mut watch_paths_revision = None;
    let mut project_root = project.root;
    prepare_initial_preview(&mut session, &mut file_watcher, &project_root, project.preview)
        .await?;
    publish_imports.set(true);
    sync_file_watcher_if_needed(
        &mut file_watcher,
        &session,
        &project_root,
        &mut watch_paths_revision,
    )?;

    const RECOMPILE_DELAY: Duration = Duration::from_millis(50);
    let mut recompile_deadline = None;
    loop {
        if session.pending_recompile.is_empty() {
            recompile_deadline = None;
        } else {
            // Preview messages must not postpone a pending source update.
            recompile_deadline.get_or_insert_with(|| tokio::time::Instant::now() + RECOMPILE_DELAY);
        }
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
            editor_message = from_editor.recv() => {
                match editor_message {
                    Some(EditorToSessionMessage::RunPreview) => {
                        if run_preview(&mut session) {
                            springboard.start();
                        }
                    }
                    Some(EditorToSessionMessage::SwitchProject(project)) => {
                        watch_paths_revision = None;
                        if let Err(error) = switch_project(
                            &mut session,
                            &mut file_watcher,
                            &mut project_root,
                            project,
                        ).await {
                            tracing::warn!("Failed to switch project: {error}");
                        }
                    }
                    None => break Ok(()),
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

fn new_editor_session(
    to_previews: Vec<Rc<LspToPreviews>>,
) -> (editor_preview::EditorSession, Rc<Cell<bool>>) {
    use editor_preview::document_cache::CompilerConfiguration;

    let publish_imports = Rc::new(Cell::new(false));
    let open_import_callback = {
        let to_previews = to_previews.clone();
        let publish_imports = publish_imports.clone();
        Rc::new(move |path: SourcePath| {
            let to_previews = to_previews.clone();
            let publish_imports = publish_imports.clone();
            Box::pin(async move {
                tracing::trace!("Importing file: {path}");
                let contents = path.read().map(std::borrow::Cow::into_owned);
                if publish_imports.get()
                    && let SourcePath::File(_) = &path
                    && let Some(url) = path.to_url()
                {
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

    let mut session = editor_preview::EditorSession::with_previews(
        editor_preview::DocumentCache::new(compiler_config),
        to_previews
            .into_iter()
            .map(|to_preview| editor_preview::PreviewConnection {
                to_preview,
                to_show: Default::default(),
            })
            .collect(),
    );
    session.preview_config = i_slint_live_preview::protocol::PreviewConfig {
        style: "fluent".into(),
        ..Default::default()
    };
    (session, publish_imports)
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
        std::iter::once(root_path.to_path_buf())
            .chain(session.document_cache.all_paths_to_watch())
            .chain(session.previews.iter().filter_map(|preview| {
                preview
                    .to_show
                    .as_ref()
                    .and_then(|component| SourcePath::from_url(&component.url).into_native_path())
            }))
            .chain(session.active_project_file_paths().map(Path::to_path_buf)),
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
            session.send_to_preview(
                RUN_PREVIEW_INDEX,
                &LspToPreviewMessage::HighlightFromEditor {
                    url: Some(file.clone()),
                    offset: offset.into(),
                },
            );
        }
        ClearHighlight if preview_index == PRIMARY_PREVIEW_INDEX => {
            session.send_to_preview(
                RUN_PREVIEW_INDEX,
                &LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
            );
        }

        Diagnostics { .. }
        | ShowDocument { .. }
        | ClearHighlight
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
            let applied = handle_workspace_edit(&session.document_cache, label.as_deref(), edit);
            preview::workspace_edit_finished(edit.clone(), applied);
        }
    }
}

async fn prepare_initial_preview(
    session: &mut editor_preview::EditorSession,
    watcher: &mut FileWatcher,
    project_root: &Path,
    component: PreviewComponent,
) -> Result<()> {
    watcher.update_watched_paths(
        std::iter::once(project_root.to_path_buf())
            .chain(SourcePath::from_url(&component.url).into_native_path()),
    )?;
    let source = SourcePath::from_url(&component.url).read_to_string()?;
    let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
    session.document_cache.load_url(&component.url, None, source, &mut diagnostics).await?;
    session.primary_preview_mut().to_show = Some(component);
    Ok(())
}

fn run_preview(session: &mut editor_preview::EditorSession) -> bool {
    let Some(component) = session.primary_preview().to_show.clone() else {
        tracing::warn!("Cannot run a preview before a component is open");
        return false;
    };
    session.show_preview(RUN_PREVIEW_INDEX, component);
    true
}

async fn switch_project(
    session: &mut editor_preview::EditorSession,
    file_watcher: &mut FileWatcher,
    project_root: &mut PathBuf,
    project: Project,
) -> Result<()> {
    let to_previews = session.previews.iter().map(|preview| preview.to_preview.clone()).collect();
    let (mut next_session, publish_imports) = new_editor_session(to_previews);
    let next_project_root = project.root;
    prepare_initial_preview(&mut next_session, file_watcher, &next_project_root, project.preview)
        .await?;
    publish_imports.set(true);

    session.previews[RUN_PREVIEW_INDEX].to_preview.shutdown().await;
    session.send_to_preview(
        RUN_PREVIEW_INDEX,
        &LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 },
    );
    *session = next_session;
    *project_root = next_project_root;
    open_project(session, PRIMARY_PREVIEW_INDEX, project_root)?;
    session.send_state_to_preview(PRIMARY_PREVIEW_INDEX);
    Ok(())
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
    let path = SourcePath::from_url(&component.url).into_native_path()?;
    let path = std::fs::canonicalize(path).ok()?;
    let url = Url::from_file_path(&path).ok()?;
    Some((PreviewComponent { url, component: component.component.clone() }, path))
}

fn handle_workspace_edit(
    document_cache: &editor_preview::DocumentCache,
    label: Option<&str>,
    edit: &lsp_types::WorkspaceEdit,
) -> bool {
    match editor_preview::editing::text_edit::apply_workspace_edit(document_cache, edit) {
        Ok(edited_texts) => {
            let mut applied = true;
            for editor_preview::editing::text_edit::EditedText { url, contents } in edited_texts {
                match SourcePath::from_url(&url).into_native_path() {
                    Some(path) => {
                        if let Err(err) = std::fs::write(&path, &contents) {
                            applied = false;
                            tracing::error!(
                                "Failed to apply workspace edit '{}' to {}: {err}",
                                label.unwrap_or("(unnamed)"),
                                path.display()
                            );
                        }
                    }
                    None => {
                        applied = false;
                        tracing::warn!("Cannot apply workspace edit to non-file URL: {url}");
                    }
                }
            }
            applied
        }
        Err(err) => {
            tracing::error!(
                "Failed to compute workspace edit '{}': {err}",
                label.unwrap_or("(unnamed)")
            );
            false
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
        let mut session = editor_preview::EditorSession::new(
            editor_preview::DocumentCache::new(Default::default()),
            LspToPreviews::with_one(RepairOnPreview(source.clone())),
        );
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
        spin_on::spin_on(prepare_initial_preview(
            &mut session,
            &mut watcher,
            &root,
            PreviewComponent { url: url.clone(), component: None },
        ))
        .unwrap();
        sync_file_watcher_if_needed(&mut watcher, &session, &root, &mut None).unwrap();
        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            &root,
        ));
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

    #[test]
    fn initial_preview_is_published_once_in_response_to_request_state() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source_path = root.join("Main.slint");
        let import_path = root.join("Child.slint");
        std::fs::write(
            &source_path,
            "import { Child } from \"Child.slint\"; export component Main { Child {} }",
        )
        .unwrap();
        std::fs::write(&import_path, "export component Child inherits Rectangle {}").unwrap();
        let captures: [_; 2] = std::array::from_fn(|_| editor_preview::test::preview_capture());
        let (mut session, publish_imports) =
            new_editor_session(captures.iter().map(|(connection, _)| connection.clone()).collect());
        let component = PreviewComponent {
            url: Url::from_file_path(&source_path).unwrap(),
            component: Some("Main".into()),
        };
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();

        spin_on::spin_on(prepare_initial_preview(
            &mut session,
            &mut watcher,
            &root,
            component.clone(),
        ))
        .unwrap();
        publish_imports.set(true);
        assert!(captures.iter().all(|(_, messages)| messages.borrow().is_empty()));
        assert_eq!(session.primary_preview().to_show, Some(component.clone()));
        assert!(session.pending_recompile.is_empty());

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() },
            PRIMARY_PREVIEW_INDEX,
            &mut session,
            &root,
        ));
        let messages = captures[PRIMARY_PREVIEW_INDEX].1.borrow();
        assert_eq!(messages.len(), 5);
        assert!(matches!(&messages[0], LspToPreviewMessage::OpenProject { root: current_root }
            if current_root == &Url::from_directory_path(&root).unwrap()));
        let mut source_urls = messages[1..3]
            .iter()
            .map(|message| {
                let LspToPreviewMessage::SetContents { url, .. } = message else {
                    panic!("Expected source contents: {message:?}")
                };
                url.url().clone()
            })
            .collect::<Vec<_>>();
        source_urls.sort();
        let mut expected_urls =
            [source_path, import_path].map(|path| Url::from_file_path(path).unwrap());
        expected_urls.sort();
        assert_eq!(source_urls, expected_urls);
        assert!(matches!(&messages[3], LspToPreviewMessage::SetConfiguration { .. }));
        assert!(
            matches!(&messages[4], LspToPreviewMessage::ShowPreview(current) if current == &component)
        );
        assert!(captures[RUN_PREVIEW_INDEX].1.borrow().is_empty());
    }

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
        let session = editor_preview::EditorSession::with_previews(
            editor_preview::test::empty_document_cache(),
            previews,
        );
        (session, messages)
    }

    fn clear_messages(messages: &[editor_preview::test::CapturedPreviewMessages]) {
        for messages in messages {
            messages.borrow_mut().clear();
        }
    }

    fn component(file_name: &str, name: &str) -> PreviewComponent {
        PreviewComponent {
            url: editor_preview::test::test_file_name(file_name).to_url().unwrap(),
            component: Some(name.into()),
        }
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
    fn switching_project_replaces_the_document_session_and_opens_the_new_preview() {
        let (mut session, messages) = session_with_recording_previews();
        let old_project = tempfile::tempdir().unwrap();
        let old_path = old_project.path().join("old.slint");
        let old_url = Url::from_file_path(&old_path).unwrap();
        spin_on::spin_on(session.load_document_impl(
            "export component Old {}".into(),
            old_url.clone(),
            None,
        ));
        assert!(session.document_cache.get_document(&old_url).is_some());
        session.show_preview(
            PRIMARY_PREVIEW_INDEX,
            PreviewComponent { url: old_url.clone(), component: Some("Old".into()) },
        );
        assert!(run_preview(&mut session));

        let old_run_preview = session.previews[RUN_PREVIEW_INDEX].to_preview.clone();
        let new_project = tempfile::tempdir().unwrap();
        let new_path = new_project.path().join("new.slint");
        std::fs::write(&new_path, "export component New {}").unwrap();
        let project = Project::from_file(&new_path, Some("New".into())).unwrap();
        let expected_root = project.root.clone();
        let expected_component = project.preview.clone();
        let mut project_root = old_project.path().to_path_buf();
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        clear_messages(&messages);

        spin_on::spin_on(switch_project(&mut session, &mut watcher, &mut project_root, project))
            .unwrap();

        assert_eq!(project_root, expected_root);
        assert!(session.document_cache.get_document(&old_url).is_none());
        assert!(session.document_cache.get_document(&expected_component.url).is_some());
        assert!(session.previews[RUN_PREVIEW_INDEX].to_show.is_none());
        assert!(Rc::ptr_eq(&session.previews[RUN_PREVIEW_INDEX].to_preview, &old_run_preview));
        let expected_root_url =
            Url::from_directory_path(std::fs::canonicalize(&project_root).unwrap()).unwrap();
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::OpenProject { root }
                if root == &expected_root_url)
        }));
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component)
                if component == &expected_component)
        }));
        assert!(
            messages[PRIMARY_PREVIEW_INDEX]
                .borrow()
                .iter()
                .all(|message| !matches!(message, LspToPreviewMessage::Quit))
        );
        assert_eq!(
            messages[RUN_PREVIEW_INDEX]
                .borrow()
                .iter()
                .filter(|message| matches!(message, LspToPreviewMessage::Quit))
                .count(),
            1
        );
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: None, .. })
        }));
        assert!(
            messages[RUN_PREVIEW_INDEX]
                .borrow()
                .iter()
                .all(|message| { !matches!(message, LspToPreviewMessage::ShowPreview(_)) })
        );

        clear_messages(&messages);
        assert!(run_preview(&mut session));
        assert_eq!(session.previews[RUN_PREVIEW_INDEX].to_show, Some(expected_component.clone()));
        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component)
                if component == &expected_component)
        }));
    }

    #[test]
    fn failed_project_switch_preserves_the_active_session() {
        let (mut session, messages) = session_with_recording_previews();
        let old_project = tempfile::tempdir().unwrap();
        let old_path = old_project.path().join("old.slint");
        let old_url = Url::from_file_path(&old_path).unwrap();
        spin_on::spin_on(session.load_document_impl(
            "export component Old {}".into(),
            old_url.clone(),
            None,
        ));
        let old_component =
            PreviewComponent { url: old_url.clone(), component: Some("Old".into()) };
        session.show_preview(PRIMARY_PREVIEW_INDEX, old_component.clone());
        assert!(run_preview(&mut session));
        let old_run_preview = session.previews[RUN_PREVIEW_INDEX].to_preview.clone();
        let mut project_root = old_project.path().to_path_buf();
        let expected_root = project_root.clone();
        let missing_root = old_project.path().join("missing");
        let project = Project {
            root: missing_root.clone(),
            preview: PreviewComponent {
                url: Url::from_file_path(missing_root.join("main.slint")).unwrap(),
                component: Some("Missing".into()),
            },
        };
        let mut watcher = FileWatcher::start(|_| {}, |_| {}).unwrap();
        clear_messages(&messages);

        assert!(
            spin_on::spin_on(switch_project(
                &mut session,
                &mut watcher,
                &mut project_root,
                project,
            ))
            .is_err()
        );

        assert_eq!(project_root, expected_root);
        assert!(session.document_cache.get_document(&old_url).is_some());
        assert_eq!(session.primary_preview().to_show, Some(old_component.clone()));
        assert_eq!(session.previews[RUN_PREVIEW_INDEX].to_show, Some(old_component));
        assert!(Rc::ptr_eq(&session.previews[RUN_PREVIEW_INDEX].to_preview, &old_run_preview));
        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
    }

    #[test]
    fn preview_requests_are_answered_through_the_originating_connection() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();
        let path = project.path().join("secondary.slint");
        std::fs::write(&path, "export component Secondary {}").unwrap();
        let path = std::fs::canonicalize(path).unwrap();
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

        clear_messages(&messages);

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

    #[test]
    fn run_without_a_component_does_not_start_springboard() {
        let (mut session, messages) = session_with_recording_previews();
        assert!(!run_preview(&mut session));
        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
    }

    #[test]
    fn run_preview_uses_the_primary_preview_target() {
        let (mut session, messages) = session_with_recording_previews();
        let primary_component = component("primary.slint", "Primary");
        session.show_preview(PRIMARY_PREVIEW_INDEX, primary_component.clone());
        clear_messages(&messages);

        run_preview(&mut session);

        assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
        assert!(messages[RUN_PREVIEW_INDEX].borrow().iter().any(|message| {
            matches!(message, LspToPreviewMessage::ShowPreview(component) if component == &primary_component)
        }));
        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(primary_component));
    }

    #[test]
    fn run_preview_target_changes_only_when_run_is_requested() {
        let (mut session, messages) = session_with_recording_previews();
        let first_component = component("first.slint", "First");
        let second_component = component("second.slint", "Second");
        session.show_preview(PRIMARY_PREVIEW_INDEX, first_component.clone());
        run_preview(&mut session);
        clear_messages(&messages);

        session.show_preview(PRIMARY_PREVIEW_INDEX, second_component.clone());

        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(first_component));
        assert!(messages[RUN_PREVIEW_INDEX].borrow().is_empty());

        run_preview(&mut session);
        assert_eq!(session.preview(RUN_PREVIEW_INDEX).unwrap().to_show, Some(second_component));
    }

    #[test]
    fn editor_forwards_highlight_without_replaying_it_in_state_response() {
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
        run_preview(&mut session);
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
        ));
        let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
        assert!(
            run_messages
                .iter()
                .any(|message| matches!(message, LspToPreviewMessage::ShowPreview(_)))
        );
        let highlight_position = run_messages.iter().position(|message| {
            matches!(message, LspToPreviewMessage::HighlightFromEditor { url: Some(current_url), offset }
                if current_url == &url && *offset == expected_offset)
        });
        assert!(highlight_position.is_none());
    }

    #[test]
    fn clear_highlight_only_clears_the_run_preview_from_the_primary_preview() {
        for preview_index in [PRIMARY_PREVIEW_INDEX, RUN_PREVIEW_INDEX] {
            let (mut session, messages) = session_with_recording_previews();
            let project = tempfile::tempdir().unwrap();

            spin_on::spin_on(handle_preview_message(
                PreviewToLspMessage::ClearHighlight,
                preview_index,
                &mut session,
                project.path(),
            ));

            assert!(messages[PRIMARY_PREVIEW_INDEX].borrow().is_empty());
            let run_messages = messages[RUN_PREVIEW_INDEX].borrow();
            if preview_index == PRIMARY_PREVIEW_INDEX {
                assert!(matches!(
                    run_messages.as_slice(),
                    [LspToPreviewMessage::HighlightFromEditor { url: None, offset: 0 }]
                ));
            } else {
                assert!(run_messages.is_empty());
            }
        }
    }

    #[test]
    fn show_document_from_the_run_preview_is_ignored() {
        let (mut session, messages) = session_with_recording_previews();
        let project = tempfile::tempdir().unwrap();

        spin_on::spin_on(handle_preview_message(
            PreviewToLspMessage::ShowDocument {
                file: editor_preview::test::test_file_name("run.slint").to_url().unwrap(),
                selection: Default::default(),
                take_focus: false,
            },
            RUN_PREVIEW_INDEX,
            &mut session,
            project.path(),
        ));

        assert!(messages.iter().all(|messages| messages.borrow().is_empty()));
    }
}
