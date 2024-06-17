// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{
    self, component_catalog, ComponentInformation, ElementRcNode, PreviewComponent, PreviewConfig,
};
use crate::lsp_ext::Health;
use crate::preview::element_selection::ElementSelection;
use crate::util;
use i_slint_compiler::diagnostics;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind};
use i_slint_core::component_factory::FactoryContext;
use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalSize};
use i_slint_core::model::VecModel;
use lsp_types::Url;
use slint_interpreter::{ComponentDefinition, ComponentHandle, ComponentInstance};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
use crate::wasm_prelude::*;

mod debug;
mod drop_location;
mod element_selection;
mod ext;
use ext::ElementRcNodeExt;
pub mod ui;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

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

type SourceCodeCache = HashMap<Url, (common::UrlVersion, String)>;

#[derive(Default)]
struct ContentCache {
    source_code: SourceCodeCache,
    dependency: HashSet<Url>,
    current: Option<PreviewComponent>,
    config: PreviewConfig,
    loading_state: PreviewFutureState,
    highlight: Option<(Url, u32)>,
    ui_is_visible: bool,
}

static CONTENT_CACHE: std::sync::OnceLock<Mutex<ContentCache>> = std::sync::OnceLock::new();

#[derive(Default)]
struct PreviewState {
    ui: Option<ui::PreviewUi>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
    document_cache: Rc<RefCell<Option<Rc<common::DocumentCache>>>>,
    selected: Option<element_selection::ElementSelection>,
    notify_editor_about_selection_after_update: bool,
    known_components: Vec<ComponentInformation>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

struct DummyWaker();

impl std::task::Wake for DummyWaker {
    fn wake(self: std::sync::Arc<Self>) {}
}

pub fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    let waker = std::sync::Arc::new(DummyWaker()).into();
    let mut ctx = std::task::Context::from_waker(&waker);

    let future = std::pin::pin!(future);

    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}

pub fn set_contents(url: &common::VersionedUrl, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let old = cache.source_code.insert(url.url().clone(), (*url.version(), content.clone()));

    if let Some((old_version, old)) = old {
        if content == old && old_version == *url.version() {
            return;
        }
    }

    if cache.dependency.contains(url.url()) {
        let ui_is_visible = cache.ui_is_visible;
        let Some(current) = cache.current.clone() else {
            return;
        };

        drop(cache);

        if ui_is_visible {
            load_preview(current);
        }
    }
}

/// Try to find the parent of element `child` below `root`.
fn search_for_parent_element(root: &ElementRc, child: &ElementRc) -> Option<ElementRc> {
    for c in &root.borrow().children {
        if std::rc::Rc::ptr_eq(c, child) {
            return Some(root.clone());
        }

        if let Some(parent) = search_for_parent_element(c, child) {
            return Some(parent);
        }
    }
    None
}

// triggered from the UI, running in UI thread
fn add_new_component() {
    eprintln!("Add a new component!");    
}

// triggered from the UI, running in UI thread
fn can_drop_component(
    component_type: slint::SharedString,
    x: f32,
    y: f32,
    on_drop_area: bool,
) -> bool {
    if !on_drop_area {
        set_drop_mark(&None);
        return false;
    }

    let position = LogicalPoint::new(x, y);
    let component_type = component_type.to_string();

    drop_location::can_drop_at(position, &component_type)
}

// triggered from the UI, running in UI thread
fn drop_component(component_type: slint::SharedString, x: f32, y: f32) {
    let component_type = component_type.to_string();
    let position = LogicalPoint::new(x, y);

    let drop_result = PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow();

        let component_index = &preview_state
            .known_components
            .binary_search_by_key(&component_type.as_str(), |ci| ci.name.as_str())
            .unwrap_or(usize::MAX);

        drop_location::drop_at(position, preview_state.known_components.get(*component_index)?)
    });

    if let Some((edit, drop_data)) = drop_result {
        element_selection::select_element_at_source_code_position(
            drop_data.path,
            drop_data.selection_offset,
            None,
            true,
        );

        send_message_to_lsp(crate::common::PreviewToLspMessage::SendWorkspaceEdit {
            label: Some(format!("Add element {}", component_type)),
            edit,
        });
    };
}

