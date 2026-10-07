// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore nonsync

//! This is the live-preview part of the program.
//!
//! All functions defined in this file must be called in the UI thread! Different rules
//! may apply to the functions re-exported from the  `wasm` and `native` modules!
//! These functions integrate the preview with the surrounding environment which in
//! the case of `native` runs in a separate thread at this time.

use crate::preview::element_selection::ElementSelection;
use i_slint_compiler::parser::{TextSize, syntax_nodes};
use i_slint_compiler::source_path::SourcePath;
use i_slint_compiler::{EmbedResourcesKind, diagnostics};
use i_slint_core::DataTransfer;
use i_slint_core::component_factory::FactoryContext;
use i_slint_core::lengths::{
    LogicalPoint, LogicalRect, LogicalSize as CoreLogicalSize, LogicalVector,
};
use i_slint_editor_preview::{
    ElementRcNode,
    component_catalog::{self, ComponentInformation},
    editing::{rename_component, text_edit},
    util,
};
use i_slint_live_preview::protocol::{
    LspToPreviewMessage, PreviewComponent, PreviewConfig, PreviewToLspMessage, SourceFileVersion,
    VersionedUrl,
};
use lsp_types::Url;
use slint::{LogicalPosition, LogicalSize, PlatformError, SharedString, ToSharedString};
use slint_editor_mcp::{
    EditorComment as SnapshotComment, SnapshotPublisher, SourcePosition, SourceRange,
};
use slint_interpreter::{ComponentDefinition, ComponentHandle, ComponentInstance};
use smol_str::SmolStr;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;

mod drop_location;
mod element_catalog;
mod element_selection;
pub mod eval;
mod ext;
mod inspector;
#[cfg(target_os = "macos")]
pub mod macos_titlebar;
mod preview_data;
use ext::ElementRcNodeExt;
mod outline;
mod properties;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
pub mod remote;
pub(crate) mod settings;
#[cfg(feature = "system-testing")]
mod test_sync;
pub mod ui;
mod undo_redo;

use settings::{Project, SETTINGS_FILE, VisualEditorSettings};

pub fn initialize(
    editor_ui: &ui::EditorUi,
    to_lsp: Rc<dyn i_slint_editor_preview::PreviewToLsp>,
    settings: VisualEditorSettings,
) {
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        *preview_state.to_lsp.borrow_mut() = Some(to_lsp.clone());
        let api = editor_ui.global::<ui::Api>();
        preview_state.api = <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api);
        preview_state.editor_ui = Some(editor_ui.clone_strong());
        preview_state.settings = settings;
        api.set_element_comments(preview_state.element_comments_model.clone().into());
        api.set_comment_markers(preview_state.comment_markers_model.clone().into());
    });

    #[cfg(feature = "system-testing")]
    test_sync::initialize();
    let settings = PREVIEW_STATE.with_borrow(|preview_state| preview_state.settings.clone());
    editor_ui.set_elements_pane_height(
        settings.elements_pane_height.map_or(0.0, |height| height as f32),
    );
    editor_ui
        .set_outline_pane_height(settings.outline_pane_height.map_or(0.0, |height| height as f32));

    editor_ui
        .set_left_pane_width_preference(settings.left_pane_width.map_or(0.0, |width| width as f32));
    editor_ui.set_inspector_pane_width_preference(
        settings.inspector_pane_width.map_or(0.0, |width| width as f32),
    );
    let resize_pane = |pane| {
        let to_lsp = to_lsp.clone();
        move |size| update_pane_size(pane, Some(size), &to_lsp)
    };
    let reset_pane = |pane| {
        let to_lsp = to_lsp.clone();
        move || update_pane_size(pane, None, &to_lsp)
    };
    editor_ui.on_left_pane_resized(resize_pane(PaneSize::LeftWidth));
    editor_ui.on_inspector_pane_resized(resize_pane(PaneSize::InspectorWidth));
    editor_ui.on_elements_pane_resized(resize_pane(PaneSize::ElementsHeight));
    editor_ui.on_outline_pane_resized(resize_pane(PaneSize::OutlineHeight));
    editor_ui.on_left_pane_reset(reset_pane(PaneSize::LeftWidth));
    editor_ui.on_inspector_pane_reset(reset_pane(PaneSize::InspectorWidth));
    editor_ui.on_elements_pane_reset(reset_pane(PaneSize::ElementsHeight));
    editor_ui.on_outline_pane_reset(reset_pane(PaneSize::OutlineHeight));

    to_lsp
        .send_telemetry(&mut [(
            "type".to_string(),
            serde_json::to_value("preview_opened").unwrap(),
        )])
        .ok();

    tracing::debug!("Preview: requesting state from LSP");
    to_lsp
        .send(&PreviewToLspMessage::RequestState { files: Vec::new(), settings: Vec::new() })
        .unwrap();
}

/// Apply a message from the LSP to the preview. Whatever transport carried the
/// message calls this on the UI thread.
pub fn lsp_to_preview(message: LspToPreviewMessage) {
    use LspToPreviewMessage as M;
    match message {
        M::InvalidateContents { url } => invalidate_contents(&url),
        M::ForgetFile { url } => delete_document(&url),
        M::SetContents { url, contents } => {
            if let Ok(contents) = String::from_utf8(contents) {
                set_contents(&url, contents);
            }
        }
        M::SetConfiguration { config } => {
            config_changed(config);
        }
        M::SetUserSettings { name, contents } => {
            set_user_settings(name, contents);
        }
        M::ShowPreview(preview_component) => {
            tracing::debug!(
                "Preview: opening url={}, component={:?}",
                preview_component.url,
                preview_component.component
            );
            PREVIEW_STATE.with_borrow(|preview_state| {
                if let Some(editor_ui) = &preview_state.editor_ui {
                    editor_ui.global::<ui::Preview>().set_can_run(true);
                }
            });
            apply_preview_to_file_tree(&preview_component);
            load_preview(preview_component, LoadBehavior::BringWindowToFront);
        }
        M::OpenProject { root } => {
            reset_project_state(root.clone());
            apply_project_to_file_tree(&root);
            record_current_project();
        }
        M::HighlightFromEditor { url, offset } => {
            highlight(url, offset.into());
        }
        M::RemoteConnectionState { state, target, error } => {
            set_remote_connection_state(state, target, error);
        }
        M::Quit => {
            tracing::debug!("Preview: Quit requested");
            #[cfg(not(target_arch = "wasm32"))]
            let _ = slint::quit_event_loop();
        }
        M::Ping => {
            // Keepalive for the remote-preview WebSocket; local previews never see it.
        }
        // Part of the remote pairing handshake, which the LSP's WebSocket
        // connector completes before a session exists. A local preview is
        // never on the receiving end of one.
        M::PairingHello { .. } | M::PairingResponse { .. } => {
            tracing::warn!("Ignoring a pairing message addressed to a local preview");
        }
    }
}

thread_local! {
    static RESOURCE_URL_MAPPER: RefCell<Option<i_slint_compiler::ResourceUrlMapper>> =
        const { RefCell::new(None) };
}

/// Install the mapper the preview compiles resource URLs with. Only
/// applications know how to resolve them: SlintPad maps them through JS,
/// native previews read them from disk and need no mapper at all.
#[allow(dead_code)] // Only the wasm application maps resource URLs today.
pub fn set_resource_url_mapper(mapper: i_slint_compiler::ResourceUrlMapper) {
    RESOURCE_URL_MAPPER.set(Some(mapper));
}

pub fn resource_url_mapper() -> Option<i_slint_compiler::ResourceUrlMapper> {
    RESOURCE_URL_MAPPER.with_borrow(Clone::clone)
}

/// The state of the preview engine:
///
/// ```text
///                               ┌─────────────┐
///                            ┌──│ NeedsReload │◄─┐
///                            │  └─────────────┘  │
///                            ▼                   │
/// ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
/// │ Pending     │────►│ PreLoading  │────►│ Loading     │
/// └─────────────┘     └─────────────┘     └─────────────┘
///        ▲                                       │
///        │                                       │
///        └───────────────────────────────────────┘
/// ```
#[derive(Default, Copy, Clone, PartialEq, Eq, Debug)]
enum PreviewFutureState {
    /// The preview future is currently no running
    #[default]
    Pending,
    /// The preview future has been started, but we haven't started compiling
    PreLoading,
    /// The preview future is currently loading the preview
    Loading,
    /// The preview future is currently loading an outdated preview, we should abort loading and restart loading again
    NeedsReload,
}

#[derive(Clone, Debug)]
struct SourceCodeCacheEntry {
    // None when read from disk!
    version: SourceFileVersion,
    code: String,
}
type SourceCodeCache = HashMap<Url, SourceCodeCacheEntry>;

/// Property overrides the editor pushes into the previewed instance, keyed by debug hook id.
type DebugHookOverrides = Rc<
    RefCell<HashMap<SmolStr, Pin<Box<i_slint_core::Property<Option<slint_interpreter::Value>>>>>>,
>;

/// Routes `instance`'s debug hooks through `overrides`, so that the editor can override a
/// property of the previewed element without going through the source.
fn install_debug_hook_callback(instance: &ComponentInstance, overrides: DebugHookOverrides) {
    instance.set_debug_hook_callback(Some(Box::new(move |id: &str| {
        let mut hooks = (*overrides).borrow_mut();
        let property = hooks.entry(SmolStr::from(id)).or_insert_with(|| {
            tracing::trace!("Inserting Property override: {id}");
            Box::pin(i_slint_core::Property::new(None))
        });
        property.as_ref().get()
    })));
}

#[derive(Default)]
struct EditorComments {
    publisher: Option<SnapshotPublisher>,
    project_root: PathBuf,
    comments: Vec<StoredEditorComment>,
    next_id: u64,
}

struct StoredEditorComment {
    selection: SourceElement,
    snapshot: SnapshotComment,
    unread: bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SourceElement {
    path: PathBuf,
    offset: TextSize,
}

impl SourceElement {
    fn from_selection(selection: &ElementSelection) -> Option<Self> {
        Some(Self {
            path: std::path::absolute(selection.path.as_native_path()?).ok()?,
            offset: selection.offset,
        })
    }
}

impl EditorComments {
    fn new(project_root: &Path) -> Self {
        let publisher = SnapshotPublisher::new()
            .map_err(|error| tracing::warn!("Failed to create comment snapshot: {error}"))
            .ok();
        let result = Self {
            publisher,
            project_root: project_root.to_path_buf(),
            comments: Vec::new(),
            next_id: 1,
        };
        result.publish();
        result
    }

    fn add(&mut self, selection: SourceElement, mut comment: SnapshotComment) {
        comment.id = self.next_id.to_string();
        self.next_id += 1;
        self.comments.push(StoredEditorComment { selection, snapshot: comment, unread: true });
        self.publish();
    }

    fn remove(&mut self, id: &str) {
        let old_length = self.comments.len();
        self.comments.retain(|comment| comment.snapshot.id != id);
        if self.comments.len() != old_length {
            self.publish();
        }
    }

    fn visible(&self, selection: &SourceElement) -> Vec<ui::EditorComment> {
        self.comments
            .iter()
            .filter(|comment| comment.selection == *selection)
            .map(|comment| ui::EditorComment {
                id: comment.snapshot.id.as_str().into(),
                text: comment.snapshot.text.as_str().into(),
            })
            .collect()
    }

    fn mark_read(&mut self, selection: &SourceElement) {
        for comment in &mut self.comments {
            if comment.selection == *selection {
                comment.unread = false;
            }
        }
    }

    fn markers(&self) -> Vec<ui::EditorCommentMarker> {
        let mut markers =
            std::collections::BTreeMap::<&SourceElement, ui::EditorCommentMarker>::new();
        for comment in &self.comments {
            let Ok(uri) = Url::from_file_path(&comment.selection.path) else { continue };
            let marker =
                markers.entry(&comment.selection).or_insert_with(|| ui::EditorCommentMarker {
                    source_uri: uri.as_str().into(),
                    offset: u32::from(comment.selection.offset) as i32,
                    label: comment
                        .snapshot
                        .element_id
                        .as_deref()
                        .unwrap_or(&comment.snapshot.element_type)
                        .into(),
                    ..Default::default()
                });
            marker.count += 1;
            marker.unread |= comment.unread;
        }
        markers.into_values().collect()
    }

    fn publish(&self) {
        let Some(publisher) = &self.publisher else { return };
        let comments =
            self.comments.iter().map(|comment| comment.snapshot.clone()).collect::<Vec<_>>();
        if let Err(error) = publisher.publish(&self.project_root, &comments) {
            tracing::warn!("Failed to publish comment snapshot: {error}");
        }
    }
}

#[derive(Default)]
pub struct PreviewState {
    pub editor_ui: Option<ui::EditorUi>,
    pub api: slint::Weak<ui::Api<'static>>,
    property_range_declarations: Option<ui::PropertyDeclarations>,
    /// The handle to the previewed component instance
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
    document_cache: Rc<RefCell<Option<Rc<i_slint_editor_preview::DocumentCache>>>>,
    debug_hook_overrides: DebugHookOverrides,
    selected: Option<element_selection::ElementSelection>,
    notify_editor_about_selection_after_update: bool,
    workspace_edit_sent: bool,
    known_components: Vec<ComponentInformation>,
    preview_loading_delay_timer: Option<slint::Timer>,
    initial_live_data: preview_data::PreviewDataMap,
    current_live_data: preview_data::PreviewDataMap,
    undo_redo_stack: undo_redo::UndoRedoStack,
    pending_history: std::collections::VecDeque<bool>,
    inspector_edit: Option<inspector::Edit>,
    fill_refresh: Option<inspector::FillRefresh>,

    source_code: SourceCodeCache,
    resources: HashSet<Url>,
    dependencies: HashSet<Url>,
    pub config: PreviewConfig,
    /// The most recent settings synced with the editor, used to suppress
    /// redundant updates when the UI re-reports settings we just applied.
    settings: VisualEditorSettings,
    current_previewed_component: Option<PreviewComponent>,
    current_project_root: Option<Url>,
    project_generation: u64,
    file_tree_controller: Option<ui::file_tree::SharedFileTreeController>,
    current_load_behavior: Option<LoadBehavior>,
    loading_state: PreviewFutureState,
    comments: EditorComments,
    element_comments_model: Rc<slint::VecModel<ui::EditorComment>>,
    comment_markers_model: Rc<slint::VecModel<ui::EditorCommentMarker>>,

    pub to_lsp: RefCell<Option<Rc<dyn i_slint_editor_preview::PreviewToLsp>>>,

    #[cfg(all(not(target_arch = "wasm32"), feature = "preview-remote"))]
    pub remote_discovery: Rc<remote::RemoteDiscovery>,
}

impl PreviewState {
    fn component_instance(&self) -> Option<ComponentInstance> {
        self.handle.borrow().as_ref().map(|ci| ci.clone_strong())
    }

    pub fn current_component(&self) -> Option<PreviewComponent> {
        self.current_previewed_component.clone()
    }

    pub fn set_current_component(&mut self, component: PreviewComponent) {
        self.current_previewed_component = Some(component);
    }

    pub fn rename_current_component(&mut self, url: &Url, old_name: &str, new_name: &str) {
        if let Some(pc) = &mut self.current_previewed_component
            && pc.url == *url
            && pc.component.as_deref() == Some(old_name)
        {
            pc.component = Some(new_name.to_string());
        }
    }

    pub fn format(&self) -> i_slint_editor_preview::ByteFormat {
        self.document_cache
            .borrow()
            .as_ref()
            .map_or(i_slint_editor_preview::ByteFormat::Utf8, |dc| dc.format)
    }
}

pub(in crate::preview) fn set_file_tree_controller(
    controller: ui::file_tree::SharedFileTreeController,
) {
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.file_tree_controller = Some(controller);
    });
}

fn file_edit_pending() -> bool {
    PREVIEW_STATE.with_borrow(undo_redo::edit_pending)
}

