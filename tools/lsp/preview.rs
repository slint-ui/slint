// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use crate::common::{
    ComponentInformation, PreviewComponent, PreviewConfig, UrlVersion, VersionedPosition,
    VersionedUrl,
};
use crate::lsp_ext::Health;
use crate::preview::element_selection::ElementSelection;
use crate::util::map_position;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes::Element, SyntaxKind};
use i_slint_core::component_factory::FactoryContext;
use i_slint_core::lengths::{LogicalLength, LogicalPoint};
use i_slint_core::model::VecModel;
use lsp_types::Url;
use slint_interpreter::highlight::ComponentPositions;
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
mod ui;
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
    source_code: HashMap<Url, (UrlVersion, String)>,
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
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

pub fn set_contents(url: &VersionedUrl, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let old = cache.source_code.insert(url.url().clone(), (url.version().clone(), content.clone()));
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
fn can_drop_component(_component_name: slint::SharedString, x: f32, y: f32) -> bool {
    drop_location::can_drop_at(x, y)
}

// triggered from the UI, running in UI thread
fn drop_component(
    component_name: slint::SharedString,
    import_path: slint::SharedString,
    is_layout: bool,
    x: f32,
    y: f32,
) {
    if let Some((component, drop_data)) =
        drop_location::drop_at(x, y, component_name.to_string(), import_path.to_string())
    {
        let path = Url::to_file_path(component.insert_position.url()).ok().unwrap_or_default();
        element_selection::select_element_at_source_code_position(
            path,
            drop_data.selection_offset,
            drop_data.debug_index,
            is_layout,
            None,
            true,
        );

        send_message_to_lsp(crate::common::PreviewToLspMessage::AddComponent {
            label: Some(format!("Dropped {}", component_name.as_str())),
            component,
        });
    };
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

    send_message_to_lsp(crate::common::PreviewToLspMessage::RemoveElement {
        label: Some("Deleting element".to_string()),
        position: VersionedPosition::new(VersionedUrl::new(url, version), selected.offset),
    });
}

// triggered from the UI, running in UI thread
fn change_geometry_of_selected_element(x: f32, y: f32, width: f32, height: f32) {
    let Some(selected) = selected_element() else {
        return;
    };
    let Some(selected_element) = selected.as_element() else {
        return;
    };
    let Some(component_instance) = component_instance() else {
        return;
    };

    let Some(geometry) = component_instance
        .element_position(&selected_element)
        .get(selected.instance_index)
        .cloned()
    else {
        return;
    };

    let click_position = LogicalPoint::from_lengths(LogicalLength::new(x), LogicalLength::new(y));
    let root_element = element_selection::root_element(&component_instance);

    let (parent_x, parent_y) = search_for_parent_element(&root_element, &selected_element)
        .and_then(|parent_element| {
            component_instance
                .element_position(&parent_element)
                .iter()
                .find(|g| g.contains(click_position))
                .map(|g| (g.origin.x, g.origin.y))
        })
        .unwrap_or_default();

    let (properties, op) = {
        let mut p = Vec::with_capacity(4);
        let mut op = "";
        if geometry.origin.x != x {
            p.push(crate::common::PropertyChange::new(
                "x",
                format!("{}px", (x - parent_x).round()),
            ));
            op = "Moving";
        }
        if geometry.origin.y != y {
            p.push(crate::common::PropertyChange::new(
                "y",
                format!("{}px", (y - parent_y).round()),
            ));
            op = "Moving";
        }
        if geometry.size.width != width {
            p.push(crate::common::PropertyChange::new("width", format!("{}px", width.round())));
            op = "Resizing";
        }
        if geometry.size.height != height {
            p.push(crate::common::PropertyChange::new("height", format!("{}px", height.round())));
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
            position: VersionedPosition::new(VersionedUrl::new(url, version), selected.offset),
            properties,
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

pub fn adjust_selection(url: VersionedUrl, start_offset: u32, end_offset: u32, new_length: u32) {
    let Some((version, _)) = get_url_from_cache(url.url()) else {
        return;
    };

    run_in_ui_thread(move || async move {
        if &version != url.version() {
            // We are outdated anyway, no use updating now.
            return;
        };

        let Ok(path) = Url::to_file_path(url.url()) else {
            return;
        };

        let Some(selected) = PREVIEW_STATE.with(move |preview_state| {
            let preview_state = preview_state.borrow();

            preview_state.selected.clone()
        }) else {
            return;
        };

        if selected.path != path {
            // Not relevant for the current selection
            return;
        }
        if selected.offset < start_offset {
            // Nothing to do!
        } else if selected.offset >= start_offset {
            // Worst case if we get the offset wrong:
            // Some other nearby element ends up getting marked as selected.
            // So ignore special cases :-)
            let old_length = end_offset - start_offset;
            let offset = selected.offset + new_length - old_length;
            PREVIEW_STATE.with(move |preview_state| {
                let mut preview_state = preview_state.borrow_mut();
                preview_state.selected = Some(ElementSelection { offset, ..selected });
            });
        }
    })
}

/// If the file is in the cache, returns it.
/// In any way, register it as a dependency
fn get_url_from_cache(url: &Url) -> Option<(UrlVersion, String)> {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let r = cache.source_code.get(url).cloned();
    cache.dependency.insert(url.to_owned());
    r
}

fn get_path_from_cache(path: &Path) -> Option<(UrlVersion, String)> {
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
                se.debug_index,
                se.is_layout,
                None,
                false,
            );

            if notify_editor {
                if let Some(component_instance) = component_instance() {
                    if let Some(element) = component_instance
                        .element_at_source_code_position(&se.path, se.offset)
                        .first()
                    {
                        if let Some((node, _)) =
                            element.borrow().debug.iter().find(|n| !is_element_node_ignored(&n.0))
                        {
                            let sf = &node.source_file;
                            let pos = map_position(sf, se.offset.into());
                            ask_editor_to_show_document(
                                &se.path.to_string_lossy(),
                                lsp_types::Range::new(pos.clone(), pos),
                            );
                        }
                    }
                }
            }
        }
    });
}