fn placeholder_node_text(selected: &common::ElementRcNode) -> String {
    let Some(parent) = selected.parent() else {
        return Default::default();
    };

    if parent.layout_kind() != ui::LayoutKind::None && parent.children().len() == 1 {
        return format!("Rectangle {{ /* {} */ }}", NODE_IGNORE_COMMENT);
    }

    Default::default()
}

// triggered from the UI, running in UI thread
fn delete_selected_element() {
    let Some(selected) = selected_element() else {
        return;
    };

    let Ok(url) = Url::from_file_path(&selected.path) else {
        return;
    };

    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let Some((version, _)) = cache.source_code.get(&url).cloned() else {
        return;
    };

    let Some(selected_node) = selected.as_element_node() else {
        return;
    };

    let Some(range) = selected_node.with_decorated_node(|n| util::map_node(&n)) else {
        return;
    };

    // Insert a placeholder node into layouts if those end up empty:
    let new_text = placeholder_node_text(&selected_node);

    let edit =
        common::create_workspace_edit(url, version, vec![lsp_types::TextEdit { range, new_text }]);

    send_message_to_lsp(crate::common::PreviewToLspMessage::SendWorkspaceEdit {
        label: Some("Delete element".to_string()),
        edit,
    });
}

// triggered from the UI, running in UI thread
fn resize_selected_element(x: f32, y: f32, width: f32, height: f32) {
    resize_selected_element_impl(LogicalRect::new(
        LogicalPoint::new(x, y),
        LogicalSize::new(width, height),
    ))
}

fn resize_selected_element_impl(rect: LogicalRect) {
    let Some(selected) = selected_element() else {
        return;
    };
    let Some(selected_element_node) = selected.as_element_node() else {
        return;
    };
    let Some(component_instance) = component_instance() else {
        return;
    };

    let Some(geometry) =
        selected_element_node.geometries(&component_instance).get(selected.instance_index).cloned()
    else {
        return;
    };

    let position = rect.origin;
    let root_element = element_selection::root_element(&component_instance);

    let parent = search_for_parent_element(&root_element, &selected_element_node.element)
        .and_then(|parent_element| {
            component_instance
                .element_positions(&parent_element)
                .iter()
                .find(|g| g.contains(position))
                .map(|g| g.origin)
        })
        .unwrap_or_default();

    let (properties, op) = {
        let mut p = Vec::with_capacity(4);
        let mut op = "";
        if geometry.origin.x != position.x && position.x.is_finite() {
            p.push(crate::common::PropertyChange::new(
                "x",
                format!("{}px", (position.x - parent.x).round()),
            ));
            op = "Moving";
        }
        if geometry.origin.y != position.y && position.y.is_finite() {
            p.push(crate::common::PropertyChange::new(
                "y",
                format!("{}px", (position.y - parent.y).round()),
            ));
            op = "Moving";
        }
        if geometry.size.width != rect.size.width && rect.size.width.is_finite() {
            p.push(crate::common::PropertyChange::new(
                "width",
                format!("{}px", rect.size.width.round()),
            ));
            op = "Resizing";
        }
        if geometry.size.height != rect.size.height && rect.size.height.is_finite() {
            p.push(crate::common::PropertyChange::new(
                "height",
                format!("{}px", rect.size.height.round()),
            ));
            op = "Resizing";
        }
        (p, op)
    };

    if !properties.is_empty() {
        let Ok(url) = Url::from_file_path(&selected.path) else {
            return;
        };

        let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        let Some((version, _)) = cache.source_code.get(&url).cloned() else {
            return;
        };

        send_message_to_lsp(crate::common::PreviewToLspMessage::UpdateElement {
            label: Some(format!("{op} element")),
            position: common::VersionedPosition::new(
                common::VersionedUrl::new(url, version),
                selected.offset,
            ),
            properties,
        });
    }
}