fn invalidate_file_history() {
    PREVIEW_STATE.with_borrow_mut(|state| {
        state.undo_redo_stack.clear();
        state.pending_history.clear();
        undo_redo::set_undo_redo_enabled(state);
    });
}
thread_local! {pub static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

fn invalidate_contents(url: &lsp_types::Url) {
    let needs_reload = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if let Some(cache_entry) = preview_state.source_code.get_mut(url) {
            // A source file was invalidated that is not currently open in the LSP.
            //
            // Just mark the cache as "read from disk" by setting the version to None.
            // Do not reset the code: We can check once the LSP has re-read it from disk
            // whether we need to refresh the preview or not.
            //
            // We should get an updated version of the file from the LSP when it recompiled, so
            // no reload needed at the moment.
            cache_entry.version = None;
        }
        // If a resource file was invalidated - we need to reload the preview
        //
        // This is a rather heavy-handed operation, but currently the best we can do.
        // Ideally, this should just reload that specific resource.
        preview_state.resources.contains(url)
    });

    if needs_reload {
        reload_preview();
    }
}

fn delete_document(url: &lsp_types::Url) {
    let (current, url_is_used) = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.source_code.remove(url);
        (
            preview_state.current_previewed_component.clone(),
            preview_state.dependencies.contains(url),
        )
    });

    if let Some(current) = current
        && (&current.url == url || url_is_used)
    {
        // Trigger a compile error now!
        load_preview(current, LoadBehavior::Reload);
    }
}

fn reset_project_state(root: Url) {
    let (api, editor_ui) = PREVIEW_STATE.with_borrow_mut(|state| {
        tracing::debug!(
            %root,
            current_component = ?state.current_previewed_component,
            loading_state = ?state.loading_state,
            load_behavior = ?state.current_load_behavior,
            "Preview: resetting project state"
        );
        state.property_range_declarations = None;
        state.handle.replace(None);
        state.document_cache.replace(None);
        (*state.debug_hook_overrides).borrow_mut().clear();
        state.selected = None;
        state.notify_editor_about_selection_after_update = false;
        state.workspace_edit_sent = false;
        state.known_components.clear();
        state.initial_live_data.clear();
        state.current_live_data.clear();
        state.undo_redo_stack.clear();
        state.pending_history.clear();
        state.inspector_edit = None;
        state.fill_refresh = None;
        state.source_code.clear();
        state.resources.clear();
        state.dependencies.clear();
        state.preview_loading_delay_timer = None;
        state.current_load_behavior = None;
        state.loading_state = PreviewFutureState::Pending;
        state.current_previewed_component = None;
        state.comments = root
            .to_file_path()
            .map(|project_root| EditorComments::new(&project_root))
            .unwrap_or_default();
        state.element_comments_model.set_vec(Vec::new());
        state.comment_markers_model.set_vec(Vec::new());
        state.current_project_root = Some(root);
        state.project_generation = state.project_generation.wrapping_add(1);
        (state.api.upgrade(), state.editor_ui.as_ref().map(|editor_ui| editor_ui.clone_strong()))
    });

    if let Some(api) = api {
        api.set_status_text(Default::default());
        api.set_current_element(Default::default());
        api.set_properties(Default::default());
        api.set_selection(ui::Selection { highlight_index: -1, ..Default::default() });
        api.set_undo_enabled(false);
        api.set_redo_enabled(false);
        api.set_inspector_fill_refresh_pending(false);
        ui::ui_set_known_components(&api, &[], usize::MAX);
        ui::ui_set_preview_data(&api, Default::default(), None);
        outline::reset_outline(&api, None);
    }
    if let Some(editor_ui) = editor_ui {
        editor_ui.global::<ui::Preview>().set_can_run(false);
    }
    inspector::invalidate_fill();
}

fn is_current_project_generation(project_generation: u64) -> bool {
    PREVIEW_STATE.with_borrow(|state| state.project_generation == project_generation)
}

pub fn set_user_settings(name: String, contents: String) {
    if name.as_str() == SETTINGS_FILE {
        let Some(settings) = VisualEditorSettings::deserialize(&contents) else {
            return;
        };
        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            if let Some(editor_ui) = preview_state.editor_ui.as_ref() {
                apply_visible_recent_projects(editor_ui, &settings);
                editor_ui.set_inspector_pane_width_preference(
                    settings.inspector_pane_width.map_or(0.0, |width| width as f32),
                );
                editor_ui.set_left_pane_width_preference(
                    settings.left_pane_width.map_or(0.0, |width| width as f32),
                );
                editor_ui.set_elements_pane_height(
                    settings.elements_pane_height.map_or(0.0, |height| height as f32),
                );
                editor_ui.set_outline_pane_height(
                    settings.outline_pane_height.map_or(0.0, |height| height as f32),
                );
            }
            preview_state.settings = settings;
        });
    }
}

#[derive(Copy, Clone)]
enum PaneSize {
    ElementsHeight,
    OutlineHeight,
    LeftWidth,
    InspectorWidth,
}

fn update_pane_size(
    pane: PaneSize,
    size: Option<f32>,
    to_lsp: &Rc<dyn i_slint_editor_preview::PreviewToLsp>,
) {
    let value = size.map(|size| size.round() as i32).filter(|size| *size > 0);
    let update = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        let target = match pane {
            PaneSize::ElementsHeight => &mut preview_state.settings.elements_pane_height,
            PaneSize::OutlineHeight => &mut preview_state.settings.outline_pane_height,
            PaneSize::InspectorWidth => &mut preview_state.settings.inspector_pane_width,
            PaneSize::LeftWidth => &mut preview_state.settings.left_pane_width,
        };
        let changed = *target != value;
        *target = value;
        if let Some(editor_ui) = preview_state.editor_ui.as_ref() {
            let size = value.map_or(0.0, |size| size as f32);
            match pane {
                PaneSize::ElementsHeight => editor_ui.set_elements_pane_height(size),
                PaneSize::OutlineHeight => editor_ui.set_outline_pane_height(size),
                PaneSize::InspectorWidth => editor_ui.set_inspector_pane_width_preference(size),
                PaneSize::LeftWidth => editor_ui.set_left_pane_width_preference(size),
            }
        }
        changed.then(|| preview_state.settings.serialize())
    });
    let Some(contents) = update else { return };
    if let Err(error) = to_lsp
        .send(&PreviewToLspMessage::UpdateUserSettings { name: SETTINGS_FILE.into(), contents })
    {
        tracing::warn!("Failed to save visual editor pane settings: {error}");
    }
}

pub(crate) fn apply_visible_recent_projects(
    editor_ui: &ui::EditorUi,
    settings: &VisualEditorSettings,
) {
    let project = editor_ui.global::<ui::Project>();
    project.set_recent(Rc::new(slint::VecModel::from(settings.visible_recent_projects())).into());
}

fn record_current_project() {
    let Some((root, component)) = PREVIEW_STATE.with_borrow(|preview_state| {
        Some((
            preview_state.current_project_root.clone()?,
            preview_state.current_previewed_component.clone()?,
        ))
    }) else {
        return;
    };
    let Ok(root) = root.to_file_path() else { return };
    let Ok(root) = std::fs::canonicalize(root) else { return };
    if !root.is_dir() {
        return;
    }
    let Ok(path) = component.url.to_file_path() else { return };
    let Ok(path) = std::fs::canonicalize(path) else { return };
    if !path.is_file() || !path.starts_with(&root) {
        return;
    }
    let Some(component_name) = component.component else { return };
    let Ok(url) = Url::from_file_path(path) else { return };
    let project =
        Project { root, preview: PreviewComponent { url, component: Some(component_name) } };
    let update = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if !preview_state.settings.add_recent_project(project) {
            return None;
        }
        if let Some(editor_ui) = preview_state.editor_ui.as_ref() {
            apply_visible_recent_projects(editor_ui, &preview_state.settings);
        }
        Some(preview_state.settings.serialize())
    });
    let Some(contents) = update else { return };
    PREVIEW_STATE.with_borrow(|preview_state| {
        if let Some(to_lsp) = preview_state.to_lsp.borrow().as_ref()
            && let Err(error) = to_lsp.send(&PreviewToLspMessage::UpdateUserSettings {
                name: SETTINGS_FILE.into(),
                contents,
            })
        {
            tracing::warn!("Failed to send visual editor settings update: {error}");
        }
    });
}

fn set_current_live_data(mut result: preview_data::PreviewDataMap) {
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.current_live_data.append(&mut result);
    })
}

fn apply_live_preview_data() {
    let Some(instance) = component_instance() else {
        return;
    };

    let new_initial_data = preview_data::query_preview_data_properties_and_callbacks(&instance);

    let (mut previous_initial, mut previous_current) =
        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            (
                std::mem::replace(&mut preview_state.initial_live_data, new_initial_data),
                std::mem::take(&mut preview_state.current_live_data),
            )
        });

    while let Some((kc, vc)) = previous_current.pop_last() {
        let prev = previous_initial.pop_last();

        let vc = vc.value.unwrap_or_default();

        if matches!(vc, slint_interpreter::Value::Void) {
            continue;
        }

        if let Some((ki, vi)) = prev {
            let vi = vi.value.unwrap_or_default();

            if ki == kc && vi == vc {
                continue;
            }
        }

        let _ = preview_data::set_preview_data(&instance, &kc.container, &kc.property_name, vc);
    }
}

fn set_contents(url: &VersionedUrl, content: String) {
    let own_fill_edit = inspector::fill_contents_changed(url.url(), &content);
    let (reload, invalidate) = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if !own_fill_edit
            && !preview_state.undo_redo_stack.check_set_contents_valid(url.url(), &content)
        {
            undo_redo::set_undo_redo_enabled(preview_state);
        }
        let old = preview_state.source_code.insert(
            url.url().clone(),
            SourceCodeCacheEntry { version: *url.version(), code: content.clone() },
        );
        let changed = old.as_ref().is_none_or(|old| old.code != content);
        let version_changed = old.as_ref().is_none_or(|old| old.version != *url.version());
        let selected_document =
            preview_state.selected.as_ref().and_then(|selected| selected.path.to_url()).as_ref()
                == Some(url.url());
        let dependency = preview_state.dependencies.contains(url.url());
        let invalidate =
            (selected_document && (changed || version_changed)) || (dependency && changed);
        let reload = (dependency && changed).then(|| preview_state.current_component()).flatten();
        (reload, invalidate)
    });
    if invalidate {
        inspector::invalidate();
    }
    if let Some(current) = reload {
        load_preview(current, LoadBehavior::Reload);
    }
}

fn apply_project_to_file_tree(root: &Url) {
    PREVIEW_STATE.with_borrow(|preview_state| {
        let Some(editor_ui) = preview_state.editor_ui.as_ref() else { return };
        let Some(controller) = preview_state.file_tree_controller.as_ref() else { return };
        let api = editor_ui.global::<ui::Api>();
        let project = editor_ui.global::<ui::Project>();
        ui::file_tree::open_project(controller, root, &api, &project);
    });
}

fn apply_preview_to_file_tree(component: &PreviewComponent) {
    PREVIEW_STATE.with_borrow(|preview_state| {
        let Some(editor_ui) = preview_state.editor_ui.as_ref() else { return };
        let Some(controller) = preview_state.file_tree_controller.as_ref() else { return };
        let api = editor_ui.global::<ui::Api>();
        let project = editor_ui.global::<ui::Project>();
        ui::file_tree::open_preview(controller, component, &api, &project);
    });
}

fn preview_component(path: &Path, component: Option<String>) -> Option<PreviewComponent> {
    let Ok(path) = std::fs::canonicalize(path) else {
        return None;
    };
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("slint"))
    {
        return None;
    }
    let Ok(url) = Url::from_file_path(&path) else {
        return None;
    };
    Some(PreviewComponent { url, component })
}

pub(super) fn request_preview_path(path: &Path, component: Option<String>) -> bool {
    let Some(component) = preview_component(path, component) else { return false };
    request_preview(component)
}

fn request_preview(component: PreviewComponent) -> bool {
    let to_lsp = PREVIEW_STATE.with_borrow(|preview_state| preview_state.to_lsp.borrow().clone());
    let Some(to_lsp) = to_lsp else {
        return false;
    };
    let component_url = component.url.clone();
    if let Err(error) = to_lsp.send(&PreviewToLspMessage::RequestPreview { component }) {
        tracing::warn!("Failed to request preview for {component_url}: {error}");
        return false;
    }
    true
}

fn property_declaration_ranges(name: slint::SharedString) -> ui::PropertyDeclaration {
    let name = name.to_string();
    PREVIEW_STATE
        .with_borrow(|preview_state| {
            preview_state
                .property_range_declarations
                .as_ref()
                .and_then(|d| d.get(name.as_str()).cloned())
        })
        .unwrap_or_default()
}

fn add_new_component() {
    fn find_component_name() -> Option<String> {
        PREVIEW_STATE.with_borrow(|preview_state| {
            for i in 0..preview_state.known_components.len() {
                let name =
                    format!("MyComponent{}", if i == 0 { "".to_string() } else { i.to_string() });

                if preview_state
                    .known_components
                    .binary_search_by_key(&name.as_str(), |ci| ci.name.as_str())
                    .is_err()
                {
                    return Some(name);
                }
            }
            None
        })
    }

    let Some(document_cache) = document_cache() else {
        return;
    };

    let preview_component =
        PREVIEW_STATE.with_borrow(|preview_state| preview_state.current_component());

    let Some(preview_component) = preview_component else {
        return;
    };

    let Some(component_name) = find_component_name() else {
        return;
    };

    let Some(document) = document_cache.get_document(&preview_component.url) else {
        return;
    };

    let Some(document) = &document.node else {
        return;
    };

    if let Some((edit, drop_data)) =
        drop_location::add_new_component(&document_cache, &component_name, document)
    {
        element_selection::select_element_at_source_code_position(
            drop_data.path,
            drop_data.selection_offset,
            None,
            SelectionNotification::AfterUpdate,
        );

        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            preview_state.set_current_component(PreviewComponent {
                url: preview_component.url.clone(),
                component: Some(component_name.clone()),
            })
        });

        send_workspace_edit(format!("Add {component_name}"), edit, true);
    }
}

/// Find the identifier that belongs to a component of the given `name` in the `document`
fn find_component_identifiers(
    document: &syntax_nodes::Document,
    name: &str,
) -> Vec<syntax_nodes::DeclaredIdentifier> {
    let name = Some(i_slint_compiler::parser::normalize_identifier(name));

    let mut result = Vec::new();
    for el in document.ExportsList() {
        if let Some(component) = el.Component() {
            let identifier = component.DeclaredIdentifier();
            if i_slint_compiler::parser::identifier_text(&identifier) == name {
                result.push(identifier);
            }
        }
    }

    for component in document.Component() {
        let identifier = component.DeclaredIdentifier();
        if i_slint_compiler::parser::identifier_text(&identifier) == name {
            result.push(identifier);
        }
    }

    result.sort_by_key(|i| i.text_range().start());
    result
}

fn rename_component(
    old_name: slint::SharedString,
    old_url: slint::SharedString,
    new_name: slint::SharedString,
) {
    let old_name = old_name.to_string();
    let Ok(old_url) = lsp_types::Url::parse(old_url.as_ref()) else {
        return;
    };
    let new_name = new_name.to_string();

    let Some(document_cache) = document_cache() else {
        return;
    };
    let Some(document) = document_cache.get_document(&old_url) else {
        return;
    };
    let Some(document) = document.node.as_ref() else {
        return;
    };

    let identifiers = find_component_identifiers(document, &old_name);
    if identifiers.is_empty() {
        return;
    };

    if let Ok(edit) = rename_component::find_declaration_node(
        &document_cache,
        &identifiers
            .first()
            .unwrap()
            .child_token(i_slint_compiler::parser::SyntaxKind::Identifier)
            .unwrap(),
    )
    .unwrap()
    .rename(&document_cache, &new_name)
    {
        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            preview_state.rename_current_component(&old_url, &old_name, &new_name);

            if let Some(current) = &mut preview_state.current_component()
                && current.url == old_url
                && let Some(component) = &current.component
                && component == &old_name
            {
                current.component = Some(new_name.clone());
            }
        });
        // Update which component to show after refresh from the editor.

        send_workspace_edit(format!("Rename component {old_name} to {new_name}"), edit, true);
    }
}

fn evaluate_bindings(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    bindings: impl IntoIterator<Item = ui::CodeBinding>,
) -> Option<lsp_types::WorkspaceEdit> {
    let element_url = Url::parse(element_url.as_ref()).ok()?;
    let element_version = if element_version < 0 { None } else { Some(element_version) };
    let element_offset = u32::try_from(element_offset).ok()?.into();
    let document_cache = document_cache()?;
    let changes = bindings
        .into_iter()
        .map(|binding| {
            i_slint_editor_preview::editing::PropertyChange::new(
                binding.name.as_str(),
                binding.value.to_string(),
            )
        })
        .collect();
    properties::update_element_properties(
        &document_cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(element_url, element_version),
            element_offset,
        ),
        changes,
    )
}

