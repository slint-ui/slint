// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Keep Windows from opening a console behind the editor. Debug builds keep
// theirs, so that printing something still reaches somewhere visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    path::{Path, PathBuf},
    pin::Pin,
    rc::Rc,
    time::Duration,
};

use i_slint_editor_preview as editor_preview;
use i_slint_editor_preview::preview;
use i_slint_editor_preview::{LspToPreviews, Result, document_cache::OpenImportCallback};
use i_slint_live_preview::file_watcher::{FileWatcher, WatchEvent};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewTarget, PreviewToLspMessage, SourceFileVersion,
    VersionedUrl,
};
use lsp_types::{MessageType, Url};

#[cfg(target_os = "macos")]
mod sparkle;
mod springboard;
mod springboard_ui;

fn main() -> std::result::Result<(), slint::PlatformError> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    use clap::Parser;

    let cli = Cli::parse();

    let (to_lsp, from_preview) = crossbeam_channel::unbounded();
    let (springboard_actions, from_springboard_ui) = crossbeam_channel::unbounded();

    let to_lsp = Rc::new(EmbeddedPreviewToLsp { sender: to_lsp })
        as Rc<dyn editor_preview::PreviewToLsp + 'static>;

    // Set up the Slint backend (installing the macOS unified-title-bar hook)
    // *before* spawning the LSP thread, so that no other thread can lazily
    // initialize the default platform first and lose the hook.
    select_backend()?;

    let _lsp_thread = start_lsp_thread(from_preview, from_springboard_ui, cli);

    let app_window = preview::ui::create_ui(&to_lsp, "", preview::PreviewUiKind::Editor)?;
    springboard_ui::setup(&app_window, springboard_actions);

    // The updater needs to stay in scope for as long as the window is up.
    #[cfg(target_os = "macos")]
    let _updater = setup_macos_chrome(&app_window);

    preview::run_with_ui(app_window, to_lsp, false)
}

/// Set up the editor's macOS chrome: the unified title bar and the Sparkle
/// auto-updater driving the update section of the editor UI.
#[cfg(target_os = "macos")]
fn setup_macos_chrome(app_window: &preview::ui::AppWindow) -> Option<Rc<crate::sparkle::Sparkle>> {
    use preview::ui;
    use slint::ComponentHandle;

    let ui::AppWindow::Editor(editor) = app_window.clone_strong() else {
        return None;
    };

    preview::macos_titlebar::setup(editor.as_weak());
    crate::sparkle::connect(&editor)
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
    // See bug #10274 on macOS.
    let selector = slint::BackendSelector::new();
    // On macOS, request a unified title bar: the editor content extends underneath
    // a transparent title bar (see `preview::macos_titlebar`).
    #[cfg(target_os = "macos")]
    let selector =
        selector.with_winit_window_attributes_hook(preview::macos_titlebar::apply_unified_titlebar);
    selector.select()
}

struct LspThread {
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl Drop for LspThread {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(handle) = self.handle.take()
            && handle.join().is_err()
        {
            tracing::error!("Visual Editor host thread panicked during shutdown");
        }
    }
}