// triggered from the UI, running in UI thread
fn can_move_selected_element(x: f32, y: f32, mouse_x: f32, mouse_y: f32) -> bool {
    let position = LogicalPoint::new(x, y);
    let mouse_position = LogicalPoint::new(mouse_x, mouse_y);
    let Some(selected) = selected_element() else {
        return false;
    };
    let Some(selected_element_node) = selected.as_element_node() else {
        return false;
    };
    let Some(document_cache) = document_cache() else {
        return false;
    };

    drop_location::can_move_to(&document_cache, position, mouse_position, selected_element_node)
}

// triggered from the UI, running in UI thread
fn move_selected_element(x: f32, y: f32, mouse_x: f32, mouse_y: f32) {
    let position = LogicalPoint::new(x, y);
    let mouse_position = LogicalPoint::new(mouse_x, mouse_y);
    let Some(selected) = selected_element() else {
        return;
    };
    let Some(selected_element_node) = selected.as_element_node() else {
        return;
    };
    let Some(document_cache) = document_cache() else {
        return;
    };

    if let Some((edit, drop_data)) = drop_location::move_element_to(
        &document_cache,
        selected_element_node,
        position,
        mouse_position,
    ) {
        element_selection::select_element_at_source_code_position(
            drop_data.path,
            drop_data.selection_offset,
            None,
            true,
        );

        send_message_to_lsp(crate::common::PreviewToLspMessage::SendWorkspaceEdit {
            label: Some("Move element".to_string()),
            edit,
        });
    } else {
        element_selection::reselect_element();
    }
}

fn change_style() {
    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let ui_is_visible = cache.ui_is_visible;
    let Some(current) = cache.current.clone() else {
        return;
    };

    drop(cache);

    if ui_is_visible {
        load_preview(current);
    }
}

fn start_parsing() {
    set_status_text("Updating Preview...");
    set_diagnostics(&[]);
    send_status("Loading Preview…", Health::Ok);
}

fn finish_parsing(ok: bool) {
    set_status_text("");
    if ok {
        send_status("Preview Loaded", Health::Ok);
    } else {
        send_status("Preview not updated", Health::Error);
    }

    if let Some(document_cache) = document_cache() {
        let mut components = Vec::new();
        // `_SLINT_LivePreview` gets returned as `-SLINT-LivePreview`, which is unfortunately not a valid identifier.
        // I do not want to store two constants, so map it over ;-/
        let private_preview_component = SLINT_LIVEPREVIEW_COMPONENT.replace('_', "-");
        component_catalog::builtin_components(&document_cache, &mut components);
        component_catalog::all_exported_components(
            &document_cache,
            &mut |ci| !(ci.is_global || ci.name == private_preview_component),
            &mut components,
        );

        components.sort_by(|a, b| a.name.cmp(&b.name));

        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            preview_state.known_components = components;

            if let Some(ui) = &preview_state.ui {
                ui::ui_set_known_components(ui, &preview_state.known_components)
            }
        });
    }
}

pub fn config_changed(config: PreviewConfig) {
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        if cache.config != config {
            cache.config = config.clone();

            let current = cache.current.clone();
            let ui_is_visible = cache.ui_is_visible;
            let hide_ui = cache.config.hide_ui;

            drop(cache);

            if ui_is_visible {
                if let Some(hide_ui) = hide_ui {
                    set_show_preview_ui(!hide_ui);
                }
                if let Some(current) = current {
                    load_preview(current);
                }
            }
        }
    };
}

/// If the file is in the cache, returns it.
/// In any way, register it as a dependency
fn get_url_from_cache(url: &Url) -> Option<(common::UrlVersion, String)> {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let r = cache.source_code.get(url).cloned();
    cache.dependency.insert(url.to_owned());
    r
}

fn get_path_from_cache(path: &Path) -> Option<(common::UrlVersion, String)> {
    let url = Url::from_file_path(path).ok()?;
    get_url_from_cache(&url)
}