fn test_code_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: slint::SharedString,
) -> bool {
    let Some(edit) = evaluate_bindings(
        element_url,
        element_version,
        element_offset,
        [ui::CodeBinding { name: property_name, value: property_value }],
    ) else {
        return false;
    };

    let Some(document_cache) = document_cache() else {
        return false;
    };

    drop_location::workspace_edit_compiles(&document_cache, &edit) != CompilationResult::ChangeFails
}

fn set_code_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: slint::SharedString,
) -> bool {
    set_code_bindings(
        element_url,
        element_version,
        element_offset,
        [ui::CodeBinding { name: property_name, value: property_value }],
    )
}

fn set_code_bindings(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    bindings: impl IntoIterator<Item = ui::CodeBinding>,
) -> bool {
    let lsp = PREVIEW_STATE.with_borrow(|ps| ps.to_lsp.borrow().clone().unwrap());
    lsp.send_telemetry(&mut [(
        "type".to_string(),
        serde_json::to_value("property_changed").unwrap(),
    )])
    .ok();

    let Some(edit) = evaluate_bindings(element_url, element_version, element_offset, bindings)
    else {
        return false;
    };
    send_workspace_edit("Edit properties".to_string(), edit, true)
}

fn set_color_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    value: slint::Color,
) {
    // We need a CSS value which is rgba, color converts to a argb only :-/
    let rgba: slint::RgbaColor<u8> = value.into();
    let value: u32 = ((rgba.red as u32) << 24)
        + ((rgba.green as u32) << 16)
        + ((rgba.blue as u32) << 8)
        + (rgba.alpha as u32);

    let _ = set_code_binding(
        element_url,
        element_version,
        element_offset,
        property_name,
        format!("#{value:08x}").into(),
    );
}

fn set_element_id(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    new_id: slint::SharedString,
) {
    let Ok(element_url) = Url::parse(element_url.as_ref()) else { return };
    let element_version = if element_version < 0 { None } else { Some(element_version) };
    let element_offset = TextSize::from(element_offset as u32);

    let Some(document_cache) = document_cache() else { return };
    let Some(element) = document_cache.element_at_offset(&element_url, element_offset) else {
        return;
    };

    let Some(edits) = element.with_element_node(|node| {
        node.parent().and_then(syntax_nodes::SubElement::new).and_then(|node| {
            i_slint_editor_preview::editing::rename_element_id::rename_element_id(
                node,
                &new_id,
                document_cache.format,
            )
        })
    }) else {
        return;
    };
    send_workspace_edit(
        "Rename element".to_string(),
        i_slint_editor_preview::editing::create_workspace_edit(element_url, element_version, edits),
        true,
    );
}

fn show_component(name: slint::SharedString, url: slint::SharedString) {
    let name = name.to_string();
    let Ok(url) = Url::parse(url.as_ref()) else {
        return;
    };

    let Some(document_cache) = document_cache() else {
        return;
    };
    let Some(document) = document_cache.get_document(&url) else {
        return;
    };
    let Some(document) = document.node.as_ref() else {
        return;
    };

    let Some(identifier) = find_component_identifiers(document, &name).last().cloned() else {
        return;
    };

    let start = util::text_size_to_lsp_position(
        &identifier.source_file,
        identifier.text_range().start(),
        document_cache.format,
    );
    let lsp = PREVIEW_STATE.with_borrow(|ps| ps.to_lsp.borrow().clone().unwrap());
    lsp.ask_editor_to_show_document(url, lsp_types::Range::new(start, start), false).ok();
}

fn show_document_offset_range(url: slint::SharedString, start: i32, end: i32, take_focus: bool) {
    fn internal(
        url: slint::SharedString,
        start: i32,
        end: i32,
    ) -> Option<(Url, lsp_types::Position, lsp_types::Position)> {
        let url = Url::parse(url.as_ref()).ok()?;

        let start = u32::try_from(start).ok()?;
        let end = u32::try_from(end).ok()?;

        let document_cache = document_cache()?;
        let document = document_cache.get_document(&url)?;
        let document = document.node.as_ref()?;

        let start = util::text_size_to_lsp_position(
            &document.source_file,
            start.into(),
            document_cache.format,
        );
        let end = util::text_size_to_lsp_position(
            &document.source_file,
            end.into(),
            document_cache.format,
        );

        Some((url, start, end))
    }

    if let Some((f, s, e)) = internal(url, start, end) {
        let lsp = PREVIEW_STATE.with_borrow(|ps| ps.to_lsp.borrow().clone().unwrap());
        lsp.ask_editor_to_show_document(f, lsp_types::Range::new(s, e), take_focus).ok();
    }
}

fn show_preview_for(name: slint::SharedString, url: slint::SharedString) {
    let name = name.to_string();
    let Ok(url) = Url::parse(url.as_ref()) else {
        return;
    };

    request_preview(PreviewComponent { url, component: Some(name) });
}

/// An item in the preview UI being dragged.
#[derive(Clone, PartialEq, Eq, Debug, Hash)]
enum DragItem {
    /// An existing element instance to be moved.
    MoveElementInstance { uri: SharedString, offset: u32 },
    /// A new component from the palette to be instantiated.
    NewComponent { kind: ui::ElementKind },
}

fn new_component_data_for_kind(kind: ui::ElementKind) -> DataTransfer {
    if element_catalog::primitive(kind).is_none() {
        return Default::default();
    }
    DragItem::NewComponent { kind }.into()
}

/// Tried to convert a [`DataTransfer`] to a `DragItem`, but the data transfer's user data
/// was of the wrong type.
#[derive(Copy, Clone, PartialEq, Eq, Debug, Hash, Default)]
pub struct InvalidDataTransferForDragItem;

impl std::fmt::Display for InvalidDataTransferForDragItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "`DataTransfer` user data was not `DropCommand`")
    }
}

impl TryFrom<DataTransfer> for DragItem {
    type Error = InvalidDataTransferForDragItem;

    fn try_from(value: DataTransfer) -> Result<Self, Self::Error> {
        value
            .user_data()
            .and_then(|any| any.downcast::<Self>().ok().as_deref().cloned())
            .ok_or(InvalidDataTransferForDragItem)
    }
}

impl From<DragItem> for DataTransfer {
    fn from(value: DragItem) -> Self {
        let mut out = DataTransfer::default();
        out.set_user_data(Rc::new(value));
        out
    }
}

fn can_drop_component(data: DataTransfer, x: f32, y: f32, on_drop_area: bool) -> bool {
    let Ok(DragItem::NewComponent { kind }) = data.try_into() else {
        return false;
    };

    if !on_drop_area {
        set_drop_mark(&None);
        return false;
    }

    let Some(document_cache) = document_cache() else {
        return false;
    };

    let position = LogicalPoint::new(x, y);

    let Some(component) = palette_component(kind) else {
        return false;
    };

    drop_location::can_drop_at(&document_cache, position, &component)
}

fn palette_component(kind: ui::ElementKind) -> Option<ComponentInformation> {
    let primitive = element_catalog::primitive(kind)?;
    PREVIEW_STATE.with_borrow(|preview_state| {
        preview_state
            .known_components
            .iter()
            .find(|component| component.name == primitive.type_name && component.is_builtin)
            .cloned()
    })
}

fn drop_component(data: DataTransfer, x: f32, y: f32) {
    let Ok(DragItem::NewComponent { kind }) = data.try_into() else {
        return;
    };

    let Some(document_cache) = document_cache() else {
        return;
    };

    let position = LogicalPoint::new(x, y);

    let Some(component) = palette_component(kind) else {
        return;
    };

    let drop_result = drop_location::drop_at(&document_cache, position, &component)
        .map(|(e, d)| (e, d, component.name.clone()));

    if let Some((edit, drop_data, component_name)) = drop_result {
        element_selection::select_element_at_source_code_position(
            drop_data.path,
            drop_data.selection_offset,
            None,
            SelectionNotification::AfterUpdate,
        );

        send_workspace_edit(format!("Add element {component_name}"), edit, false);
    };
}

fn drop_component_with_geometry(
    data: DataTransfer,
    hit_position: LogicalPosition,
    position: LogicalPosition,
    size: LogicalSize,
) {
    let Ok(DragItem::NewComponent { kind }) = data.try_into() else {
        return;
    };

    let Some(document_cache) = document_cache() else {
        return;
    };

    let hit_position = LogicalPoint::new(hit_position.x, hit_position.y);
    let geometry = LogicalRect::new(
        LogicalPoint::new(position.x, position.y),
        CoreLogicalSize::new(size.width, size.height),
    );

    let Some(component) = palette_component(kind) else {
        return;
    };

    let drop_result =
        drop_location::drop_at_with_geometry(&document_cache, hit_position, &component, geometry)
            .map(|(edit, data)| (edit, data, component.name.clone()));

    if let Some((edit, drop_data, component_name)) = drop_result {
        element_selection::select_element_at_source_code_position(
            drop_data.path,
            drop_data.selection_offset,
            None,
            SelectionNotification::AfterUpdate,
        );

        send_workspace_edit(format!("Add element {component_name}"), edit, false);
    };
}

fn placeholder_node_text(selected: &i_slint_editor_preview::ElementRcNode) -> String {
    let Some(parent) = selected.parent() else {
        return Default::default();
    };

    if parent.layout_kind() != ui::LayoutKind::None && parent.children().len() == 1 {
        return format!("Rectangle {{ /* {} */ }}", i_slint_editor_preview::NODE_IGNORE_COMMENT);
    }

    Default::default()
}

fn delete_selected_element() {
    let Some(selected) = selected_element() else {
        return;
    };

    let Some(url) = selected.path.to_url() else {
        return;
    };

    let version = PREVIEW_STATE
        .with_borrow(|preview_state| preview_state.source_code.get(&url).and_then(|e| e.version));

    let Some(selected_node) = selected.as_element_node() else {
        return;
    };

    let Some(document_cache) = document_cache() else { return };

    let range =
        selected_node.with_decorated_node(|n| util::node_to_lsp_range(&n, document_cache.format));

    // Insert a placeholder node into layouts if those end up empty:
    let new_text = placeholder_node_text(&selected_node);

    let edit = i_slint_editor_preview::editing::create_workspace_edit(
        url,
        version,
        vec![lsp_types::TextEdit { range, new_text }],
    );

    send_workspace_edit("Delete element".to_string(), edit, true);
}

fn resize_selected_element(x: f32, y: f32, width: f32, height: f32) {
    let Some(element_selection) = &selected_element() else {
        return;
    };
    let Some(element_node) = element_selection.as_element_node() else {
        return;
    };

    let Some((edit, label)) = resize_selected_element_impl(
        &element_node,
        element_selection.instance_index,
        LogicalRect::new(LogicalPoint::new(x, y), CoreLogicalSize::new(width, height)),
    ) else {
        return;
    };

    send_workspace_edit(label, edit, true);
}

fn persist_selected_element_geometry() -> bool {
    let Some(element_selection) = &selected_element() else {
        return false;
    };
    let Some(element_node) = element_selection.as_element_node() else {
        return false;
    };

    let Some((edit, label)) = persist_selected_element_geometry_impl(&element_node) else {
        return false;
    };

    send_workspace_edit(label, edit, true)
}

fn rotate_selected_element(angle: f32) {
    let Some(element_selection) = &selected_element() else { return };
    let Some(element_node) = element_selection.as_element_node() else { return };

    let Some((edit, label)) =
        rotate_selected_element_impl(&element_node, element_selection.instance_index, angle)
    else {
        return;
    };

    send_workspace_edit(label, edit, true);
}

fn rotate_selected_element_impl(
    element_node: &ElementRcNode,
    instance_index: usize,
    angle: f32,
) -> Option<(lsp_types::WorkspaceEdit, String)> {
    let rotation = override_selected_element_rotation_impl(element_node, instance_index, angle)?;

    let (path, offset) = element_node.path_and_offset();
    let url = path.to_url()?;
    let document_cache = document_cache()?;

    let version = document_cache.document_version(&url);

    properties::update_element_properties(
        &document_cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            offset,
        ),
        vec![i_slint_editor_preview::editing::PropertyChange::new(
            "transform-rotation",
            format!("{rotation}deg"),
        )],
    )
    .map(|edit| (edit, "Rotating element".to_owned()))
}

fn override_selected_element_rotation(angle: f32) {
    let Some(element_selection) = &selected_element() else { return };
    let Some(element_node) = element_selection.as_element_node() else { return };
    let _ = override_selected_element_rotation_impl(
        &element_node,
        element_selection.instance_index,
        angle,
    );
}

fn override_element_text(
    override_id: slint::SharedString,
    text: slint::SharedString,
) -> slint::SharedString {
    let id = if override_id.is_empty() {
        let Some(element_selection) = selected_element() else { return Default::default() };
        let Some(element) = element_selection.as_element_node() else { return Default::default() };
        let hash = element.with_element_debug(|debug| debug.element_hash);
        i_slint_compiler::passes::property_id(hash, &SmolStr::from("text"))
    } else {
        SmolStr::from(override_id.as_str())
    };
    let overrides = PREVIEW_STATE.with_borrow(|state| state.debug_hook_overrides.clone());
    let mut overrides = (*overrides).borrow_mut();
    let text_override =
        overrides.entry(id.clone()).or_insert_with(|| Box::pin(i_slint_core::Property::new(None)));
    text_override.as_ref().set(Some(slint_interpreter::Value::String(text)));
    drop(overrides);
    if let Some(instance) = component_instance() {
        instance.window().request_redraw();
    }
    id.as_str().into()
}

/// Returns the applied parent-relative rotation in degrees, which the caller can commit to the
/// source, or `None` when the element has no rotation debug hook to override.
fn override_selected_element_rotation_impl(
    element_node: &ElementRcNode,
    instance_index: usize,
    angle: f32,
) -> Option<f64> {
    tracing::trace!("Setting rotation preview angle: {angle}");

    let component_instance = component_instance()?;

    let element_hash = element_node
        .element
        .borrow()
        .debug
        .get(element_node.debug_index)
        .map(|debug| debug.element_hash)
        .unwrap_or(0);
    if element_hash == 0 {
        tracing::debug!("Element does not have a hash, cannot override rotation");
        return None;
    }

    // `transform-rotation` is relative to the element's parent, so subtract the parent's
    // absolute rotation. Like `parent_origin` for the geometry override, the parent's rotation
    // rides along on the selected instance's geometry, so we don't have to guess which parent
    // instance the element belongs to.
    let Some(geometry) = element_node.geometries(&component_instance).get(instance_index).cloned()
    else {
        tracing::debug!("Selected element does not have geometry, refusing to override rotation");
        return None;
    };

    // Round to whole degrees, matching the value committed on release.
    let new_rotation = (angle - geometry.parent_rotation()).round() as f64;
    let current_rotation = (geometry.angle - geometry.parent_rotation()).round() as f64;
    if new_rotation == current_rotation {
        return Some(new_rotation);
    }

    PREVIEW_STATE.with_borrow(|preview_state| {
        let overrides = (*preview_state.debug_hook_overrides).borrow();
        let id = i_slint_compiler::passes::property_id(
            element_hash,
            &SmolStr::from("transform-rotation"),
        );
        let Some(rotation_override) = overrides.get(&id) else {
            tracing::debug!(
                "Element does not have a transform-rotation debug hook, cannot override rotation"
            );
            return None;
        };
        (**rotation_override).set(Some(slint_interpreter::Value::Number(new_rotation)));
        Some(())
    })?;

    component_instance.window().request_redraw();
    Some(new_rotation)
}

fn override_selected_element_geometry(x: f32, y: f32, width: f32, height: f32) {
    let Some(element_selection) = &selected_element() else { return };
    let Some(element_node) = element_selection.as_element_node() else { return };
    override_selected_element_geometry_impl(
        &element_node,
        element_selection.instance_index,
        x,
        y,
        width,
        height,
    );
}