fn start_lsp_thread(
    from_preview: crossbeam_channel::Receiver<PreviewToLspMessage>,
    from_springboard_ui: crossbeam_channel::Receiver<springboard_ui::SpringboardUiAction>,
    cli: Cli,
) -> LspThread {
    let (shutdown, shutdown_rx) = tokio::sync::oneshot::channel();
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .enable_time()
            .build()
            .unwrap();
        let local_set = tokio::task::LocalSet::new();
        if let Err(err) =
            local_set.block_on(&rt, lsp_main(from_preview, from_springboard_ui, cli, shutdown_rx))
        {
            tracing::error!("{err}");
            std::process::exit(1);
        }
    });
    LspThread { shutdown: Some(shutdown), handle: Some(handle) }
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
    from_springboard_ui: crossbeam_channel::Receiver<springboard_ui::SpringboardUiAction>,
    cli: Cli,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    use editor_preview::document_cache::CompilerConfiguration;

    let mut from_preview_rx = bridge_crossbeam_to_tokio(from_preview);
    let (springboard_actions_tx, mut springboard_actions_rx) =
        tokio::sync::mpsc::unbounded_channel();
    std::thread::spawn(move || {
        while let Ok(action) = from_springboard_ui.recv() {
            if springboard_actions_tx.send(action).is_err() {
                break;
            }
        }
    });
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

    let open_import_callback = {
        let to_preview = Rc::clone(&to_preview);
        Rc::new(move |path: String| {
            let to_preview = Rc::clone(&to_preview);
            Box::pin(async move {
                tracing::trace!("Importing file: {}", path);
                let contents = std::fs::read(&path);
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
    let mut project_root = None;
    let mut springboard_process = springboard::SpringboardProcess::default();
    let mut run_after_springboard_snapshot = false;

    // Load the initial document through the compiler if the editor was launched
    // with a file. Finder launches without a file stay on the startup wizard.
    if let Some(file) = cli.file.as_ref() {
        let full_path = std::fs::canonicalize(file)
            .map_err(|err| format!("Failed to determine full path for {file}: {err}"))?;
        let url = Url::from_file_path(full_path.clone())
            .map_err(|_| format!("Failed to convert {file} to URL!"))?;
        session.show_preview(PreviewComponent { url: url.clone(), component: cli.component });

        // Make sure the document is loaded before we start processing messages from the preview, so
        // we have the correct state already loaded.
        // The editor has no LSP client to publish diagnostics to, so they are dropped here.
        let _diagnostics = session
            .reload_document(url)
            .await
            .map_err(|err| format!("Failed to load file: {file}: {err}"))?;
        project_root = project_root_for_path(&full_path);
        if let Some(project_root) = project_root.as_deref()
            && let Err(error) = springboard_process.ensure_project(project_root).await
        {
            forward_springboard_error(error.to_string());
        }
        sync_file_watcher_if_needed(
            &mut file_watcher,
            &session,
            project_root.as_deref().unwrap_or(&full_path),
            &mut watch_paths_revision,
        )?;
    }

    const RECOMPILE_IDLE_TIMEOUT: Duration = Duration::from_millis(50);
    let result = loop {
        let recompile_idle_timeout = if session.pending_recompile.is_empty() {
            Duration::MAX
        } else {
            RECOMPILE_IDLE_TIMEOUT
        };
        tokio::select! {
            _ = &mut shutdown => {
                tracing::debug!("Visual Editor host thread shutting down");
                break Ok(());
            }
            watcher_event = file_watcher_rx.recv() => {
                match watcher_event {
                    Some(event) => trigger_editor_file_watcher(&mut session, event).await?,
                    None => break Err("File watcher channel closed".into()),
                }
            }
            msg = from_preview_rx.recv() => {
                match msg {
                    Some(msg) => {
                        if let Some(root) = handle_preview_message(
                            msg,
                            &mut session,
                            &mut springboard_process,
                            &mut run_after_springboard_snapshot,
                        ).await {
                            project_root = Some(root);
                            watch_paths_revision = None;
                        }
                    }
                    None => {
                        tracing::debug!("Preview->LSP channel closed, exiting");
                        break Ok(());
                    }
                }
            }
            springboard_event = springboard_process.recv() => {
                if let Some(event) = springboard_event {
                    let last_used = springboard_snapshot_last_used(&event);
                    let failed = matches!(
                        event,
                        springboard::SpringboardHostEvent::Error(_)
                            | springboard::SpringboardHostEvent::Closed
                    );
                    handle_springboard_event(event);
                    if failed {
                        run_after_springboard_snapshot = false;
                    } else if run_after_springboard_snapshot
                        && let Some(last_used) = last_used
                    {
                        run_after_springboard_snapshot = false;
                        if let Some(device_id) = last_used {
                            if let Err(error) = springboard_process
                                .send(i_slint_springboard::ClientCommand::Launch { device_id })
                                .await
                            {
                                forward_springboard_error(error.to_string());
                            }
                        } else {
                            forward_open_device_manager();
                        }
                    }
                }
            }
            action = springboard_actions_rx.recv() => {
                if let Some(action) = action
                    && let Err(error) = handle_springboard_action(
                        &mut springboard_process,
                        &session,
                        &mut run_after_springboard_snapshot,
                        action,
                    ).await
                {
                    let message = error.to_string();
                    forward_springboard_error(message);
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

        if let Some(project_root) = project_root.as_deref() {
            if let Err(error) = springboard_process.ensure_project(project_root).await {
                forward_springboard_error(error.to_string());
            }
            sync_file_watcher_if_needed(
                &mut file_watcher,
                &session,
                project_root,
                &mut watch_paths_revision,
            )?;
        }
    };

    if let Err(error) = springboard_process.stop().await {
        tracing::error!("Failed to stop Springboard: {error}");
    }
    result
}

fn handle_springboard_event(event: springboard::SpringboardHostEvent) {
    match event {
        springboard::SpringboardHostEvent::Message(message) => {
            forward_springboard_message(message);
        }
        springboard::SpringboardHostEvent::Error(message) => {
            forward_springboard_error(message);
        }
        springboard::SpringboardHostEvent::Closed => {
            let message =
                "Springboard exited unexpectedly. Restart the project or reopen the editor."
                    .to_string();
            forward_springboard_error(message);
        }
    }
}

fn springboard_snapshot_last_used(
    event: &springboard::SpringboardHostEvent,
) -> Option<Option<i_slint_springboard::DeviceId>> {
    let springboard::SpringboardHostEvent::Message(message) = event else { return None };
    match message {
        i_slint_springboard::ServerMessage::Response(response) => match &response.response {
            i_slint_springboard::ResponsePayload::Snapshot { snapshot } => {
                Some(snapshot.last_used_device.clone())
            }
            _ => None,
        },
        i_slint_springboard::ServerMessage::Event(event) => match &event.event {
            i_slint_springboard::ServerEvent::Snapshot { snapshot } => {
                Some(snapshot.last_used_device.clone())
            }
            _ => None,
        },
    }
}

fn forward_springboard_message(message: i_slint_springboard::ServerMessage) {
    if let Err(error) = slint::invoke_from_event_loop(move || {
        springboard_ui::apply_message(message);
    }) {
        tracing::error!("Failed to forward a Springboard event to the editor UI: {error}");
    }
}

fn forward_springboard_error(message: String) {
    if let Err(error) = slint::invoke_from_event_loop(move || {
        springboard_ui::set_connection_error(message);
    }) {
        tracing::error!("Failed to forward a Springboard error to the editor UI: {error}");
    }
}

fn forward_open_device_manager() {
    if let Err(error) = slint::invoke_from_event_loop(springboard_ui::open_device_manager) {
        tracing::error!("Failed to open the Springboard device manager: {error}");
    }
}

async fn handle_springboard_action(
    process: &mut springboard::SpringboardProcess,
    session: &editor_preview::EditorSession,
    run_after_snapshot: &mut bool,
    action: springboard_ui::SpringboardUiAction,
) -> Result<()> {
    let command = match action {
        springboard_ui::SpringboardUiAction::ConfigureProject(project_root) => {
            let project_root = std::fs::canonicalize(&project_root).map_err(|error| {
                format!("Failed to resolve Visual Editor project {project_root}: {error}")
            })?;
            let project_root_url = Url::from_directory_path(&project_root).map_err(|()| {
                format!("Failed to convert project {} to a file URL", project_root.display())
            })?;
            if editor_preview::project::load_project_run_target(&project_root)?.is_none() {
                session.to_preview.send(&LspToPreviewMessage::SelectProjectEntry {
                    project_root: project_root_url,
                });
            } else {
                start_configured_springboard_project(process, &project_root, run_after_snapshot)
                    .await?;
            }
            return Ok(());
        }
        springboard_ui::SpringboardUiAction::Launch(device_id) => {
            i_slint_springboard::ClientCommand::Launch {
                device_id: i_slint_springboard::DeviceId::new(device_id)?,
            }
        }
        springboard_ui::SpringboardUiAction::Stop(device_id) => {
            i_slint_springboard::ClientCommand::Stop {
                device_id: i_slint_springboard::DeviceId::new(device_id)?,
            }
        }
        springboard_ui::SpringboardUiAction::Refresh(device_id) => {
            i_slint_springboard::ClientCommand::Refresh {
                device_id: i_slint_springboard::DeviceId::new(device_id)?,
            }
        }
        springboard_ui::SpringboardUiAction::Rebuild(device_id) => {
            i_slint_springboard::ClientCommand::Rebuild {
                device_id: i_slint_springboard::DeviceId::new(device_id)?,
            }
        }
        springboard_ui::SpringboardUiAction::AddManualDevice(address) => {
            i_slint_springboard::ClientCommand::AddManualDevice { address }
        }
    };
    process.send(command).await?;
    Ok(())
}

async fn start_configured_springboard_project(
    process: &mut springboard::SpringboardProcess,
    project_root: &Path,
    run_after_snapshot: &mut bool,
) -> Result<()> {
    process.ensure_project(project_root).await?;
    process.send(i_slint_springboard::ClientCommand::Snapshot).await?;
    *run_after_snapshot = true;
    Ok(())
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
    springboard_process: &mut springboard::SpringboardProcess,
    run_after_snapshot: &mut bool,
) -> Option<PathBuf> {
    use PreviewToLspMessage::*;
    match &msg {
        RequestState { files, settings } => {
            tracing::debug!("Preview requested state");
            let requested_preview = requested_file_tree_preview(files);
            let requested_project_root = requested_preview
                .as_ref()
                .and_then(editor_preview::uri_to_file)
                .and_then(|path| project_root_for_path(&path));
            let slint_files: Vec<_> =
                files.iter().filter(|url| is_slint_url(url)).cloned().collect();
            for url in slint_files {
                if let Err(err) = session.reload_document(url.clone()).await {
                    tracing::error!("Failed document reload requested by preview for {url}: {err}");
                }
            }
            if let Some(url) = requested_preview {
                session.to_show = Some(PreviewComponent { url, component: None });
            }
            if files.is_empty() {
                session.send_state_to_preview();
            } else {
                session.send_files_to_preview(files, |_| true);
            }
            for name in settings {
                if let Some(contents) = i_slint_editor_preview::settings_store::load(name) {
                    session.to_preview.send(&LspToPreviewMessage::SetUserSettings {
                        name: name.clone(),
                        contents,
                    });
                }
            }
            requested_project_root
        }
        UpdateUserSettings { name, contents } => {
            if let Err(error) = i_slint_editor_preview::settings_store::save(name, contents) {
                tracing::warn!("Failed to save preview user settings: {error}");
            }
            None
        }
        SendShowMessage { message } => {
            match message.typ {
                MessageType::ERROR => tracing::error!("Preview: {}", message.message),
                MessageType::WARNING => tracing::warn!("Preview: {}", message.message),
                MessageType::LOG => tracing::debug!("Preview: {}", message.message),
                _ => tracing::info!("Preview: {}", message.message),
            };
            None
        }
        DebugMessage { location, message } => {
            eprintln!("{}", editor_preview::preview_log_message_to_string(location, message));
            None
        }
        LaunchLivePreview { .. } => {
            tracing::debug!("Ignoring a standalone live-preview request in the Visual Editor");
            None
        }
        SetProjectEntry { project_root, entry } => {
            match configure_project_entry(session, project_root, entry).await {
                Ok(ProjectEntryAction::Configured) => match project_root_path(project_root) {
                    Ok(project_root) => {
                        if let Err(error) = start_configured_springboard_project(
                            springboard_process,
                            &project_root,
                            run_after_snapshot,
                        )
                        .await
                        {
                            forward_springboard_error(error.to_string());
                        }
                    }
                    Err(error) => send_project_preview_error(session, error.to_string()),
                },
                Ok(ProjectEntryAction::SelectComponent { entry, components }) => {
                    session.to_preview.send(&LspToPreviewMessage::SelectProjectComponent {
                        project_root: project_root.clone(),
                        entry,
                        components,
                    })
                }
                Err(error) => send_project_preview_error(session, error.to_string()),
            }
            None
        }
        SetProjectComponent { project_root, entry, component } => {
            match configure_project_component(session, project_root, entry, component).await {
                Ok(()) => match project_root_path(project_root) {
                    Ok(project_root) => {
                        if let Err(error) = start_configured_springboard_project(
                            springboard_process,
                            &project_root,
                            run_after_snapshot,
                        )
                        .await
                        {
                            forward_springboard_error(error.to_string());
                        }
                    }
                    Err(error) => send_project_preview_error(session, error.to_string()),
                },
                Err(error) => send_project_preview_error(session, error.to_string()),
            }
            None
        }

        Diagnostics { .. }
        | ShowDocument { .. }
        | PreviewTypeChanged { .. }
        | TelemetryEvent(..)
        | ConnectRemote { .. }
        | DisconnectRemote
        | Pong => {
            tracing::debug!("Ignoring message from preview: {msg:?}");
            None
        }
        SendWorkspaceEdit { label, edit } => {
            handle_workspace_edit(&session.document_cache, label.as_deref(), edit);
            None
        }
    }
}

fn project_root_for_path(path: &Path) -> Option<PathBuf> {
    editor_preview::project::project_root_for_path(path)
}

fn project_root_path(project_root: &Url) -> Result<PathBuf> {
    let project_root = editor_preview::uri_to_file(project_root)
        .ok_or_else(|| format!("Project preview requires a file URL, got {project_root}"))?;
    std::fs::canonicalize(&project_root).map_err(|error| {
        format!("Failed to resolve project directory {}: {error}", project_root.display()).into()
    })
}

#[derive(Debug, Eq, PartialEq)]
enum ProjectEntryAction {
    Configured,
    SelectComponent { entry: Url, components: Vec<String> },
}

struct ProjectEntryComponents {
    project_root: PathBuf,
    entry_file: PathBuf,
    entry_url: Url,
    components: Vec<String>,
}

async fn project_entry_components(
    session: &mut editor_preview::EditorSession,
    project_root: &Url,
    entry: &Url,
) -> Result<ProjectEntryComponents> {
    let project_root = editor_preview::uri_to_file(project_root)
        .ok_or_else(|| format!("Project preview requires a file URL, got {project_root}"))?;
    let project_root = std::fs::canonicalize(&project_root).map_err(|error| {
        format!("Failed to resolve project directory {}: {error}", project_root.display())
    })?;
    let entry_file = editor_preview::uri_to_file(entry)
        .ok_or_else(|| format!("Project entry requires a file URL, got {entry}"))?;
    let entry_file = std::fs::canonicalize(&entry_file).map_err(|error| {
        format!("Failed to resolve project entry {}: {error}", entry_file.display())
    })?;
    if !entry_file.starts_with(&project_root) {
        return Err(format!(
            "Project entry {} is outside {}",
            entry_file.display(),
            project_root.display()
        )
        .into());
    }
    if !entry_file
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
    {
        return Err(format!("Project entry {} is not a .slint file", entry_file.display()).into());
    }
    let entry_url = Url::from_file_path(&entry_file).map_err(|_| {
        format!("Failed to convert project entry {} to a file URL", entry_file.display())
    })?;
    session.reload_document(entry_url.clone()).await?;

    let mut components = Vec::new();
    editor_preview::component_catalog::all_exported_components(
        &session.document_cache,
        &mut |information| {
            information.is_exported
                && !information.is_global
                && !information.is_interface
                && information
                    .defined_at
                    .as_ref()
                    .is_some_and(|position| position.url() == &entry_url)
        },
        &mut components,
    );
    components.sort_by_key(|information| {
        information
            .defined_at
            .as_ref()
            .map(|position| u32::from(position.offset()))
            .unwrap_or(u32::MAX)
    });
    let components = components.into_iter().map(|information| information.name).collect();

    Ok(ProjectEntryComponents { project_root, entry_file, entry_url, components })
}

async fn configure_project_entry(
    session: &mut editor_preview::EditorSession,
    project_root: &Url,
    entry: &Url,
) -> Result<ProjectEntryAction> {
    let entry = project_entry_components(session, project_root, entry).await?;
    finish_project_entry(entry)
}

fn finish_project_entry(entry: ProjectEntryComponents) -> Result<ProjectEntryAction> {
    match entry.components.as_slice() {
        [] => {
            Err(format!("Project entry {} has no exported components", entry.entry_file.display())
                .into())
        }
        [component] => {
            create_project_component(&entry.project_root, &entry.entry_file, component)?;
            Ok(ProjectEntryAction::Configured)
        }
        _ => Ok(ProjectEntryAction::SelectComponent {
            entry: entry.entry_url,
            components: entry.components,
        }),
    }
}

async fn configure_project_component(
    session: &mut editor_preview::EditorSession,
    project_root: &Url,
    entry: &Url,
    component: &str,
) -> Result<()> {
    let entry = project_entry_components(session, project_root, entry).await?;
    if !entry.components.iter().any(|candidate| candidate == component) {
        return Err(format!(
            "Component {component} is not an exported component in {}",
            entry.entry_file.display()
        )
        .into());
    }
    create_project_component(&entry.project_root, &entry.entry_file, component)
}

fn create_project_component(project_root: &Path, entry_file: &Path, component: &str) -> Result<()> {
    editor_preview::project::create_project_manifest(project_root, entry_file, component)?;
    Ok(())
}

fn send_project_preview_error(session: &editor_preview::EditorSession, message: String) {
    tracing::error!("Project preview: {message}");
    session.to_preview.send(&LspToPreviewMessage::ProjectPreviewError { message });
}

fn requested_file_tree_preview(files: &[Url]) -> Option<Url> {
    if files.len() == 1 && is_slint_url(&files[0]) { Some(files[0].clone()) } else { None }
}

fn is_slint_url(url: &Url) -> bool {
    editor_preview::uri_to_file(url).is_some_and(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
    })
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

    #[test]
    fn springboard_snapshot_exposes_the_global_last_used_device() {
        let device_id = i_slint_springboard::DeviceId::new("builtin:local-viewer").unwrap();
        let event = springboard::SpringboardHostEvent::Message(
            i_slint_springboard::ServerMessage::Event(i_slint_springboard::EventEnvelope::new(
                i_slint_springboard::ServerEvent::Snapshot {
                    snapshot: i_slint_springboard::ProjectSnapshot {
                        project_root: PathBuf::from("/project"),
                        entry_file: PathBuf::from("/project/app.slint"),
                        component: "App".into(),
                        devices: Vec::new(),
                        active_device: None,
                        last_used_device: Some(device_id.clone()),
                    },
                },
            )),
        );

        assert_eq!(springboard_snapshot_last_used(&event), Some(Some(device_id)));
    }

    fn project_entry_with_components(
        directory: &tempfile::TempDir,
        components: &[&str],
    ) -> ProjectEntryComponents {
        let entry_file = directory.path().join("main.slint");
        std::fs::write(&entry_file, "export component App inherits Window {}\n").unwrap();
        ProjectEntryComponents {
            project_root: std::fs::canonicalize(directory.path()).unwrap(),
            entry_file: std::fs::canonicalize(&entry_file).unwrap(),
            entry_url: Url::from_file_path(std::fs::canonicalize(entry_file).unwrap()).unwrap(),
            components: components.iter().map(|component| (*component).into()).collect(),
        }
    }

    #[test]
    fn single_export_creates_the_manifest() {
        let directory = tempfile::tempdir().unwrap();

        let action =
            finish_project_entry(project_entry_with_components(&directory, &["App"])).unwrap();

        assert_eq!(action, ProjectEntryAction::Configured);
        assert!(directory.path().join(editor_preview::project::PROJECT_MANIFEST_FILE).is_file());
    }

    #[test]
    fn multiple_exports_request_a_component_without_writing_the_manifest() {
        let directory = tempfile::tempdir().unwrap();

        let action =
            finish_project_entry(project_entry_with_components(&directory, &["App", "Demo"]))
                .unwrap();

        assert!(matches!(
            action,
            ProjectEntryAction::SelectComponent { components, .. }
                if components == ["App", "Demo"]
        ));
        assert!(!directory.path().join(editor_preview::project::PROJECT_MANIFEST_FILE).exists());
    }

    #[test]
    fn entry_without_exports_is_rejected() {
        let directory = tempfile::tempdir().unwrap();

        let error =
            finish_project_entry(project_entry_with_components(&directory, &[])).unwrap_err();

        assert!(error.to_string().contains("has no exported components"));
    }
}