pub fn load_preview(preview_component: PreviewComponent) {
    {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.current = Some(preview_component.clone());

        if !cache.ui_is_visible {
            return;
        }
        match cache.loading_state {
            PreviewFutureState::Pending => (),
            PreviewFutureState::PreLoading => return,
            PreviewFutureState::Loading => {
                cache.loading_state = PreviewFutureState::NeedsReload;
                return;
            }
            PreviewFutureState::NeedsReload => return,
        }
        cache.loading_state = PreviewFutureState::PreLoading;
    };

    run_in_ui_thread(move || async move {
        let (selected, notify_editor) = PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            let notify_editor = preview_state.notify_editor_about_selection_after_update;
            preview_state.notify_editor_about_selection_after_update = false;
            (preview_state.selected.take(), notify_editor)
        });

        loop {
            let (preview_component, config) = {
                let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
                let Some(current) = &mut cache.current else { return };
                let preview_component = current.clone();
                current.style.clear();

                assert_eq!(cache.loading_state, PreviewFutureState::PreLoading);
                if !cache.ui_is_visible {
                    cache.loading_state = PreviewFutureState::Pending;
                    return;
                }
                cache.loading_state = PreviewFutureState::Loading;
                cache.dependency.clear();
                (preview_component, cache.config.clone())
            };
            let style = if preview_component.style.is_empty() {
                get_current_style()
            } else {
                set_current_style(preview_component.style.clone());
                preview_component.style.clone()
            };

            reload_preview_impl(preview_component, style, config).await;

            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            match cache.loading_state {
                PreviewFutureState::Loading => {
                    cache.loading_state = PreviewFutureState::Pending;
                    break;
                }
                PreviewFutureState::Pending => unreachable!(),
                PreviewFutureState::PreLoading => unreachable!(),
                PreviewFutureState::NeedsReload => {
                    cache.loading_state = PreviewFutureState::PreLoading;
                    continue;
                }
            };
        }

        if let Some(se) = selected {
            element_selection::select_element_at_source_code_position(
                se.path.clone(),
                se.offset,
                None,
                false,
            );

            if notify_editor {
                if let Some(component_instance) = component_instance() {
                    if let Some((element, debug_index)) = component_instance
                        .element_node_at_source_code_position(&se.path, se.offset)
                        .first()
                    {
                        let Some(element_node) = ElementRcNode::new(element.clone(), *debug_index)
                        else {
                            return;
                        };
                        let (path, pos) = element_node.with_element_node(|node| {
                            let sf = &node.source_file;
                            (sf.path().to_owned(), util::map_position(sf, se.offset.into()))
                        });
                        ask_editor_to_show_document(
                            &path.to_string_lossy(),
                            lsp_types::Range::new(pos, pos),
                        );
                    }
                }
            }
        }
    });
}

async fn parse_source(
    include_paths: Vec<PathBuf>,
    library_paths: HashMap<String, PathBuf>,
    path: PathBuf,
    source_code: String,
    style: String,
    file_loader_fallback: impl Fn(
            &Path,
        ) -> core::pin::Pin<
            Box<dyn core::future::Future<Output = Option<std::io::Result<String>>>>,
        > + 'static,
) -> (Vec<diagnostics::Diagnostic>, Option<ComponentDefinition>) {
    let mut builder = slint_interpreter::ComponentCompiler::default();

    #[cfg(target_arch = "wasm32")]
    {
        let cc = builder.compiler_configuration(i_slint_core::InternalToken);
        cc.resource_url_mapper = resource_url_mapper();
    }

    if !style.is_empty() {
        builder.set_style(style);
    }
    builder.set_include_paths(include_paths);
    builder.set_library_paths(library_paths);
    builder.set_file_loader(file_loader_fallback);

    let compiled = builder.build_from_source(source_code, path).await;

    (builder.diagnostics().clone(), compiled)
}

pub const SLINT_LIVEPREVIEW_COMPONENT: &str = "_SLINT_LivePreview";