/// Maps the selection frame's root-space rectangle back into the element's parent coordinate
/// system, where it can be written to the source `x`, `y`, `width` and `height`.
///
/// Returns `None` when the frame does not describe the element,
/// which is the case once a non-uniform scale and a rotation shear it into a parallelogram.
fn parent_relative_rect(
    geometry: &slint_interpreter::highlight::HighlightedRect,
    origin: LogicalPoint,
    size: CoreLogicalSize,
) -> Option<LogicalRect> {
    if !geometry.renders_as_rectangle {
        return None;
    }

    // `is_normal` rejects the degenerate frames that `inverse` still inverts.
    if !geometry.parent_transform.determinant().is_normal() {
        return None;
    }
    let to_parent = geometry.parent_transform.inverse()?;

    // The frame is measured in rendered pixels, the source in the parent's units.
    // `rect` is the rendered size, so its ratio to `local_rect` is everything that scales the
    // element, its own transform included.
    let scale_x = geometry.rect.width() / geometry.local_rect.width();
    let scale_y = geometry.rect.height() / geometry.local_rect.height();
    if !scale_x.is_normal() || !scale_y.is_normal() {
        return None;
    }
    let local_size = CoreLogicalSize::new(size.width / scale_x, size.height / scale_y);

    // Only the distance dragged has to cross into the parent's frame: wherever the element's
    // transform puts its rendered center, that center travels with the element.
    let dragged: LogicalVector =
        LogicalPoint::new(origin.x + size.width / 2., origin.y + size.height / 2.)
            - geometry.rect.center();
    let center = geometry.local_rect.center() + to_parent.transform_vector(dragged.cast()).cast();
    Some(LogicalRect::new(
        LogicalPoint::new(center.x - local_size.width / 2., center.y - local_size.height / 2.),
        local_size,
    ))
}

fn override_selected_element_geometry_impl(
    element_node: &ElementRcNode,
    instance_index: usize,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    tracing::trace!(
        "Setting geometry preview x: {}, y: {}, width: {}, height: {}",
        x,
        y,
        width,
        height
    );
    let Some(component_instance) = component_instance() else { return };

    let element_hash = element_node
        .element
        .borrow()
        .debug
        .get(element_node.debug_index)
        .map(|d| d.element_hash)
        .unwrap_or(0);
    if element_hash == 0 {
        tracing::debug!("Element does not have a hash, cannot override geometry");
        return;
    }

    // The parent frame rides along on the selected instance's geometry, so we don't have to
    // guess which parent instance contains the dragged point (which breaks once the element is
    // dragged outside of its parent).
    let Some(geometry) = element_node.geometries(&component_instance).get(instance_index).cloned()
    else {
        tracing::debug!("Selected element does not have geometry, refusing to override");
        return;
    };
    let Some(new_rect) = parent_relative_rect(
        &geometry,
        LogicalPoint::new(x, y),
        CoreLogicalSize::new(width, height),
    ) else {
        tracing::debug!("Refusing to override geometry of a sheared element");
        return;
    };
    let current_rect = geometry.local_rect;

    // Round to match the values written on release by resize_selected_element_impl,
    // so the element does not jump by a sub-pixel amount when the drag is committed.
    let values = [
        ("x", new_rect.origin.x.round() as f64, current_rect.origin.x.round() as f64),
        ("y", new_rect.origin.y.round() as f64, current_rect.origin.y.round() as f64),
        ("width", new_rect.width().round() as f64, current_rect.width().round() as f64),
        ("height", new_rect.height().round() as f64, current_rect.height().round() as f64),
    ];
    let values = values
        .into_iter()
        .filter_map(|(name, new_value, current_value)| {
            (new_value != current_value).then_some((name, new_value))
        })
        .collect::<Vec<_>>();

    PREVIEW_STATE.with_borrow(|preview_state| {
        let m = (*preview_state.debug_hook_overrides).borrow();
        for (name, value) in values {
            let id = i_slint_compiler::passes::property_id(element_hash, &SmolStr::from(name));
            let Some(property_override) = m.get(&id) else {
                tracing::debug!(
                    "Property debug hook {name} does not exist, cannot override geometry",
                );
                return;
            };
            (**property_override).set(Some(slint_interpreter::Value::Number(value)));
        }
    });

    component_instance.window().request_redraw();
}

fn override_selected_element_border_radius(
    corner: ui::CanvasCorner,
    length: f32,
    single_corner: bool,
) {
    if !single_corner {
        override_selected_element_border_radius(ui::CanvasCorner::TopLeft, length, true);
        override_selected_element_border_radius(ui::CanvasCorner::BottomRight, length, true);
        override_selected_element_border_radius(ui::CanvasCorner::TopRight, length, true);
        override_selected_element_border_radius(ui::CanvasCorner::BottomLeft, length, true);
    }

    let Some(element_selection) = &selected_element() else { return };
    let Some(element_node) = element_selection.as_element_node() else { return };
    let Some(component_instance) = component_instance() else { return };

    let element_hash = element_node
        .element
        .borrow()
        .debug
        .get(element_node.debug_index)
        .map(|d| d.element_hash)
        .unwrap_or(0);
    if element_hash == 0 {
        tracing::debug!("Element does not have a hash, cannot override geometry");
        return;
    }

    let property_name = match corner {
        ui::CanvasCorner::TopLeft => "border-top-left-radius",
        ui::CanvasCorner::TopRight => "border-top-right-radius",
        ui::CanvasCorner::BottomLeft => "border-bottom-left-radius",
        ui::CanvasCorner::BottomRight => "border-bottom-right-radius",
    };

    PREVIEW_STATE.with_borrow(|preview_state| {
        let m = (*preview_state.debug_hook_overrides).borrow();
        let id = i_slint_compiler::passes::property_id(element_hash, &SmolStr::from(property_name));
        let Some(property_override) = m.get(&id) else {
            tracing::debug!(
                "Property debug hook {property_name} does not exist, cannot override geometry",
            );
            return;
        };
        (**property_override).set(Some(slint_interpreter::Value::Number(length as f64)));
    });

    component_instance.window().request_redraw();
}

fn persist_selected_element_border_radius() {
    let Some(element_selection) = &selected_element() else {
        return;
    };
    let Some(element_node) = element_selection.as_element_node() else {
        return;
    };

    let element_hash = element_node
        .element
        .borrow()
        .debug
        .get(element_node.debug_index)
        .map(|d| d.element_hash)
        .unwrap_or(0);
    if element_hash == 0 {
        tracing::debug!("Element does not have a hash, cannot resize");
        return;
    }

    // They all have the same size anyway:
    let (path, offset) = element_node.path_and_offset();

    // Apply the overrides permanently
    let properties = [
        "border-top-left-radius",
        "border-top-right-radius",
        "border-bottom-left-radius",
        "border-bottom-right-radius",
    ];
    let geometry_changes = PREVIEW_STATE.with_borrow(|preview_state| {
        let overrides = (*preview_state.debug_hook_overrides).borrow();

        properties
            .into_iter()
            .filter_map(|property| {
                let id =
                    i_slint_compiler::passes::property_id(element_hash, &SmolStr::from(property));
                overrides
                    .get(&id)
                    .and_then(|property_override| property_override.as_ref().get())
                    .and_then(|value| {
                        if let slint_interpreter::Value::Number(value) = value && value.is_finite() {
                            Some((property, value))
                        } else {
                            tracing::debug!(
                                "Property override '{property}' is not a finite number, cannot reposition"
                            );
                            None
                        }
                    })
                    .map(|(property, value)| i_slint_editor_preview::editing::PropertyChange::new(
                        property,
                        format!("{}px", value)
                    ))
            })
            .collect::<Vec<_>>()
    });

    if geometry_changes.is_empty() {
        return;
    }

    let Some(url) = path.to_url() else {
        return;
    };
    let Some(document_cache) = document_cache() else {
        return;
    };

    let version = document_cache.document_version(&url);

    let Some((updates, _)) = properties::update_element_properties(
        &document_cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            offset,
        ),
        geometry_changes,
    )
    .map(|edit| (edit, "Changing border radius".to_owned())) else {
        return;
    };

    send_workspace_edit("Changing border radius".to_string(), updates, false);
}

fn resize_selected_element_impl(
    element_node: &ElementRcNode,
    instance_index: usize,
    rect: LogicalRect,
) -> Option<(lsp_types::WorkspaceEdit, String)> {
    // apply as override, then commit.
    override_selected_element_geometry_impl(
        element_node,
        instance_index,
        rect.origin.x,
        rect.origin.y,
        rect.width(),
        rect.height(),
    );

    persist_selected_element_geometry_impl(element_node)
}

fn persist_selected_element_geometry_impl(
    element_node: &ElementRcNode,
) -> Option<(lsp_types::WorkspaceEdit, String)> {
    let element_hash = element_node
        .element
        .borrow()
        .debug
        .get(element_node.debug_index)
        .map(|d| d.element_hash)
        .unwrap_or(0);
    if element_hash == 0 {
        tracing::debug!("Element does not have a hash, cannot persist geometry");
        return None;
    }

    let (path, offset) = element_node.path_and_offset();

    let properties = ["x", "y", "width", "height"];
    let geometry_changes = PREVIEW_STATE.with_borrow(|preview_state| {
        let overrides = (*preview_state.debug_hook_overrides).borrow();

        properties
            .into_iter()
            .filter_map(|property| {
                let id =
                    i_slint_compiler::passes::property_id(element_hash, &SmolStr::from(property));
                overrides
                    .get(&id)
                    .and_then(|property_override| property_override.as_ref().get())
                    .and_then(|value| {
                        if let slint_interpreter::Value::Number(value) = value && value.is_finite() {
                            Some((property, value))
                        } else {
                            tracing::debug!(
                                "Property override '{property}' is not a finite number, cannot reposition"
                            );
                            None
                        }
                    })
                    .map(|(property, value)| i_slint_editor_preview::editing::PropertyChange::new(
                        property,
                        format!("{}px", value)
                    ))
            })
            .collect::<Vec<_>>()
    });

    if geometry_changes.is_empty() {
        return None;
    }

    let url = path.to_url()?;
    let document_cache = document_cache()?;

    let version = document_cache.document_version(&url);

    properties::update_element_properties(
        &document_cache,
        i_slint_editor_preview::editing::VersionedPosition::new(
            VersionedUrl::new(url, version),
            offset,
        ),
        geometry_changes,
    )
    .map(|edit| (edit, "Repositioning element".to_owned()))
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum CompilationResult {
    ChangeCompiles,
    ChangeFails,
    NoChange,
}

pub(super) fn workspace_edit_finished(edit: lsp_types::WorkspaceEdit, applied: bool) {
    let _ =
        slint::invoke_from_event_loop(move || inspector::workspace_edit_finished(edit, applied));
}

fn send_workspace_edit(label: String, edit: lsp_types::WorkspaceEdit, test_edit: bool) -> bool {
    submit_workspace_edit(label, edit, test_edit, None)
}

fn submit_workspace_edit(
    label: String,
    edit: lsp_types::WorkspaceEdit,
    test_edit: bool,
    fill: Option<ui::FillData>,
) -> bool {
    let Some(document_cache) = document_cache() else {
        return false;
    };
    let Ok(result) = text_edit::apply_workspace_edit(&document_cache, &edit) else {
        return false;
    };
    let fill_refresh = if let Some(fill) = fill.clone() {
        let [expected] = result.as_slice() else { return false };
        let unchanged = PREVIEW_STATE.with_borrow(|state| {
            state
                .source_code
                .get(&expected.url)
                .is_some_and(|source| source.code == expected.contents)
        });
        if unchanged {
            inspector::cancel();
            return true;
        }
        Some((
            fill,
            text_edit::EditedText {
                url: expected.url.clone(),
                contents: expected.contents.clone(),
            },
        ))
    } else {
        None
    };
    let file_hashes = undo_redo::compute_file_hashes(&result);

    if test_edit {
        let test_result = drop_location::edited_text_compiles(&document_cache, result);
        match test_result {
            CompilationResult::ChangeCompiles => {}
            CompilationResult::ChangeFails => return false,
            CompilationResult::NoChange => return true,
        }
    }

    let reverse_edit = text_edit::reversed_edit(&document_cache, &edit);

    let accepted = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if undo_redo::edit_pending(preview_state) {
            return false;
        }
        if let Some((fill, expected)) = fill_refresh {
            let Some(reverse) = reverse_edit else { return false };
            preview_state.fill_refresh = Some(inspector::FillRefresh {
                expected,
                submitted_edit: edit.clone(),
                fill,
                undo: Some(undo_redo::EditItem {
                    title: label.clone(),
                    edit: reverse,
                    file_hashes,
                }),
            });
        } else {
            preview_state.undo_redo_stack.push(label.clone(), reverse_edit, file_hashes);
        }
        preview_state.workspace_edit_sent = true;
        undo_redo::set_undo_redo_enabled(preview_state);
        preview_state
            .to_lsp
            .borrow()
            .as_ref()
            .unwrap()
            .send(&PreviewToLspMessage::SendWorkspaceEdit { label: Some(label), edit })
            .unwrap();
        true
    });
    if accepted && fill.is_some() {
        let api = PREVIEW_STATE.with_borrow(|state| state.api.upgrade());
        if let Some(api) = api {
            api.set_inspector_fill_refresh_pending(true);
        }
    }
    accepted
}

fn change_style() {
    // The user picked a style in the ComboBox; remember it as the requested
    // style so the next build uses it.
    let style = get_current_style();
    let Some(current) = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.config.style = style;
        preview_state.current_component()
    }) else {
        return;
    };

    load_preview(current, LoadBehavior::Reload);
}

fn start_parsing() {
    set_status_text("Updating Preview...");
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if let Some(api) = preview_state.api.upgrade() {
            ui::set_diagnostics(&api, &[]);
        }
    });
}

fn extract_resources(
    dependencies: &HashSet<Url>,
    component_instance: &ComponentInstance,
) -> HashSet<Url> {
    let type_loader = component_instance.definition().type_loader();

    let mut result: HashSet<Url> = Default::default();

    for dependency in dependencies {
        let path = SourcePath::from_url(dependency);
        let Some(doc) = type_loader.get_document(&path) else {
            continue;
        };

        result.extend(
            doc.embedded_file_resources.borrow().iter().filter_map(|er| er.path.as_ref()?.to_url()),
        );
    }

    result
}

fn finish_parsing() {
    set_status_text("");
}

fn previewed_component_changed() {
    // TODO: Return early on !success (see previous finish_parsing implementation)
    let (Some(previewed_url), preview_component, source_code) =
        PREVIEW_STATE.with_borrow(|preview_state| {
            let pc = preview_state.current_component();
            (
                pc.as_ref().map(|pc| pc.url.clone()),
                pc.as_ref().and_then(|pc| pc.component.clone()),
                preview_state.source_code.clone(),
            )
        })
    else {
        return;
    };

    if let Some(document_cache) = document_cache() {
        let mut document_cache = document_cache.snapshot().unwrap();

        for (url, cache_entry) in &source_code {
            let mut diag = diagnostics::BuildDiagnostics::default();
            if document_cache.get_document(url).is_none() {
                i_slint_editor_preview::util::poll_once(document_cache.load_url(
                    url,
                    cache_entry.version,
                    cache_entry.code.clone(),
                    &mut diag,
                ));
            }
        }

        let uses_widgets = document_cache.uses_widgets(&previewed_url);

        let mut components = Vec::new();
        component_catalog::builtin_components(&document_cache, &mut components);
        component_catalog::all_exported_components(
            &document_cache,
            &mut |ci| !ci.is_global,
            &mut components,
        );

        for url in document_cache.all_urls().filter(|u| u.scheme() != "builtin") {
            component_catalog::file_local_components(&document_cache, &url, &mut components);
        }

        let index = if let Some(component) = preview_component.as_ref() {
            components
                .iter()
                .position(|ci| {
                    &ci.name == component
                        && ci.defined_at.as_ref().map(|da| da.url()) == Some(&previewed_url)
                })
                .unwrap_or(usize::MAX)
        } else {
            usize::MAX
        };

        apply_live_preview_data();

        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            preview_state.known_components = components;

            let document_cache = Rc::new(document_cache);
            preview_state.document_cache.borrow_mut().replace(Some(document_cache.clone()));

            let preview_data = preview_state
                .component_instance()
                .map(|component_instance| {
                    preview_data::query_preview_data_properties_and_callbacks(&component_instance)
                })
                .unwrap_or_default();

            if let Some(api) = preview_state.api.upgrade() {
                if let Some(editor_ui) = &preview_state.editor_ui {
                    let win = i_slint_core::window::WindowInner::from_pub(editor_ui.window())
                        .window_adapter();
                    let palettes =
                        ui::palette::collect_palette(&document_cache, &previewed_url, &win);
                    ui::palette::set_palette(&api, palettes);
                }
                ui::ui_set_uses_widgets(&api, uses_widgets);
                ui::ui_set_known_components(&api, &preview_state.known_components, index);
                let component = document_cache.get_document(&previewed_url).and_then(|doc| {
                    match preview_component.as_ref() {
                        Some(c_id) => doc.inner_components.iter().find(|c| c.id == c_id).cloned(),
                        None => doc.last_exported_component(),
                    }
                });
                outline::reset_outline(&api, component);
                ui::ui_set_preview_data(&api, preview_data, preview_component.clone());
            }
        });
    }

    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if let Some(component_instance) = preview_state.component_instance() {
            preview_state.resources =
                extract_resources(&preview_state.dependencies, &component_instance);
        } else {
            preview_state.resources.clear();
        }
    });
}