// Most be inside the thread running the slint event loop
async fn reload_preview_impl(
    preview_component: PreviewComponent,
    style: String,
    config: PreviewConfig,
) {
    let component = PreviewComponent { style: String::new(), ..preview_component };

    start_parsing();

    let mut builder = slint_interpreter::ComponentCompiler::default();

    #[cfg(target_arch = "wasm32")]
    {
        let cc = builder.compiler_configuration(i_slint_core::InternalToken);
        cc.resource_url_mapper = resource_url_mapper();
    }

    if !style.is_empty() {
        builder.set_style(style.clone());
    }
    builder.set_include_paths(config.include_paths);
    builder.set_library_paths(config.library_paths);

    builder.set_file_loader(|path| {
        let path = path.to_owned();
        Box::pin(async move { get_path_from_cache(&path).map(|(_, c)| Result::Ok(c)) })
    });

    // to_file_path on a WASM Url just returns the URL as the path!
    let path = component.url.to_file_path().unwrap_or(PathBuf::from(&component.url.to_string()));

    let compiled = if let Some((_, mut from_cache)) = get_url_from_cache(&component.url) {
        if let Some(component_name) = &component.component {
            from_cache = format!(
                "{from_cache}\nexport component _SLINT_LivePreview inherits {component_name} {{ /* {NODE_IGNORE_COMMENT} */ }}\n",
            );
        }
        builder.build_from_source(from_cache, path).await
    } else {
        builder.build_from_path(path).await
    };

    notify_diagnostics(builder.diagnostics());

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

    if cache.highlight.as_ref().map_or(true, |(url, _)| cache.dependency.contains(url)) {
        run_in_ui_thread(move || async move {
            let Some(component_instance) = component_instance() else {
                return;
            };
            let Some(path) = url.and_then(|u| Url::to_file_path(&u).ok()) else {
                return;
            };
            let elements = component_instance.element_at_source_code_position(&path, offset);
            if let Some(e) = elements.first() {
                let Some(debug_index) = e.borrow().debug.iter().position(|(n, _)| {
                    n.text_range().contains(offset.into()) && n.source_file.path() == path
                }) else {
                    return;
                };
                let is_layout =
                    e.borrow().debug.get(debug_index).map_or(false, |(_, l)| l.is_some());
                element_selection::select_element_at_source_code_position(
                    path,
                    offset,
                    debug_index,
                    is_layout,
                    None,
                    false,
                );
            } else {
                element_selection::unselect_element();
            }
        })
    }
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn known_components(_url: &Option<VersionedUrl>, components: Vec<ComponentInformation>) {
    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let current_url = cache.current.as_ref().map(|pc| pc.url.clone());

    run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            if let Some(ui) = &preview_state.ui {
                ui::ui_set_known_components(ui, &current_url, &components)
            }
        })
    });
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
    is_layout: bool,
    positions: ComponentPositions,
) {
    let Some(ui) = ui else {
        return;
    };

    let border_color = if is_layout {
        i_slint_core::Color::from_argb_encoded(0xffff0000)
    } else {
        i_slint_core::Color::from_argb_encoded(0xff0000ff)
    };
    let secondary_border_color = if is_layout {
        i_slint_core::Color::from_argb_encoded(0x80ff0000)
    } else {
        i_slint_core::Color::from_argb_encoded(0x800000ff)
    };

    let values = positions
        .geometries
        .iter()
        .enumerate()
        .map(|(i, g)| ui::Selection {
            width: g.size.width,
            height: g.size.height,
            x: g.origin.x,
            y: g.origin.y,
            border_color: if i == main_index { border_color } else { secondary_border_color },
            is_primary: i == main_index,
            is_moveable: false,
            is_resizable: false,
        })
        .collect::<Vec<_>>();
    let model = Rc::new(slint::VecModel::from(values));
    ui.set_selections(slint::ModelRc::from(model));
}

fn set_selected_element(
    selection: Option<element_selection::ElementSelection>,
    positions: slint_interpreter::highlight::ComponentPositions,
    notify_editor_about_selection_after_update: bool,
) {
    PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();

        set_selections(
            preview_state.ui.as_ref(),
            selection.as_ref().map(|s| s.instance_index).unwrap_or_default(),
            selection.as_ref().map(|s| s.is_layout).unwrap_or_default(),
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
        M::AdjustSelection { url, start_offset, end_offset, new_length } => {
            adjust_selection(url, start_offset, end_offset, new_length);
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
const NODE_IGNORE_COMMENT: &str = "@lsp:ignore-node";

/// Check whether a node is marked to be ignored in the LSP/live preview
/// using a comment containing `@lsp:ignore-node`
fn is_element_node_ignored(node: &Element) -> bool {
    node.children_with_tokens().any(|nt| {
        nt.as_token()
            .map(|t| t.kind() == SyntaxKind::Comment && t.text().contains(NODE_IGNORE_COMMENT))
            .unwrap_or(false)
    })
}