// Must be inside the thread running the slint event loop
async fn reload_preview_impl(
    preview_component: PreviewComponent,
    style: String,
    config: PreviewConfig,
) {
    let component = PreviewComponent { style: String::new(), ..preview_component };

    start_parsing();

    let path = component.url.to_file_path().unwrap_or(PathBuf::from(&component.url.to_string()));
    let source = {
        let (_, from_cache) = get_url_from_cache(&component.url).unwrap_or_default();
        if let Some(component_name) = &component.component {
            format!(
                "{from_cache}\nexport component {SLINT_LIVEPREVIEW_COMPONENT} inherits {component_name} {{ /* {NODE_IGNORE_COMMENT} */ }}\n",
            )
        } else {
            from_cache
        }
    };

    let (diagnostics, compiled) =
        parse_source(config.include_paths, config.library_paths, path, source, style, |path| {
            let path = path.to_owned();
            Box::pin(async move { get_path_from_cache(&path).map(|(_, c)| Result::Ok(c)) })
        })
        .await;

    notify_diagnostics(&diagnostics);

    let success = compiled.is_some();
    update_preview_area(compiled);
    finish_parsing(success);
}

/// This sets up the preview area to show the ComponentInstance
///
/// This must be run in the UI thread.
fn set_preview_factory(
    ui: &ui::PreviewUi,
    compiled: ComponentDefinition,
    callback: Box<dyn Fn(ComponentInstance)>,
) {
    // Ensure that the popup is closed as it is related to the old factory
    i_slint_core::window::WindowInner::from_pub(ui.window()).close_popup();

    let factory = slint::ComponentFactory::new(move |ctx: FactoryContext| {
        let instance = compiled.create_embedded(ctx).unwrap();

        if let Some((url, offset)) =
            CONTENT_CACHE.get().and_then(|c| c.lock().unwrap().highlight.clone())
        {
            highlight(Some(url), offset);
        } else {
            highlight(None, 0);
        }

        callback(instance.clone_strong());

        Some(instance)
    });
    ui.set_preview_area(factory);
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn highlight(url: Option<Url>, offset: u32) {
    let highlight = url.clone().map(|u| (u, offset));
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.highlight == highlight {
        return;
    }
    cache.highlight = highlight;

    let selected = selected_element();

    if cache.highlight.as_ref().map_or(true, |(url, _)| cache.dependency.contains(url)) {
        run_in_ui_thread(move || async move {
            let Some(path) = url.and_then(|u| Url::to_file_path(&u).ok()) else {
                return;
            };

            if Some((path.clone(), offset)) == selected.map(|s| (s.path, s.offset)) {
                // Already selected!
                return;
            }
            element_selection::select_element_at_source_code_position(path, offset, None, false);
        })
    }
}

pub fn get_component_info(component_type: &str) -> Option<ComponentInformation> {
    PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow();
        let index = preview_state
            .known_components
            .binary_search_by(|ci| ci.name.as_str().cmp(component_type))
            .ok()?;
        preview_state.known_components.get(index).cloned()
    })
}

fn convert_diagnostics(
    diagnostics: &[slint_interpreter::Diagnostic],
) -> HashMap<Url, Vec<lsp_types::Diagnostic>> {
    let mut result: HashMap<Url, Vec<lsp_types::Diagnostic>> = Default::default();
    for d in diagnostics {
        if d.source_file().map_or(true, |f| !i_slint_compiler::pathutils::is_absolute(f)) {
            continue;
        }
        let uri = Url::from_file_path(d.source_file().unwrap())
            .ok()
            .unwrap_or_else(|| Url::parse("file:/unknown").unwrap());
        result.entry(uri).or_default().push(crate::util::to_lsp_diag(d));
    }
    result
}

fn reset_selections(ui: &ui::PreviewUi) {
    let model = Rc::new(slint::VecModel::from(Vec::new()));
    ui.set_selections(slint::ModelRc::from(model));
}