fn config_changed(config: PreviewConfig) {
    let Some((current, config)) = PREVIEW_STATE.with_borrow_mut(move |preview_state| {
        (preview_state.config != config).then(|| {
            preview_state.config = config.clone();

            (preview_state.current_component(), preview_state.config.clone())
        })
    }) else {
        return;
    };

    if let Some(hide_ui) = config.hide_ui {
        set_show_preview_ui(!hide_ui);
    }

    if let Some(current) = current {
        load_preview(current, LoadBehavior::Reload);
    }
}

/// If the file is in the cache, returns it.
///
/// If the file is not known, return a NotFound error:
/// Usually the LSP side will load the file and inform us about it soon.
/// Otherwise the file is indeed missing.
///
/// In any way, register it as a dependency
fn get_url_from_cache(url: &Url) -> std::io::Result<(SourceFileVersion, String)> {
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.dependencies.insert(url.to_owned());

        preview_state.source_code.get(url).map(|r| (r.version, r.code.clone())).ok_or(
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "File not registered in Live-Preview!",
            ),
        )
    })
}

fn get_path_from_cache(path: &SourcePath) -> std::io::Result<(SourceFileVersion, String)> {
    let url = path.to_url().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Failed to convert path to URL")
    })?;
    get_url_from_cache(&url)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadBehavior {
    /// We reload the preview, most likely because a file has changed
    Reload,
    /// Load the preview and make the window visible if it wasn't already.
    LoadWithoutLiveData,
    /// We show the preview because the user asked for it. The UI should become visible and focused if it wasn't already
    BringWindowToFront,
}

pub fn reload_preview() {
    let pc = PREVIEW_STATE
        .with_borrow(|preview_state| preview_state.current_previewed_component.clone());

    let Some(pc) = pc else {
        return;
    };

    load_preview(pc, LoadBehavior::LoadWithoutLiveData);
}

async fn reload_timer_function(project_generation: u64) {
    tracing::debug!("Preview: reload task started");
    if !is_current_project_generation(project_generation) {
        return;
    }
    let (selected, notify_editor) = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        let notify_editor = preview_state.notify_editor_about_selection_after_update;
        preview_state.notify_editor_about_selection_after_update = false;
        (preview_state.selected.take(), notify_editor)
    });

    loop {
        let Some((preview_component, config, behavior, project_generation)) = PREVIEW_STATE
            .with_borrow_mut(|preview_state| {
                tracing::debug!(
                    current_component = ?preview_state.current_previewed_component,
                    loading_state = ?preview_state.loading_state,
                    load_behavior = ?preview_state.current_load_behavior,
                    "Preview: taking queued reload"
                );
                let behavior = preview_state.current_load_behavior.take()?;
                let preview_component = preview_state.current_component()?;

                assert_eq!(preview_state.loading_state, PreviewFutureState::PreLoading);

                preview_state.loading_state = PreviewFutureState::Loading;
                preview_state.dependencies.clear();

                Some((
                    preview_component,
                    preview_state.config.clone(),
                    behavior,
                    preview_state.project_generation,
                ))
            })
        else {
            tracing::debug!("Preview: no queued component to reload");
            PREVIEW_STATE
                .with_borrow_mut(|state| state.loading_state = PreviewFutureState::Pending);
            return;
        };
        // An empty style lets the compiler apply its own default (and SLINT_STYLE);
        // the ComboBox is updated to the resolved style once the build finishes.
        let style = config.style.clone();

        match reload_preview_impl(preview_component, behavior, style, config, project_generation)
            .await
        {
            Ok(()) => {}
            Err(e) => {
                tracing::debug!("Preview reload failed: {}", e);
                PREVIEW_STATE.with_borrow_mut(|preview_state| {
                    preview_state.loading_state = PreviewFutureState::Pending;
                });
                tracing::error!("{e}");
                std::process::exit(3);
            }
        }

        if !is_current_project_generation(project_generation) {
            return;
        }

        match PREVIEW_STATE.with_borrow(|preview_state| preview_state.loading_state) {
            PreviewFutureState::Loading => {
                PREVIEW_STATE.with_borrow_mut(|preview_state| {
                    preview_state.loading_state = PreviewFutureState::Pending;
                });
                break;
            }
            PreviewFutureState::NeedsReload => {
                PREVIEW_STATE.with_borrow_mut(|preview_state| {
                    preview_state.loading_state = PreviewFutureState::PreLoading;
                });
                continue;
            }
            PreviewFutureState::Pending | PreviewFutureState::PreLoading => unreachable!(),
        };
    }

    if let Some(selection) = selected {
        element_selection::restore_selection(selection.clone(), SelectionNotification::Never);

        if notify_editor
            && selected_element().is_some()
            && let Some(component_instance) = component_instance()
            && let Some((element, debug_index)) = component_instance
                .element_node_at_source_code_position(&selection.path, selection.offset.into())
                .first()
        {
            let Some(element_node) = ElementRcNode::new(element.clone(), *debug_index) else {
                return;
            };
            let format = PREVIEW_STATE.with_borrow(|ps| ps.format());
            let (path, pos) = element_node.with_element_node(|node| {
                let sf = &node.source_file;
                (sf.path().to_url(), util::text_size_to_lsp_position(sf, selection.offset, format))
            });
            let lsp = PREVIEW_STATE.with_borrow(|ps| ps.to_lsp.borrow().clone().unwrap());
            if let Some(url) = path {
                lsp.ask_editor_to_show_document(url, lsp_types::Range::new(pos, pos), false).ok();
            }
        }
    }
    if notify_editor && selected_element().is_none() {
        let lsp = PREVIEW_STATE
            .with_borrow(|preview_state| preview_state.to_lsp.borrow().clone().unwrap());
        lsp.send(&PreviewToLspMessage::ClearHighlight).ok();
    }
}

pub fn load_preview(preview_component: PreviewComponent, behavior: LoadBehavior) {
    inspector::invalidate_fill();
    tracing::debug!(
        "Preview: load url={}, component={:?}, behavior={:?}",
        preview_component.url,
        preview_component.component,
        behavior
    );
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        match behavior {
            LoadBehavior::Reload => {}
            LoadBehavior::LoadWithoutLiveData | LoadBehavior::BringWindowToFront => {
                preview_state.set_current_component(preview_component)
            }
        }

        preview_state.current_load_behavior = Some(behavior);

        tracing::debug!(loading_state = ?preview_state.loading_state, "Preview: scheduling reload");

        match preview_state.loading_state {
            PreviewFutureState::Pending => {}
            PreviewFutureState::Loading => {
                preview_state.loading_state = PreviewFutureState::NeedsReload;
                return;
            }
            PreviewFutureState::NeedsReload | PreviewFutureState::PreLoading => {
                return;
            }
        }
        preview_state.loading_state = PreviewFutureState::PreLoading;
        let project_generation = preview_state.project_generation;

        preview_state
            .preview_loading_delay_timer
            .get_or_insert_with(|| {
                let timer = slint::Timer::default();
                timer.start(
                    slint::TimerMode::SingleShot,
                    i_slint_live_preview::REBUILD_DEBOUNCE,
                    move || {
                        tracing::debug!("Preview: reload timer fired");
                        if let Err(error) =
                            slint::spawn_local(reload_timer_function(project_generation))
                        {
                            tracing::error!(%error, "Preview: failed to spawn reload");
                        }
                    },
                );
                timer
            })
            .restart();
    });
}

async fn parse_source(
    config: PreviewConfig,
    path: SourcePath,
    version: SourceFileVersion,
    source_code: String,
    style: String,
    component: Option<String>,
    file_loader_fallback: impl Fn(
        SourcePath,
    ) -> core::pin::Pin<
        Box<
            dyn core::future::Future<Output = Option<std::io::Result<(SourceFileVersion, String)>>>,
        >,
    > + 'static,
) -> (
    Vec<diagnostics::Diagnostic>,
    Option<ComponentDefinition>,
    Option<i_slint_editor_preview::document_cache::OpenImportCallback>,
    Rc<RefCell<i_slint_editor_preview::document_cache::SourceFileVersionMap>>,
) {
    let mut builder = slint_interpreter::Compiler::default();

    let cc = builder.compiler_configuration(i_slint_core::InternalToken);
    cc.components_to_generate = if let Some(name) = component {
        i_slint_compiler::ComponentSelection::Named(name)
    } else {
        i_slint_compiler::ComponentSelection::LastExported
    };
    cc.resource_url_mapper = resource_url_mapper();
    cc.embed_resources = EmbedResourcesKind::ListAllResources;
    cc.no_native_menu = true;
    // Otherwise this may cause a runtime panic because of the recursion
    cc.error_on_binding_loop_with_window_layout = true;
    cc.is_preview = true;

    if !style.is_empty() {
        cc.style = Some(style);
    }
    cc.include_paths = config.include_paths;
    cc.library_paths = config.library_paths;
    cc.enable_experimental |= config.enable_experimental;
    cc.debug_hooks = Some(std::hash::RandomState::new());

    let (open_file_fallback, source_file_versions) =
        i_slint_editor_preview::document_cache::document_cache_parts_setup(
            cc,
            Some(Rc::new(file_loader_fallback)),
            i_slint_editor_preview::document_cache::SourceFileVersionMap::from([(
                path.clone(),
                version,
            )]),
        );

    tracing::debug!(%path, "Preview: compiling source");
    let result =
        builder.build_static_from_source(source_code, path, i_slint_core::InternalToken).await;

    let compiled = result.components().next();
    (result.diagnostics().collect(), compiled, open_file_fallback, source_file_versions)
}

// Must be inside the thread running the slint event loop
async fn reload_preview_impl(
    component: PreviewComponent,
    behavior: LoadBehavior,
    style: String,
    config: PreviewConfig,
    project_generation: u64,
) -> Result<(), PlatformError> {
    tracing::debug!(url = %component.url, "Preview: starting reload");
    start_parsing();

    if let Some(component_instance) = component_instance() {
        let live_preview_data = if behavior != LoadBehavior::LoadWithoutLiveData {
            preview_data::query_preview_data_properties_and_callbacks(&component_instance)
        } else {
            preview_data::PreviewDataMap::default()
        };
        set_current_live_data(live_preview_data);
    }

    let path = SourcePath::from_url(&component.url);
    let (version, source) = get_url_from_cache(&component.url).unwrap_or_else(|err| {
        tracing::debug!("Preview: Failed to load source for url={}, error={}", component.url, err);
        Default::default()
    });

    let format = if config.format_utf8 {
        i_slint_editor_preview::ByteFormat::Utf8
    } else {
        i_slint_editor_preview::ByteFormat::Utf16
    };

    let (diagnostics, compiled, open_import_callback, source_file_versions) = parse_source(
        config,
        path,
        version,
        source,
        style,
        component.component.clone(),
        move |path| {
            Box::pin(async move {
                // Always return Some to stop the compiler from trying to load itself...
                // All loading is done by the LSP for us!
                let result = get_path_from_cache(&path);
                tracing::debug!(%path, loaded = result.is_ok(), "Preview: loading import from cache");
                Some(result)
            })
        },
    )
    .await;

    if !is_current_project_generation(project_generation) {
        tracing::debug!("Discarding preview compiled for an inactive project");
        return Ok(());
    }

    let success = compiled.is_some();
    let loaded_component_name = compiled.as_ref().map(|c| c.name().to_string());

    // Reflect the style the compiler actually used (after resolving the default,
    // SLINT_STYLE, and "native") in the ComboBox.
    if let Some(compiled) = &compiled {
        set_current_style(compiled.type_loader().resolved_style.clone());
    }

    tracing::debug!(
        "Preview: compiled url={}, component={:?}, success={}, diagnostics={}",
        component.url,
        loaded_component_name,
        success,
        diagnostics.len()
    );

    let lsp = PREVIEW_STATE.with_borrow_mut(|preview_state| {
        if let Some(api) = preview_state.api.upgrade() {
            if api.get_auto_clear_console() {
                ui::log_messages::clear_log_messages_impl(&api);
            }
            ui::set_diagnostics(&api, &diagnostics);
        }
        preview_state.to_lsp.borrow().clone().unwrap()
    });
    let diags = convert_diagnostics(&diagnostics, &source_file_versions.borrow());
    lsp.notify_diagnostics(diags).unwrap();

    update_preview_area(compiled, behavior, open_import_callback, source_file_versions, format)?;

    if let Some(loaded_component_name) = loaded_component_name {
        let current_preview_loaded = PREVIEW_STATE.with_borrow_mut(|preview_state| {
            let Some(current) = preview_state.current_previewed_component.as_mut() else {
                return false;
            };
            if current != &component {
                return false;
            }
            current.component = Some(loaded_component_name);
            true
        });
        if current_preview_loaded {
            record_current_project();
        }
    }

    finish_parsing();
    Ok(())
}

/// This sets up the preview area to show the ComponentInstance
fn set_preview_factory(
    editor_ui: &ui::EditorUi,
    api: &ui::Api<'_>,
    compiled: ComponentDefinition,
    callback: Box<dyn Fn(ComponentInstance)>,
    behavior: LoadBehavior,
) {
    i_slint_core::window::WindowInner::from_pub(editor_ui.window()).close_all_popups();

    let _ = i_slint_core::window::WindowInner::from_pub(editor_ui.window())
        .context()
        .set_log_message_handler(Some(Box::new(|log_message| {
            let message = log_message.message_arguments().to_string();
            let location = log_message.location();
            PREVIEW_STATE.with_borrow_mut(|state| {
                let to_lsp = state.to_lsp.try_borrow();
                let Some(to_lsp) = to_lsp.ok() else { return };
                if let Some(to_lsp) = &*to_lsp {
                    to_lsp
                        .send(&PreviewToLspMessage::DebugMessage {
                            location: location.as_ref().map(|location| {
                                (
                                    std::path::PathBuf::from(location.path),
                                    location.line,
                                    location.column,
                                )
                            }),
                            message: message.clone(),
                        })
                        .ok();
                }
            });
            let location = location
                .as_ref()
                .map(|location| (location.path.to_shared_string(), location.line, location.column));
            let _ = slint::invoke_from_event_loop(move || {
                PREVIEW_STATE.with_borrow(|preview_state| {
                    if let Some(api) = preview_state.api.upgrade() {
                        ui::log_messages::append_log_message(
                            &api,
                            ui::LogMessageLevel::Debug,
                            location,
                            &message,
                        );
                    }
                });
            });
        })));

    let editor_ui_weak = editor_ui.as_weak();
    let factory = slint::ComponentFactory::new(move |ctx: FactoryContext| {
        let instance = compiled.create_embedded(ctx).unwrap();

        callback(instance.clone_strong());

        if let Some(editor_ui) = editor_ui_weak.upgrade() {
            let hover = editor_ui.global::<ui::Hover>();
            hover.set_preview_generation(hover.get_preview_generation() + 1);
        }

        Some(instance)
    });

    api.set_preview_area(factory);
    api.set_resize_to_preferred_size(behavior != LoadBehavior::Reload);
}

