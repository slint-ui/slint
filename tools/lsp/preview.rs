// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use crate::common::{self, ComponentInformation, ElementRcNode, PreviewComponent, PreviewConfig};
use crate::lsp_ext::Health;
use crate::preview::element_selection::ElementSelection;
use crate::util;
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

#[derive(Default)]
struct ContentCache {
    source_code: HashMap<Url, (common::UrlVersion, String)>,
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
    selected: Option<element_selection::ElementSelection>,
    notify_editor_about_selection_after_update: bool,
    known_components: Vec<ComponentInformation>,
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

pub fn set_contents(url: &common::VersionedUrl, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let old = cache.source_code.insert(url.url().clone(), (*url.version(), content.clone()));
    if cache.dependency.contains(url.url()) {
        if let Some((old_version, old)) = old {
            if content == old && old_version == *url.version() {
                return;
            }
        }
        let Some(current) = cache.current.clone() else {
            return;
        };
        let ui_is_visible = cache.ui_is_visible;

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
    let Some(component_instance) = component_instance() else {
        return Default::default();
    };
    let root_element = element_selection::root_element(&component_instance);
    let Some(parent) = selected.parent(root_element) else {
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

    let Some(range) = selected_node.with_element_node(|n| {
        if let Some(parent) = &n.parent() {
            if parent.kind() == SyntaxKind::SubElement {
                return util::map_node(parent);
            }
        }
        util::map_node(n)
    }) else {
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
fn can_move_selected_element(_x: f32, _y: f32, mouse_x: f32, mouse_y: f32) -> bool {
    let Some(selected) = selected_element() else {
        return false;
    };
    let Some(selected_element_node) = selected.as_element_node() else {
        return false;
    };

    let mouse_position = LogicalPoint::new(mouse_x, mouse_y);
    drop_location::can_move_to(mouse_position, selected_element_node)
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

    if let Some((edit, drop_data)) =
        drop_location::move_element_to(selected_element_node, position, mouse_position)
    {
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
}

pub fn config_changed(config: PreviewConfig) {
    if let Some(cache) = CONTENT_CACHE.get() {
        let mut cache = cache.lock().unwrap();
        if cache.config != config {
            cache.config = config;
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
) -> (Vec<i_slint_compiler::diagnostics::Diagnostic>, Option<ComponentDefinition>) {
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
                "{from_cache}\nexport component _SLINT_LivePreview inherits {component_name} {{ /* {} */ }}\n",
                NODE_IGNORE_COMMENT,
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

pub fn known_components(
    _url: &Option<common::VersionedUrl>,
    mut components: Vec<ComponentInformation>,
) {
    components.sort_unstable_by_key(|ci| ci.name.clone());

    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            preview_state.known_components = components;

            if let Some(ui) = &preview_state.ui {
                ui::ui_set_known_components(ui, &preview_state.known_components)
            }
        })
    });
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
) -> HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>> {
    let mut result: HashMap<lsp_types::Url, Vec<lsp_types::Diagnostic>> = Default::default();
    for d in diagnostics {
        if d.source_file().map_or(true, |f| !i_slint_compiler::pathutils::is_absolute(f)) {
            continue;
        }
        let uri = lsp_types::Url::from_file_path(d.source_file().unwrap())
            .ok()
            .unwrap_or_else(|| lsp_types::Url::parse("file:/unknown").unwrap());
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

        if let Some(compiled) = compiled {
            set_preview_factory(
                ui,
                compiled,
                Box::new(move |instance| {
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
        M::KnownComponents { url, components } => {
            known_components(&url, components);
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
mod test {
    use slint_interpreter::ComponentInstance;

    #[track_caller]
    pub fn compile_test(style: &str, source_code: &str) -> ComponentInstance {
        i_slint_backend_testing::init_no_event_loop();

        let path = std::path::PathBuf::from("/test_data.slint");
        let (diagnostics, component_definition) = spin_on::spin_on(super::parse_source(
            vec![],
            std::collections::HashMap::new(),
            path,
            source_code.to_string(),
            style.to_string(),
            |_path| {
                Box::pin(async move {
                    Some(Result::Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "Not supported in tests",
                    )))
                })
            },
        ));

        i_slint_core::debug_log!("Test source diagnostics:\n{diagnostics:?}");
        assert!(diagnostics.is_empty());

        component_definition.unwrap().create().unwrap()
    }
}