fn set_selections(
    ui: Option<&ui::PreviewUi>,
    main_index: usize,
    layout_kind: ui::LayoutKind,
    is_moveable: bool,
    is_resizable: bool,
    positions: &[i_slint_core::lengths::LogicalRect],
) {
    let Some(ui) = ui else {
        return;
    };

    let values = positions
        .iter()
        .enumerate()
        .map(|(i, g)| ui::Selection {
            geometry: ui::SelectionRectangle {
                width: g.size.width,
                height: g.size.height,
                x: g.origin.x,
                y: g.origin.y,
            },
            layout_data: layout_kind,
            is_primary: i == main_index,
            is_moveable,
            is_resizable,
        })
        .collect::<Vec<_>>();
    let model = Rc::new(slint::VecModel::from(values));
    ui.set_selections(slint::ModelRc::from(model));
}

fn set_drop_mark(mark: &Option<drop_location::DropMark>) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow();

        let Some(ui) = &preview_state.ui else {
            return;
        };

        if let Some(m) = mark {
            ui.set_drop_mark(ui::DropMark {
                x1: m.start.x,
                y1: m.start.y,
                x2: m.end.x,
                y2: m.end.y,
            });
        } else {
            ui.set_drop_mark(ui::DropMark { x1: -1.0, y1: -1.0, x2: -1.0, y2: -1.0 });
        }
    })
}

fn set_selected_element(
    selection: Option<element_selection::ElementSelection>,
    positions: &[i_slint_core::lengths::LogicalRect],
    notify_editor_about_selection_after_update: bool,
) {
    let (layout_kind, is_in_layout) = selection
        .as_ref()
        .and_then(|s| s.as_element_node())
        .map(|en| (en.layout_kind(), element_selection::is_element_node_in_layout(&en)))
        .unwrap_or((ui::LayoutKind::None, false));

    set_drop_mark(&None);

    PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();

        let is_layout = layout_kind != ui::LayoutKind::None;
        set_selections(
            preview_state.ui.as_ref(),
            selection.as_ref().map(|s| s.instance_index).unwrap_or_default(),
            layout_kind,
            true,
            !is_in_layout && !is_layout,
            positions,
        );

        preview_state.selected = selection;
        preview_state.notify_editor_about_selection_after_update =
            notify_editor_about_selection_after_update;
    })
}

fn selected_element() -> Option<ElementSelection> {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow();
        preview_state.selected.clone()
    })
}

fn component_instance() -> Option<ComponentInstance> {
    PREVIEW_STATE.with(move |preview_state| {
        preview_state.borrow().handle.borrow().as_ref().map(|ci| ci.clone_strong())
    })
}

/// This is a *read-only* snapshot of the raw type loader, use this when you
/// need to know the exact state the compiled resources were in.
fn document_cache() -> Option<Rc<common::DocumentCache>> {
    PREVIEW_STATE.with(move |preview_state| {
        preview_state.borrow().document_cache.borrow().as_ref().map(|dc| dc.clone())
    })
}

fn set_show_preview_ui(show_preview_ui: bool) {
    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            if let Some(ui) = &preview_state.ui {
                ui.set_show_preview_ui(show_preview_ui)
            }
        })
    });
}

fn set_current_style(style: String) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow_mut();
        if let Some(ui) = &preview_state.ui {
            ui.set_current_style(style.into())
        }
    });
}

fn get_current_style() -> String {
    PREVIEW_STATE.with(|preview_state| -> String {
        let preview_state = preview_state.borrow();
        if let Some(ui) = &preview_state.ui {
            ui.get_current_style().as_str().to_string()
        } else {
            String::new()
        }
    })
}

fn set_status_text(text: &str) {
    let text = text.to_string();

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                ui.set_status_text(text.into());
            }
        });
    })
    .unwrap();
}

fn set_diagnostics(diagnostics: &[slint_interpreter::Diagnostic]) {
    let data = crate::preview::ui::convert_diagnostics(diagnostics);

    i_slint_core::api::invoke_from_event_loop(move || {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();
            if let Some(ui) = &preview_state.ui {
                let model = VecModel::from(data);
                ui.set_diagnostics(Rc::new(model).into());
            }
        });
    })
    .unwrap();
}