/// Push the remote connection's state to the Remote Preview pane, which
/// shows it and, while pairing, collects the code from the user.
pub fn set_remote_connection_state(
    state: i_slint_live_preview::protocol::RemoteConnectionState,
    target: String,
    error: Option<String>,
) {
    use i_slint_live_preview::protocol::RemoteConnectionState as R;
    PREVIEW_STATE.with_borrow(|preview_state| {
        let _ = preview_state.api.upgrade_in_event_loop(move |api| {
            let ui_state = match state {
                R::Disconnected => ui::RemoteConnectionState::Disconnected,
                R::Connecting => ui::RemoteConnectionState::Connecting,
                R::Reconnecting => ui::RemoteConnectionState::Reconnecting,
                R::PairingRequired => ui::RemoteConnectionState::PairingRequired,
                R::UnpairedWarning => ui::RemoteConnectionState::UnpairedWarning,
                R::Connected => ui::RemoteConnectionState::Connected,
                R::Failed => ui::RemoteConnectionState::Failed,
            };
            api.set_remote_connection_state(ui_state);
            api.set_remote_connection_target(target.into());
            api.set_remote_connection_error(error.unwrap_or_default().into());
        });
    });
}

pub fn highlight(url: Option<Url>, offset: TextSize) {
    let Some(path) = url.as_ref().map(SourcePath::from_url) else {
        element_selection::unselect_element(SelectionNotification::Never);
        return;
    };

    let selected = selected_element();

    if let Some(selected) = &selected
        && selected.path == path
        && selected.offset == offset
    {
        return;
    }

    let contains_dependency = PREVIEW_STATE.with_borrow(|preview_state| {
        url.as_ref().is_none_or(|url| preview_state.dependencies.contains(url))
    });

    if contains_dependency {
        if Some((path.clone(), offset)) == selected.map(|s| (s.path, s.offset)) {
            // Already selected!
            return;
        }
        element_selection::select_element_at_source_code_position(
            path,
            offset,
            None,
            SelectionNotification::Never,
        );
    }
}

pub fn get_component_info(component_type: &str) -> Option<ComponentInformation> {
    PREVIEW_STATE.with_borrow(|preview_state| {
        let index = preview_state
            .known_components
            .binary_search_by(|ci| ci.name.as_str().cmp(component_type))
            .ok()?;
        preview_state.known_components.get(index).cloned()
    })
}

fn convert_diagnostics(
    diagnostics: &[slint_interpreter::Diagnostic],
    file_versions: &i_slint_editor_preview::document_cache::SourceFileVersionMap,
) -> HashMap<Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)> {
    let mut result: HashMap<Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)> =
        Default::default();

    // Pre-fill version info and an empty diagnostics to reset the state for the url
    for (path, version) in file_versions.iter() {
        result.extend(path.to_url().map(|url| (url, (*version, Vec::new()))));
    }

    PREVIEW_STATE.with_borrow(|preview_state| {
        for d in diagnostics {
            let Some(uri) = i_slint_live_preview::protocol::diagnostic_url(d) else {
                continue;
            };
            let new_version = preview_state.source_code.get(&uri).and_then(|e| e.version);
            if let Some(data) = result.get_mut(&uri) {
                if data.0.is_some() && new_version.is_some() && data.0 != new_version {
                    continue;
                }
                data.1.push(i_slint_live_preview::protocol::to_lsp_diagnostic(
                    d,
                    preview_state.format(),
                ));
            }
        }
    });

    result
}

fn set_drop_mark(mark: &Option<drop_location::DropMark>) {
    PREVIEW_STATE.with_borrow(move |preview_state| {
        let Some(api) = preview_state.api.upgrade() else {
            return;
        };

        if let Some(m) = mark {
            api.set_drop_mark(ui::DropMark {
                x1: m.start.x,
                y1: m.start.y,
                x2: m.end.x,
                y2: m.end.y,
            });
        } else {
            api.set_drop_mark(ui::DropMark { x1: -1.0, y1: -1.0, x2: -1.0, y2: -1.0 });
        }
    })
}

#[derive(Debug, PartialEq)]
pub enum SelectionNotification {
    Never,
    Now,
    AfterUpdate,
}

fn set_selected_element(
    mut selection: Option<element_selection::ElementSelection>,
    editor_notification: SelectionNotification,
) {
    inspector::cancel();
    let (layout_kind, parent_layout_kind, type_name) = {
        let selection_node = selection.as_ref().and_then(|s| s.as_element_node());
        let (layout_kind, parent_layout_kind) = selection_node
            .as_ref()
            .map(|en| (en.layout_kind(), element_selection::parent_layout_kind(en)))
            .unwrap_or((ui::LayoutKind::None, ui::LayoutKind::None));
        let type_name = selection_node
            .and_then(|n| {
                // This is an approximation, I hope it is good enough. The ElementRc was lowered, so there is nothing to see there anymore
                n.with_element_node(|n| {
                    n.QualifiedName().map(|qn| qn.text().to_string().trim().to_string())
                })
            })
            .unwrap_or_default();

        (layout_kind, parent_layout_kind, type_name)
    };

    set_drop_mark(&None);

    let element_node = selection.as_ref().and_then(|s| s.as_element_node());
    let notify_editor_about_selection_after_update =
        editor_notification == SelectionNotification::AfterUpdate;

    let (lsp, format, selection_cleared) = PREVIEW_STATE.with_borrow_mut(move |preview_state| {
        let had_selection = preview_state.selected.is_some() || selection.is_some();
        let is_in_layout = parent_layout_kind != ui::LayoutKind::None;
        let is_layout = layout_kind != ui::LayoutKind::None;
        let is_interactive = {
            let index = preview_state
                .known_components
                .iter()
                .position(|ci| ci.name.as_str() == type_name.as_str());

            index
                .and_then(|idx| preview_state.known_components.get(idx))
                .map(|kc| kc.is_interactive)
                .unwrap_or_default()
        };

        if let Some(api) = preview_state.api.upgrade() {
            api.set_selection(ui::Selection {
                highlight_index: selection.as_ref().map(|s| s.instance_index as i32).unwrap_or(-1),
                layout_data: layout_kind,
                is_interactive,
                is_moveable: true,
                is_resizable: !is_in_layout && !is_layout,
            });

            if let Some(document_cache) = document_cache_from(preview_state)
                && let Some((uri, version, selection)) = selection.as_ref().and_then(|selection| {
                    let url = selection.path.to_url()?;
                    let version = document_cache.document_version(&url);
                    Some((
                        url.clone(),
                        version,
                        document_cache.element_at_offset(&url, selection.offset)?,
                    ))
                })
            {
                if let Some(editor_ui) = &preview_state.editor_ui {
                    let win = i_slint_core::window::WindowInner::from_pub(editor_ui.window())
                        .window_adapter();
                    let palettes = ui::palette::collect_palette(&document_cache, &uri, &win);
                    ui::palette::set_palette(&api, palettes);
                }

                let in_layout = match parent_layout_kind {
                    ui::LayoutKind::None => properties::LayoutKind::None,
                    ui::LayoutKind::Horizontal => properties::LayoutKind::HorizontalBox,
                    ui::LayoutKind::Vertical => properties::LayoutKind::VerticalBox,
                    ui::LayoutKind::Grid => properties::LayoutKind::GridLayout,
                };
                if let Some(editor_ui) = &preview_state.editor_ui {
                    preview_state.property_range_declarations = Some(ui::ui_set_properties(
                        &api,
                        editor_ui.window(),
                        &document_cache,
                        properties::query_properties(&uri, version, &selection, in_layout).ok(),
                    ));
                }
            } else if selection.is_none()
                || (!notify_editor_about_selection_after_update
                    && !preview_state.workspace_edit_sent)
            {
                api.set_current_element(Default::default());
                api.set_properties(Default::default());
                api.set_selection(ui::Selection { highlight_index: -1, ..Default::default() });
                preview_state.property_range_declarations = None;
                selection = None;
            }
        }

        let selection_cleared = had_selection && selection.is_none();
        preview_state.selected = selection;
        preview_state.notify_editor_about_selection_after_update =
            notify_editor_about_selection_after_update;
        set_visible_element_comments(preview_state, preview_state.selected.as_ref());

        (preview_state.to_lsp.borrow().clone().unwrap(), preview_state.format(), selection_cleared)
    });

    if editor_notification == SelectionNotification::Now && selection_cleared {
        lsp.send(&PreviewToLspMessage::ClearHighlight).ok();
    } else if editor_notification == SelectionNotification::Now
        && let Some(element_node) = element_node
    {
        let (path, pos) = element_node.with_element_node(|node| {
            let sf = &node.source_file;
            (
                sf.path().to_url(),
                util::text_size_to_lsp_position(sf, node.text_range().start(), format),
            )
        });
        if let Some(url) = path {
            lsp.ask_editor_to_show_document(url, lsp_types::Range::new(pos, pos), false).ok();
        }
    }
}

fn add_element_comment(text: SharedString) {
    if text.is_empty() {
        return;
    }
    let Some(selection) = selected_element() else {
        return;
    };
    let Some(source_element) = SourceElement::from_selection(&selection) else { return };
    let Some(element_node) = selection.as_element_node() else { return };
    let Some(comment) = snapshot_comment(&element_node, text.to_string()) else { return };

    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.comments.add(source_element, comment);
        set_visible_element_comments(preview_state, Some(&selection));
    });
}

fn remove_element_comment(id: SharedString) {
    let selection = selected_element();
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        preview_state.comments.remove(id.as_str());
        set_visible_element_comments(preview_state, selection.as_ref());
    });
}

fn mark_element_comments_read() {
    PREVIEW_STATE.with_borrow_mut(|preview_state| {
        let Some(selection) =
            preview_state.selected.as_ref().and_then(SourceElement::from_selection)
        else {
            return;
        };
        preview_state.comments.mark_read(&selection);
        set_visible_element_comments(preview_state, preview_state.selected.as_ref());
    });
}

fn select_comment_element(source_uri: SharedString, offset: i32, instance_index: i32) {
    let Ok(uri) = Url::parse(source_uri.as_str()) else { return };
    let Ok(offset) = u32::try_from(offset) else { return };
    let Ok(instance_index) = usize::try_from(instance_index) else { return };
    element_selection::restore_selection(
        ElementSelection {
            path: SourcePath::from(uri),
            offset: TextSize::from(offset),
            instance_index,
        },
        SelectionNotification::Now,
    );
}

fn snapshot_comment(
    element_node: &i_slint_editor_preview::ElementRcNode,
    text: String,
) -> Option<SnapshotComment> {
    let (file, range) = comment_location(element_node)?;
    let (component, element_type, element_id) = element_node.with_element_node(|node| {
        let mut ancestor = node.parent();
        let mut component = None;
        while let Some(current) = ancestor {
            if let Some(component_node) = syntax_nodes::Component::new(current.clone()) {
                component =
                    i_slint_compiler::parser::identifier_text(&component_node.DeclaredIdentifier())
                        .map(|identifier| identifier.to_string());
                break;
            }
            ancestor = current.parent();
        }
        let element_type = node
            .QualifiedName()
            .map(|qualified_name| qualified_name.text().to_string().trim().to_string())
            .unwrap_or_default();
        let element_id = node
            .parent()
            .and_then(syntax_nodes::SubElement::new)
            .and_then(|sub_element| {
                sub_element.child_text(i_slint_compiler::parser::SyntaxKind::Identifier)
            })
            .map(|identifier| identifier.to_string());
        (component, element_type, element_id)
    });

    Some(SnapshotComment {
        id: String::new(),
        text,
        file,
        range,
        component,
        element_type,
        element_id,
    })
}

fn comment_location(
    element_node: &i_slint_editor_preview::ElementRcNode,
) -> Option<(PathBuf, SourceRange)> {
    element_node.with_element_node(|node| {
        let file = std::path::absolute(node.source_file.path().as_native_path()?).ok()?;
        let source_range = node.text_range();
        let start = util::text_size_to_lsp_position(
            &node.source_file,
            source_range.start(),
            i_slint_editor_preview::ByteFormat::Utf16,
        );
        let end = util::text_size_to_lsp_position(
            &node.source_file,
            source_range.end(),
            i_slint_editor_preview::ByteFormat::Utf16,
        );
        Some((
            file,
            SourceRange {
                start: SourcePosition { line: start.line, character: start.character },
                end: SourcePosition { line: end.line, character: end.character },
            },
        ))
    })
}

fn set_visible_element_comments(
    preview_state: &PreviewState,
    selection: Option<&ElementSelection>,
) {
    use slint::Model;

    let comments = selection
        .and_then(SourceElement::from_selection)
        .map(|selection| preview_state.comments.visible(&selection))
        .unwrap_or_default();
    if preview_state.element_comments_model.iter().ne(comments.iter().cloned()) {
        preview_state.element_comments_model.set_vec(comments);
    }
    let markers = preview_state.comments.markers();
    if preview_state.comment_markers_model.iter().ne(markers.iter().cloned()) {
        preview_state.comment_markers_model.set_vec(markers);
    }
}

fn selected_element() -> Option<ElementSelection> {
    PREVIEW_STATE.with_borrow(move |preview_state| preview_state.selected.clone())
}

fn component_instance() -> Option<ComponentInstance> {
    PREVIEW_STATE.with_borrow(move |preview_state| preview_state.component_instance())
}

/// This is a *read-only* snapshot of the raw type loader, use this when you
/// need to know the exact state the compiled resources were in.
fn document_cache() -> Option<Rc<i_slint_editor_preview::DocumentCache>> {
    PREVIEW_STATE.with_borrow(document_cache_from)
}

/// This is a *read-only* snapshot of the raw type loader, use this when you
/// need to know the exact state the compiled resources were in.
fn document_cache_from(
    preview_state: &PreviewState,
) -> Option<Rc<i_slint_editor_preview::DocumentCache>> {
    preview_state.document_cache.borrow().as_ref().map(|dc| dc.clone())
}

fn set_show_preview_ui(show_preview_ui: bool) {
    PREVIEW_STATE.with_borrow(|preview_state| {
        if let Some(api) = preview_state.api.upgrade() {
            api.set_show_preview_ui(show_preview_ui)
        }
    });
}

/// Selects `style` in the style ComboBox to reflect the style currently in use.
/// Leaves the selection untouched if the style is not in the list.
fn set_current_style(style: String) {
    PREVIEW_STATE.with_borrow(move |preview_state| {
        if let Some(api) = preview_state.api.upgrade() {
            use slint::Model;
            if let Some(index) = api.get_known_styles().iter().position(|s| s.as_str() == style) {
                api.set_current_style_index(index as i32);
            }
        }
    });
}

pub fn get_current_style() -> String {
    PREVIEW_STATE.with_borrow(|preview_state| -> String {
        if let Some(api) = preview_state.api.upgrade() {
            use slint::Model;
            let index = api.get_current_style_index();
            api.get_known_styles()
                .row_data(usize::try_from(index).unwrap_or(0))
                .map(|s| s.to_string())
                .unwrap_or_default()
        } else {
            String::new()
        }
    })
}

fn set_status_text(text: &str) {
    let text = text.to_string();

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with_borrow(|preview_state| {
            if let Some(api) = preview_state.api.upgrade() {
                api.set_status_text(text.into());
            }
        });
    })
    .unwrap();
}

/// This ensure that the preview window is visible and runs `set_preview_factory`
fn update_preview_area(
    compiled: Option<ComponentDefinition>,
    behavior: LoadBehavior,
    open_import_callback: Option<i_slint_editor_preview::document_cache::OpenImportCallback>,
    source_file_versions: Rc<RefCell<i_slint_editor_preview::document_cache::SourceFileVersionMap>>,
    format: i_slint_editor_preview::ByteFormat,
) -> Result<(), PlatformError> {
    let editor_ui = PREVIEW_STATE.with_borrow_mut(move |preview_state| {
        preview_state.workspace_edit_sent = false;

        let editor_ui = preview_state.editor_ui.as_ref().unwrap();
        let api = preview_state.api.upgrade().unwrap();
        let shared_handle = preview_state.handle.clone();
        let shared_document_cache = preview_state.document_cache.clone();
        let shared_overrides = preview_state.debug_hook_overrides.clone();

        if let Some(compiled) = compiled {
            api.set_focus_previewed_element(behavior == LoadBehavior::BringWindowToFront);
            // Keep the inspector mounted until reselection, so edits retain keyboard focus.

            set_preview_factory(
                editor_ui,
                &api,
                compiled,
                Box::new(move |instance| {
                    if let Some(rtl) = instance.definition().raw_type_loader() {
                        shared_document_cache.replace(Some(Rc::new(
                            i_slint_editor_preview::DocumentCache::new_from_raw_parts(
                                rtl,
                                open_import_callback.clone(),
                                source_file_versions.clone(),
                                format,
                            ),
                        )));
                    }

                    // element_hash (and thus hook ids) change on every recompile, so drop stale overrides.
                    (*shared_overrides).borrow_mut().clear();
                    install_debug_hook_callback(&instance, shared_overrides.clone());

                    shared_handle.replace(Some(instance));
                    previewed_component_changed();
                }),
                behavior,
            );
        }

        editor_ui.clone_strong()
    });

    editor_ui.show().and_then(|_| {
        if matches!(behavior, LoadBehavior::BringWindowToFront) {
            let window_inner = i_slint_core::window::WindowInner::from_pub(editor_ui.window());
            if let Some(window_adapter_internal) =
                window_inner.window_adapter().internal(i_slint_core::InternalToken)
            {
                window_adapter_internal.bring_to_front()?;
            }
        }

        Ok(())
    })?;

    inspector::invalidate();
    element_selection::reselect_element();
    undo_redo::apply_pending();
    Ok(())
}

#[cfg(test)]
pub mod test {
    use i_slint_compiler::source_path::SourcePath;
    use std::{collections::HashMap, rc::Rc};

    use slint_interpreter::ComponentInstance;

    use i_slint_editor_preview::test::main_test_file_name;

    #[track_caller]
    pub fn interpret_test_with_sources(
        style: &str,
        code: HashMap<SourcePath, String>,
    ) -> ComponentInstance {
        i_slint_backend_testing::init_no_event_loop();
        reinterpret_test_with_sources(style, code)
    }

    #[track_caller]
    pub fn reinterpret_test_with_sources(
        style: &str,
        code: HashMap<SourcePath, String>,
    ) -> ComponentInstance {
        let code = Rc::new(code);

        let path = main_test_file_name();
        let source_code = code.get(&path).unwrap().clone();
        let (diagnostics, component_definition, _, _) = spin_on::spin_on(super::parse_source(
            Default::default(),
            path,
            Some(24),
            source_code.to_string(),
            style.to_string(),
            None,
            move |path| {
                let code = code.clone();

                Box::pin(async move {
                    let Some(source) = code.get(&path) else {
                        return Some(Result::Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "path not found",
                        )));
                    };
                    Some(Ok((Some(24), source.clone())))
                })
            },
        ));

        assert!(diagnostics.is_empty());

        component_definition.unwrap().create().unwrap()
    }

    #[track_caller]
    pub fn interpret_test(style: &str, source_code: &str) -> ComponentInstance {
        let code = HashMap::from([(main_test_file_name(), source_code.to_string())]);
        interpret_test_with_sources(style, code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use i_slint_editor_preview::PreviewToLsp;
    use i_slint_editor_preview::test::main_test_file_name;
    use i_slint_live_preview::protocol::PreviewToLspMessage;
    use slint::Model;
    use std::fs;
    use std::path::PathBuf;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Default)]
    struct CapturePreviewToLsp {
        messages: Rc<RefCell<Vec<PreviewToLspMessage>>>,
    }

    impl PreviewToLsp for CapturePreviewToLsp {
        fn send(&self, message: &PreviewToLspMessage) -> i_slint_editor_preview::Result<()> {
            self.messages.as_ref().borrow_mut().push(message.clone());
            Ok(())
        }
    }

    fn reset_preview_state(messages: Rc<RefCell<Vec<PreviewToLspMessage>>>) {
        PREVIEW_STATE.with_borrow_mut(|state| {
            *state = PreviewState::default();
            state.to_lsp = RefCell::new(Some(Rc::new(CapturePreviewToLsp { messages })));
        });
    }

    #[test]
    fn unselect_respects_selection_notification() {
        for notification in [
            SelectionNotification::Never,
            SelectionNotification::Now,
            SelectionNotification::AfterUpdate,
        ] {
            let notify_now = notification == SelectionNotification::Now;
            let notify_after_update = notification == SelectionNotification::AfterUpdate;
            let messages = Rc::new(RefCell::new(Vec::new()));
            reset_preview_state(messages.clone());
            PREVIEW_STATE.with_borrow_mut(|state| {
                state.selected = Some(ElementSelection {
                    path: i_slint_editor_preview::test::main_test_file_name(),
                    offset: 0.into(),
                    instance_index: 0,
                });
            });

            element_selection::unselect_element(notification);

            PREVIEW_STATE.with_borrow(|state| {
                assert!(state.selected.is_none());
                assert_eq!(state.notify_editor_about_selection_after_update, notify_after_update);
            });
            if notify_now {
                assert!(matches!(
                    messages.borrow().as_slice(),
                    [PreviewToLspMessage::ClearHighlight]
                ));
            } else {
                assert!(messages.borrow().is_empty());
            }

            messages.as_ref().borrow_mut().clear();
            element_selection::reselect_element();
            element_selection::unselect_element(SelectionNotification::Now);
            assert!(messages.borrow().is_empty());
        }
    }

    #[test]
    fn missing_selection_respects_selection_notification() {
        i_slint_backend_testing::init_no_event_loop();
        let editor = ui::EditorUi::new().unwrap();
        let api = editor.global::<ui::Api>();
        for notification in [SelectionNotification::Never, SelectionNotification::Now] {
            let notify_now = notification == SelectionNotification::Now;
            let messages = Rc::new(RefCell::new(Vec::new()));
            reset_preview_state(messages.clone());
            PREVIEW_STATE.with_borrow_mut(|state| {
                state.api = <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api);
            });

            set_selected_element(
                Some(ElementSelection {
                    path: i_slint_editor_preview::test::main_test_file_name(),
                    offset: 0.into(),
                    instance_index: 0,
                }),
                notification,
            );

            assert!(PREVIEW_STATE.with_borrow(|state| state.selected.is_none()));
            if notify_now {
                assert!(matches!(
                    messages.borrow().as_slice(),
                    [PreviewToLspMessage::ClearHighlight]
                ));
            } else {
                assert!(messages.borrow().is_empty());
            }
        }
        reset_preview_state(Default::default());
    }

    #[test]
    fn opening_project_discards_previous_project_state() {
        reset_preview_state(Default::default());
        let old_url = Url::parse("file:///old/main.slint").unwrap();
        let new_root = Url::parse("file:///new/").unwrap();
        let old_project_generation = PREVIEW_STATE.with_borrow(|state| state.project_generation);
        let live_data_key = preview_data::PreviewDataKey {
            container: preview_data::PropertyContainer::Main,
            property_name: "value".into(),
        };
        let live_data = preview_data::PreviewData {
            ty: i_slint_compiler::langtype::Type::Int32,
            visibility: i_slint_compiler::object_tree::PropertyVisibility::Input,
            value: Some(slint_interpreter::Value::Number(42.0)),
        };

        PREVIEW_STATE.with_borrow_mut(|state| {
            state.source_code.insert(
                old_url.clone(),
                SourceCodeCacheEntry { version: Some(1), code: "export component Old {}".into() },
            );
            state.dependencies.insert(old_url.clone());
            state.resources.insert(old_url.clone());
            state.current_previewed_component =
                Some(PreviewComponent { url: old_url.clone(), component: Some("Old".into()) });
            state.initial_live_data.insert(live_data_key.clone(), live_data.clone());
            state.current_live_data.insert(live_data_key, live_data);
            state.undo_redo_stack.push(
                "Old project edit".into(),
                Some(Default::default()),
                undo_redo::compute_file_hashes(&[text_edit::EditedText {
                    url: old_url.clone(),
                    contents: "export component Old {}".into(),
                }]),
            );
        });

        lsp_to_preview(LspToPreviewMessage::OpenProject { root: new_root.clone() });

        let new_project_generation = PREVIEW_STATE.with_borrow_mut(|state| {
            assert_eq!(state.current_project_root, Some(new_root));
            assert_ne!(state.project_generation, old_project_generation);
            assert!(state.current_previewed_component.is_none());
            assert!(state.source_code.is_empty());
            assert!(state.dependencies.is_empty());
            assert!(state.resources.is_empty());
            assert!(state.initial_live_data.is_empty());
            assert!(state.current_live_data.is_empty());
            assert!(
                state
                    .undo_redo_stack
                    .check_set_contents_valid(&old_url, "export component Changed {}")
            );
            state.project_generation
        });
        assert!(!is_current_project_generation(old_project_generation));
        assert!(is_current_project_generation(new_project_generation));
    }

    #[test]
    fn opening_project_cancels_old_reload_work_and_allows_a_new_preview() {
        i_slint_backend_testing::init_no_event_loop();
        for loading_state in [
            PreviewFutureState::Pending,
            PreviewFutureState::PreLoading,
            PreviewFutureState::Loading,
            PreviewFutureState::NeedsReload,
        ] {
            reset_preview_state(Default::default());
            let timer_fired = Rc::new(std::cell::Cell::new(false));
            let timer_callback = timer_fired.clone();
            let old_project_generation = PREVIEW_STATE.with_borrow_mut(|state| {
                let timer = slint::Timer::default();
                timer.start(
                    slint::TimerMode::SingleShot,
                    std::time::Duration::from_millis(50),
                    move || {
                        timer_callback.set(true);
                    },
                );
                state.preview_loading_delay_timer = Some(timer);
                state.current_load_behavior = Some(LoadBehavior::BringWindowToFront);
                state.loading_state = loading_state;
                state.project_generation
            });

            reset_project_state(Url::parse("file:///new/").unwrap());
            PREVIEW_STATE.with_borrow(|state| {
                assert!(state.preview_loading_delay_timer.is_none());
                assert!(state.current_load_behavior.is_none());
                assert_eq!(state.loading_state, PreviewFutureState::Pending);
            });
            i_slint_backend_testing::mock_elapsed_time(100);
            assert!(!timer_fired.get());

            let component = PreviewComponent {
                url: Url::parse("file:///new/main.slint").unwrap(),
                component: None,
            };
            load_preview(component.clone(), LoadBehavior::BringWindowToFront);
            spin_on::spin_on(reload_timer_function(old_project_generation));
            PREVIEW_STATE.with_borrow(|state| {
                assert_eq!(state.current_component(), Some(component));
                assert_eq!(state.loading_state, PreviewFutureState::PreLoading);
                assert!(matches!(
                    state.current_load_behavior,
                    Some(LoadBehavior::BringWindowToFront)
                ));
                assert!(state.preview_loading_delay_timer.as_ref().unwrap().running());
            });
        }
        reset_preview_state(Default::default());
    }

    #[test]
    fn reload_without_a_component_returns_to_idle() {
        reset_preview_state(Default::default());
        let project_generation = PREVIEW_STATE.with_borrow_mut(|state| {
            state.loading_state = PreviewFutureState::PreLoading;
            state.current_load_behavior = Some(LoadBehavior::BringWindowToFront);
            state.project_generation
        });
        spin_on::spin_on(reload_timer_function(project_generation));
        assert_eq!(
            PREVIEW_STATE.with_borrow(|state| state.loading_state),
            PreviewFutureState::Pending
        );
    }

    #[test]
    fn opening_project_clears_comments_and_reassigns_the_snapshot() {
        reset_preview_state(Default::default());
        let old_project = tempfile::tempdir().unwrap();
        let new_project = tempfile::tempdir().unwrap();
        let range = SourceRange {
            start: SourcePosition { line: 1, character: 2 },
            end: SourcePosition { line: 3, character: 4 },
        };
        let old_file = old_project.path().join("main.slint");
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.comments = EditorComments::new(old_project.path());
            state.comments.add(
                SourceElement { path: old_file.clone(), offset: TextSize::from(12) },
                stored_comment(&old_file, range, "Old"),
            );
            state
                .element_comments_model
                .push(ui::EditorComment { id: "1".into(), text: "Old".into() });
        });

        reset_project_state(Url::from_directory_path(new_project.path()).unwrap());

        PREVIEW_STATE.with_borrow_mut(|state| {
            assert!(state.comments.comments.is_empty());
            assert_eq!(state.comments.project_root, new_project.path());
            assert_eq!(state.element_comments_model.row_count(), 0);
            let new_file = new_project.path().join("main.slint");
            state.comments.add(
                SourceElement { path: new_file.clone(), offset: TextSize::from(12) },
                stored_comment(&new_file, range, "New"),
            );
        });
        let projects = slint_editor_mcp::scan_projects().unwrap();
        assert!(!projects.iter().any(|project| project.project_root == old_project.path()));
        let published =
            projects.iter().find(|project| project.project_root == new_project.path()).unwrap();
        assert_eq!(published.comments.len(), 1);
        assert_eq!(published.comments[0].text, "New");
        reset_preview_state(Default::default());
    }

    #[test]
    fn property_edits_share_validation_and_telemetry() {
        const SOURCE: &str = r#"
export component Main inherits Rectangle {
    width: 30px;
    background: #000000;
}
"#;
        let path = i_slint_editor_preview::test::main_test_file_name();
        let url = path.to_url().unwrap();
        let mut document_cache = i_slint_editor_preview::test::empty_document_cache();
        let mut diagnostics = i_slint_compiler::diagnostics::BuildDiagnostics::default();
        spin_on::spin_on(document_cache.load_url(
            &url,
            Some(1),
            SOURCE.to_owned(),
            &mut diagnostics,
        ))
        .unwrap();
        assert!(!diagnostics.has_errors());

        let document_cache = Rc::new(document_cache);
        let offset = SOURCE.find("Rectangle {").unwrap() as i32;
        for kind in ["single", "batch", "color"] {
            for case in ["valid", "stale", "pending", "invalid"] {
                let messages = Rc::new(RefCell::new(Vec::new()));
                reset_preview_state(messages.clone());
                PREVIEW_STATE.with_borrow_mut(|state| {
                    state.document_cache.replace(Some(document_cache.clone()));
                    state.workspace_edit_sent = case == "pending";
                });
                let version = if case == "stale" { 0 } else { 1 };
                let name: SharedString =
                    if case == "invalid" { "unknown" } else { "background" }.into();
                assert_eq!(
                    test_code_binding(
                        url.as_str().into(),
                        version,
                        offset,
                        name.clone(),
                        "#12345678".into(),
                    ),
                    case == "valid" || case == "pending",
                );
                assert!(messages.borrow().is_empty());
                let accepted = match kind {
                    "single" => Some(set_code_binding(
                        url.as_str().into(),
                        version,
                        offset,
                        name,
                        "#12345678".into(),
                    )),
                    "batch" => Some(set_code_bindings(
                        url.as_str().into(),
                        version,
                        offset,
                        [
                            ui::CodeBinding { name, value: "#12345678".into() },
                            ui::CodeBinding { name: "width".into(), value: "40px".into() },
                        ],
                    )),
                    "color" => {
                        set_color_binding(
                            url.as_str().into(),
                            version,
                            offset,
                            name,
                            slint::Color::from_argb_u8(0x78, 0x12, 0x34, 0x56),
                        );
                        None
                    }
                    _ => unreachable!(),
                };
                if let Some(accepted) = accepted {
                    assert_eq!(accepted, case == "valid", "{kind}: {case}");
                }
                let messages = messages.borrow();
                assert_eq!(
                    messages
                        .iter()
                        .filter(|message| matches!(
                            message, PreviewToLspMessage::TelemetryEvent(event)
                                if event.get("type") == Some(&serde_json::json!("property_changed"))
                        ))
                        .count(),
                    1
                );
                let edits: Vec<_> = messages
                    .iter()
                    .filter_map(|message| {
                        if let PreviewToLspMessage::SendWorkspaceEdit { edit, .. } = message {
                            Some(edit)
                        } else {
                            None
                        }
                    })
                    .collect();
                assert_eq!(edits.len(), usize::from(case == "valid"), "{kind}: {case}");
                if let Some(edit) = edits.first() {
                    let applied = text_edit::apply_workspace_edit(&document_cache, edit).unwrap();
                    let mut expected = SOURCE.replace("#000000", "#12345678");
                    if kind == "batch" {
                        expected = expected.replace("30px", "40px");
                    }
                    assert_eq!(applied[0].contents, expected);
                }
            }
        }
        reset_preview_state(Default::default());
    }

    #[test]
    fn source_push_only_invalidates_relevant_inspector_edits() {
        i_slint_backend_testing::init_no_event_loop();
        reset_preview_state(Default::default());
        let editor = ui::EditorUi::new().unwrap();
        let api = editor.global::<ui::Api>();
        let path = std::env::temp_dir().join("inspector-push.slint");
        let url = Url::from_file_path(&path).unwrap();
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.api = <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api);
            state.selected = Some(ElementSelection {
                path: SourcePath::new(path),
                offset: 0.into(),
                instance_index: 0,
            });
            state.dependencies.insert(url.clone());
            state
                .source_code
                .insert(url.clone(), SourceCodeCacheEntry { version: Some(1), code: "old".into() });
        });
        set_contents(&VersionedUrl::new(url.clone(), Some(1)), "old".into());
        assert_eq!(api.get_inspector_generation(), 0);
        let unrelated = Url::from_file_path(std::env::temp_dir().join("unrelated.slint")).unwrap();
        set_contents(&VersionedUrl::new(unrelated, Some(2)), "other".into());
        assert_eq!(api.get_inspector_generation(), 0);
        set_contents(&VersionedUrl::new(url.clone(), Some(2)), "old".into());
        assert_eq!(api.get_inspector_generation(), 1);
        set_contents(&VersionedUrl::new(url, Some(2)), "new".into());
        assert_eq!(api.get_inspector_generation(), 2);
        reset_preview_state(Default::default());
    }

    const ROTATED_SOURCE: &str = r#"