/// This runs `set_preview_factory` in the UI thread
fn update_preview_area(compiled: Option<ComponentDefinition>) {
    PREVIEW_STATE.with(|preview_state| {
        #[allow(unused_mut)]
        let mut preview_state = preview_state.borrow_mut();

        #[cfg(not(target_arch = "wasm32"))]
        native::open_ui_impl(&mut preview_state);

        let ui = preview_state.ui.as_ref().unwrap();
        let shared_handle = preview_state.handle.clone();
        let shared_document_cache = preview_state.document_cache.clone();

        if let Some(compiled) = compiled {
            set_preview_factory(
                ui,
                compiled,
                Box::new(move |instance| {
                    if let Some(rtl) = instance.definition().raw_type_loader() {
                        shared_document_cache.replace(Some(Rc::new(
                            common::DocumentCache::new_from_type_loader(rtl),
                        )));
                    }
                    shared_handle.replace(Some(instance));
                }),
            );
            reset_selections(ui);
        }

        ui.show().unwrap();
    });
}

pub fn lsp_to_preview_message(
    message: crate::common::LspToPreviewMessage,
    #[cfg(not(target_arch = "wasm32"))] sender: &crate::ServerNotifier,
) {
    use crate::common::LspToPreviewMessage as M;
    match message {
        M::SetContents { url, contents } => {
            set_contents(&url, contents);
        }
        M::SetConfiguration { config } => {
            config_changed(config);
        }
        M::ShowPreview(pc) => {
            #[cfg(not(target_arch = "wasm32"))]
            native::open_ui(sender);
            load_preview(pc);
        }
        M::HighlightFromEditor { url, offset } => {
            highlight(url, offset);
        }
    }
}

/// Use this in nodes you want the language server and preview to
/// ignore a node for code analysis purposes.
pub const NODE_IGNORE_COMMENT: &str = "@lsp:ignore-node";

/// Check whether a node is marked to be ignored in the LSP/live preview
/// using a comment containing `@lsp:ignore-node`
pub fn is_element_node_ignored(node: &syntax_nodes::Element) -> bool {
    node.children_with_tokens().any(|nt| {
        nt.as_token()
            .map(|t| t.kind() == SyntaxKind::Comment && t.text().contains(NODE_IGNORE_COMMENT))
            .unwrap_or(false)
    })
}

#[cfg(test)]
pub mod test {
    use std::{collections::HashMap, path::PathBuf, rc::Rc};

    use slint_interpreter::ComponentInstance;

    use crate::common::test::main_test_file_name;

    #[track_caller]
    pub fn interpret_test_with_sources(
        style: &str,
        code: HashMap<PathBuf, String>,
    ) -> ComponentInstance {
        i_slint_backend_testing::init_no_event_loop();
        reinterpret_test_with_sources(style, code)
    }

    #[track_caller]
    pub fn reinterpret_test_with_sources(
        style: &str,
        code: HashMap<PathBuf, String>,
    ) -> ComponentInstance {
        let code = Rc::new(code);

        let path = main_test_file_name();
        let source_code = code.get(&path).unwrap().clone();
        let (diagnostics, component_definition) = spin_on::spin_on(super::parse_source(
            vec![],
            std::collections::HashMap::new(),
            path,
            source_code.to_string(),
            style.to_string(),
            move |path| {
                let code = code.clone();
                let path = path.to_owned();

                Box::pin(async move {
                    let Some(source) = code.get(&path) else {
                        return Some(Result::Err(std::io::Error::new(
                            std::io::ErrorKind::NotFound,
                            "path not found",
                        )));
                    };
                    Some(Ok(source.clone()))
                })
            },
        ));

        i_slint_core::debug_log!("Test source diagnostics:\n{diagnostics:?}");
        assert!(diagnostics.is_empty());

        component_definition.unwrap().create().unwrap()
    }

    #[track_caller]
    pub fn interpret_test(style: &str, source_code: &str) -> ComponentInstance {
        let code = HashMap::from([(main_test_file_name(), source_code.to_string())]);
        interpret_test_with_sources(style, code)
    }
}