export component Main {
    width: 400px;
    height: 400px;
    outer := Rectangle {
        x: 100px;
        y: 40px;
        width: 200px;
        height: 200px;
        transform-rotation: 90deg;
        inner := Rectangle {
            x: 20px;
            y: 30px;
            width: 40px;
            height: 60px;
            transform-rotation: 45deg;
        }
    }
    corner := Rectangle {
        x: 40px;
        y: 60px;
        width: 80px;
        height: 40px;
        transform-rotation: 90deg;
        transform-origin: { x: 0px, y: 0px };
    }
    zoomed := Rectangle {
        x: 40px;
        y: 60px;
        width: 80px;
        height: 40px;
        transform-scale: 2;
        transform-rotation: 90deg;
    }
    stretched := Rectangle {
        x: 40px;
        y: 60px;
        width: 80px;
        height: 40px;
        transform-scale-x: 3;
    }
    stretched_parent := Rectangle {
        x: 40px;
        y: 200px;
        width: 200px;
        height: 100px;
        transform-scale-x: 3;
        sheared := Rectangle {
            x: 20px;
            y: 10px;
            width: 80px;
            height: 40px;
            transform-rotation: 30deg;
        }
    }
}
"#;

    fn element_offset(id: &str) -> u32 {
        let declaration = ROTATED_SOURCE.find(&format!("{id} :=")).expect("element declaration");
        (declaration + ROTATED_SOURCE[declaration..].find("Rectangle").expect("element type"))
            as u32
    }

    fn geometry_of(
        instance: &ComponentInstance,
        id: &str,
    ) -> slint_interpreter::highlight::HighlightedRect {
        let path = i_slint_editor_preview::test::main_test_file_name();
        *instance.component_positions(&path, element_offset(id)).first().expect("geometry")
    }

    fn element_node_of(instance: &ComponentInstance, id: &str) -> ElementRcNode {
        let path = i_slint_editor_preview::test::main_test_file_name();
        let (element, debug_index) = instance
            .element_node_at_source_code_position(&path, element_offset(id))
            .first()
            .cloned()
            .expect("element");
        ElementRcNode::new(element, debug_index).expect("element node")
    }

    /// Wires `instance` into the preview state the way `set_preview_factory` does, so that the
    /// geometry overrides of a drag reach the previewed element.
    fn install_preview_instance(instance: &ComponentInstance) {
        reset_preview_state(Default::default());
        let overrides = PREVIEW_STATE.with_borrow(|preview_state| {
            preview_state.handle.replace(Some(instance.clone_strong()));
            preview_state.debug_hook_overrides.clone()
        });
        install_debug_hook_callback(instance, overrides);
    }

    #[test]
    fn frame_position_of_rotated_element_maps_back_to_source_coordinates() {
        let instance = test::interpret_test("fluent", ROTATED_SOURCE);

        // Drags the frame of `id` by `delta` and grows it by `grow` root-space pixels, then
        // reads back what a release would write to the source.
        let drag = |id: &str, delta: (f32, f32), grow: (f32, f32)| -> LogicalRect {
            let geometry = geometry_of(&instance, id);
            let origin = LogicalPoint::new(
                geometry.rect.origin.x + delta.0,
                geometry.rect.origin.y + delta.1,
            );
            let size = CoreLogicalSize::new(
                geometry.rect.width() + grow.0,
                geometry.rect.height() + grow.1,
            );
            parent_relative_rect(&geometry, origin, size).expect("parent-relative rectangle")
        };
        let check = |id: &str, delta: (f32, f32), expected: (f32, f32)| {
            let position = drag(id, delta, (0., 0.)).origin;
            assert!(
                (position.x - expected.0).abs() < 0.5 && (position.y - expected.1).abs() < 0.5,
                "{id} moved by {delta:?}: got {position:?}, expected {expected:?}"
            );
        };
        let check_size = |id: &str, grow: (f32, f32), expected: (f32, f32)| {
            let size = drag(id, (0., 0.), grow).size;
            assert!(
                (size.width - expected.0).abs() < 0.5 && (size.height - expected.1).abs() < 0.5,
                "{id} grown by {grow:?}: got {size:?}, expected {expected:?}"
            );
        };

        // Without a drag the mapping reproduces the source rectangle.
        check("inner", (0., 0.), (20., 30.));
        check("corner", (0., 0.), (40., 60.));
        check("zoomed", (0., 0.), (40., 60.));
        check_size("zoomed", (0., 0.), (80., 40.));
        // The parent turns by 90 degrees, so a drag to the right moves the element up.
        check("inner", (10., 0.), (20., 20.));
        check("outer", (10., 0.), (110., 40.));
        // A rotation that is not around the center keeps the source position recoverable.
        check("corner", (0., 10.), (40., 70.));
        // A scaled element keeps its scale. Its size is measured in rendered pixels, so the
        // source grows by the dragged distance divided by the scale, while its position is
        // measured in the parent's units and moves by the whole dragged distance: scaling
        // happens around the element's own center and leaves that center where it is.
        check("zoomed", (10., 0.), (50., 60.));
        check_size("zoomed", (20., 0.), (90., 40.));
        check("stretched", (12., 0.), (52., 60.));
        check_size("stretched", (30., 4.), (90., 44.));

        // A rotation below a non-uniform scale shears the element into a parallelogram, so the
        // selection frame no longer describes it and there is nothing to map a drag back onto.
        let sheared = geometry_of(&instance, "sheared");
        assert!(
            parent_relative_rect(&sheared, sheared.rect.origin, sheared.rect.size).is_none(),
            "a sheared element has no rectangle to write back"
        );
    }

    #[test]
    fn dragging_a_rotated_element_writes_parent_relative_coordinates() {
        let instance = test::interpret_test("fluent", ROTATED_SOURCE);
        install_preview_instance(&instance);

        let before = geometry_of(&instance, "inner");
        override_selected_element_geometry_impl(
            &element_node_of(&instance, "inner"),
            0,
            before.rect.origin.x + 10.,
            before.rect.origin.y,
            before.local_rect.width(),
            before.local_rect.height(),
        );

        let after = geometry_of(&instance, "inner");
        // The parent turns by 90 degrees, so dragging to the right moves the element up.
        assert_eq!(
            (after.local_rect.origin.x, after.local_rect.origin.y),
            (20., 20.),
            "source position after the drag"
        );
        // What the pointer dragged is what the element follows on screen.
        assert!(
            (after.rect.origin.x - before.rect.origin.x - 10.).abs() < 0.5
                && (after.rect.origin.y - before.rect.origin.y).abs() < 0.5,
            "the element should follow the pointer: {:?} -> {:?}",
            before.rect.origin,
            after.rect.origin
        );

        reset_preview_state(Default::default());
    }

    fn temp_file(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("slint-preview-test-{}-{name}", std::process::id()));
        fs::write(&path, "").unwrap();
        path
    }

    fn temp_project(name: &str) -> (PathBuf, PathBuf) {
        let root = std::env::temp_dir()
            .join(format!("slint-preview-project-{}-{name}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(root).unwrap();
        let path = root.join("main.slint");
        fs::write(&path, "").unwrap();
        (root, path)
    }

    fn stored_comment(file: &Path, range: SourceRange, text: &str) -> SnapshotComment {
        SnapshotComment {
            id: String::new(),
            text: text.into(),
            file: file.into(),
            range,
            component: Some("MainWindow".into()),
            element_type: "Rectangle".into(),
            element_id: Some("content".into()),
        }
    }

    #[test]
    fn editor_comments_assign_ids_filter_by_source_element_and_remove() {
        let first_range = SourceRange {
            start: SourcePosition { line: 1, character: 2 },
            end: SourcePosition { line: 3, character: 4 },
        };
        let second_range = SourceRange {
            start: SourcePosition { line: 5, character: 6 },
            end: SourcePosition { line: 7, character: 8 },
        };
        let file = Path::new("/project/main.slint");
        let first_element = SourceElement { path: file.into(), offset: TextSize::from(12) };
        let second_element = SourceElement { path: file.into(), offset: TextSize::from(24) };
        let mut comments = EditorComments { next_id: 1, ..Default::default() };

        comments.add(first_element.clone(), stored_comment(file, first_range, "First"));
        comments.add(second_element.clone(), stored_comment(file, second_range, "Second"));

        let visible = comments.visible(&first_element);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "1");
        assert_eq!(visible[0].text, "First");

        comments.add(first_element.clone(), stored_comment(file, first_range, "Another"));
        let markers = comments.markers();
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].count, 2);
        assert!(markers.iter().all(|marker| marker.unread));
        comments.mark_read(&first_element);
        let markers = comments.markers();
        assert!(!markers[0].unread);
        assert!(markers[1].unread);

        comments.remove("1");
        assert_eq!(comments.markers()[0].count, 1);
        comments.remove("3");
        assert!(comments.visible(&first_element).is_empty());
        assert_eq!(comments.visible(&second_element)[0].id, "2");
        assert_eq!(comments.markers().len(), 1);
    }

    #[test]
    fn selected_element_comments_are_published_to_the_ui_model() {
        i_slint_backend_testing::init_no_event_loop();
        let editor = ui::EditorUi::new().unwrap();
        let api = editor.global::<ui::Api>();
        let file = Path::new("/project/main.slint");
        let selection = ElementSelection {
            path: SourcePath::new(file),
            offset: TextSize::from(12),
            instance_index: 0,
        };
        let range = SourceRange {
            start: SourcePosition { line: 1, character: 2 },
            end: SourcePosition { line: 3, character: 4 },
        };
        let mut preview_state = PreviewState {
            api: <ui::Api as slint::Global<'_, ui::EditorUi>>::as_weak(&api),
            comments: EditorComments { next_id: 1, ..Default::default() },
            ..Default::default()
        };
        api.set_element_comments(preview_state.element_comments_model.clone().into());
        preview_state.comments.add(
            SourceElement::from_selection(&selection).unwrap(),
            stored_comment(file, range, "Visible"),
        );

        set_visible_element_comments(&preview_state, Some(&selection));

        let model = api.get_element_comments();
        assert_eq!(model.row_count(), 1);
        assert_eq!(model.row_data(0).unwrap().text, "Visible");
    }

    #[test]
    fn add_comment_publishes_source_declared_element_ids() {
        let source = r#"export component Main { named-element := Text { text: "😀"; } Text {} }"#;
        let component_instance = test::interpret_test("fluent", source);
        let path = main_test_file_name();
        let named_offset = TextSize::from(u32::try_from(source.find("Text").unwrap()).unwrap());
        let anonymous_offset =
            TextSize::from(u32::try_from(source.rfind("Text").unwrap()).unwrap());
        let named_end = source.find("} Text").unwrap() + 1;
        let expected_utf16_end = u32::try_from(source[..named_end].encode_utf16().count()).unwrap();
        let project = tempfile::tempdir().unwrap();
        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            *preview_state = PreviewState::default();
            preview_state.handle.replace(Some(component_instance));
            preview_state.selected = Some(ElementSelection {
                path: path.clone(),
                offset: named_offset,
                instance_index: 0,
            });
            preview_state.comments = EditorComments::new(project.path());
        });

        add_element_comment("Named".into());
        PREVIEW_STATE.with_borrow_mut(|preview_state| {
            preview_state.selected =
                Some(ElementSelection { path, offset: anonymous_offset, instance_index: 0 });
        });
        add_element_comment("Anonymous".into());

        let project_comments = slint_editor_mcp::scan_projects()
            .unwrap()
            .into_iter()
            .find(|comments| comments.project_root == project.path())
            .unwrap();
        assert_eq!(project_comments.comments.len(), 2);
        assert_eq!(project_comments.comments[0].element_id.as_deref(), Some("named-element"));
        assert_eq!(project_comments.comments[0].range.end.character, expected_utf16_end);
        assert_eq!(project_comments.comments[1].element_id, None);

        PREVIEW_STATE.with_borrow_mut(|preview_state| *preview_state = PreviewState::default());
    }

    #[test]
    fn request_preview_path_requests_slint_preview() {
        let messages = Rc::new(RefCell::new(Vec::new()));
        reset_preview_state(messages.clone());
        let path = temp_file("selected.slint");
        let url = Url::from_file_path(std::fs::canonicalize(&path).unwrap()).unwrap();

        assert!(request_preview_path(&path, None));

        let messages = messages.borrow();
        assert!(matches!(
            &messages[..],
            [PreviewToLspMessage::RequestPreview { component }]
                if component == &PreviewComponent { url: url.clone(), component: None }
        ));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn request_preview_path_ignores_non_slint_files() {
        let messages = Rc::new(RefCell::new(Vec::new()));
        reset_preview_state(messages.clone());
        let path = temp_file("image.png");

        assert!(!request_preview_path(&path, None));

        assert!(messages.borrow().is_empty());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn record_current_project_updates_settings() {
        let messages = Rc::new(RefCell::new(Vec::new()));
        reset_preview_state(messages.clone());
        let (root, path) = temp_project("recent");
        let component = PreviewComponent {
            url: Url::from_file_path(std::fs::canonicalize(&path).unwrap()).unwrap(),
            component: Some("MainWindow".into()),
        };
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.current_project_root =
                Some(Url::from_directory_path(std::fs::canonicalize(&root).unwrap()).unwrap());
            state.current_previewed_component = Some(component);
        });

        record_current_project();
        record_current_project();

        let messages = messages.borrow();
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0],
            PreviewToLspMessage::UpdateUserSettings { name, contents }
                if name == SETTINGS_FILE
                    && VisualEditorSettings::deserialize(contents).is_some_and(|settings| {
                        settings.visible_recent_projects().first().is_some_and(|project| {
                            project.component == "MainWindow"
                                && project.root_path == root.to_string_lossy().as_ref()
                        })
                    })
        ));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn record_current_project_ignores_preview_outside_root() {
        let messages = Rc::new(RefCell::new(Vec::new()));
        reset_preview_state(messages.clone());
        let (root, _) = temp_project("root");
        let (_, outside_path) = temp_project("outside");
        PREVIEW_STATE.with_borrow_mut(|state| {
            state.current_project_root =
                Some(Url::from_directory_path(std::fs::canonicalize(&root).unwrap()).unwrap());
            state.current_previewed_component = Some(PreviewComponent {
                url: Url::from_file_path(std::fs::canonicalize(&outside_path).unwrap()).unwrap(),
                component: Some("MainWindow".into()),
            });
        });

        record_current_project();

        assert!(messages.borrow().is_empty());
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside_path.parent().unwrap());
    }

    #[test]
    fn settings_from_lsp_are_not_sent_back() {
        let messages = Rc::new(RefCell::new(Vec::new()));
        reset_preview_state(messages.clone());
        let settings = VisualEditorSettings::deserialize(
            r#"{"version":1,"recent_projects":[{"root":"/missing","preview":{"url":"file:///missing/main.slint","component":"MainWindow"}}]}"#,
        )
        .unwrap();

        set_user_settings(SETTINGS_FILE.into(), settings.serialize());

        PREVIEW_STATE.with_borrow(|preview_state| {
            assert_eq!(preview_state.settings, settings);
        });
        assert!(messages.borrow().is_empty());
    }
}
