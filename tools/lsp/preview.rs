// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{
    self, component_catalog, rename_component, ComponentInformation, ElementRcNode,
    PreviewComponent, PreviewConfig, PreviewToLspMessage, SourceFileVersion,
};
use crate::preview::element_selection::ElementSelection;
use crate::util;
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{syntax_nodes, TextSize};
use i_slint_compiler::{diagnostics, EmbedResourcesKind};
use i_slint_core::component_factory::FactoryContext;
use i_slint_core::lengths::{LogicalPoint, LogicalRect, LogicalSize};
use lsp_types::Url;
use slint::PlatformError;
use slint_interpreter::{ComponentDefinition, ComponentHandle, ComponentInstance};
use std::borrow::BorrowMut;
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
mod preview_data;
use ext::ElementRcNodeExt;
mod properties;
pub mod ui;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
mod wasm;
#[cfg(all(target_arch = "wasm32", feature = "preview-external"))]
pub use wasm::*;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
mod native;
#[cfg(all(not(target_arch = "wasm32"), feature = "preview-builtin"))]
pub use native::*;

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

#[derive(Default)]
struct ContentCache {
    source_code: SourceCodeCache,
    resources: HashSet<Url>,
    dependencies: HashSet<Url>,
    config: PreviewConfig,
    current_previewed_component: Option<PreviewComponent>,
    current_load_behavior: Option<LoadBehavior>,
    loading_state: PreviewFutureState,
    ui_is_visible: bool,
}

static CONTENT_CACHE: std::sync::OnceLock<Mutex<ContentCache>> = std::sync::OnceLock::new();

impl ContentCache {
    pub fn current_component(&self) -> Option<PreviewComponent> {
        self.current_previewed_component.clone()
    }

    pub fn set_current_component(&mut self, component: PreviewComponent) {
        self.current_previewed_component = Some(component);
    }

    pub fn clear_style_of_component(&mut self) {
        if let Some(pc) = &mut self.current_previewed_component {
            pc.style = String::new();
        }
    }

    pub fn rename_current_component(&mut self, url: &Url, old_name: &str, new_name: &str) {
        if let Some(pc) = &mut self.current_previewed_component {
            if pc.url == *url && pc.component.as_deref() == Some(old_name) {
                pc.component = Some(new_name.to_string());
            }
        }
    }
}

#[derive(Default)]
struct PreviewState {
    ui: Option<ui::PreviewUi>,
    property_range_declarations: Option<ui::PropertyDeclarations>,
    handle: Rc<RefCell<Option<slint_interpreter::ComponentInstance>>>,
    document_cache: Rc<RefCell<Option<Rc<common::DocumentCache>>>>,
    selected: Option<element_selection::ElementSelection>,
    notify_editor_about_selection_after_update: bool,
    workspace_edit_sent: bool,
    known_components: Vec<ComponentInformation>,
    preview_loading_delay_timer: Option<slint::Timer>,
    initial_live_data: preview_data::PreviewDataMap,
    current_live_data: preview_data::PreviewDataMap,
}

impl PreviewState {
    fn component_instance(&self) -> Option<ComponentInstance> {
        self.handle.borrow().as_ref().map(|ci| ci.clone_strong())
    }
}
thread_local! {static PREVIEW_STATE: std::cell::RefCell<PreviewState> = Default::default();}

pub fn poll_once<F: std::future::Future>(future: F) -> Option<F::Output> {
    struct DummyWaker();
    impl std::task::Wake for DummyWaker {
        fn wake(self: std::sync::Arc<Self>) {}
    }

    let waker = std::sync::Arc::new(DummyWaker()).into();
    let mut ctx = std::task::Context::from_waker(&waker);

    let future = std::pin::pin!(future);

    match future.poll(&mut ctx) {
        std::task::Poll::Ready(result) => Some(result),
        std::task::Poll::Pending => None,
    }
}

// Just mark the cache as "read from disk" by setting the version to None.
// Do not reset the code: We can check once the LSP has re-read it from disk
// whether we need to refresh the preview or not.
fn invalidate_contents(url: &lsp_types::Url) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if let Some(cache_entry) = cache.source_code.get_mut(url) {
        cache_entry.version = None;
    }
}

fn delete_document(url: &lsp_types::Url) {
    let (current, url_is_used, ui_is_visible) = {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.source_code.remove(url);
        (
            cache.current_previewed_component.clone(),
            cache.dependencies.contains(url),
            cache.ui_is_visible,
        )
    };

    if let Some(current) = current {
        if (&current.url == url || url_is_used) && ui_is_visible {
            // Trigger a compile error now!
            load_preview(current, LoadBehavior::Reload);
        }
    }
}

fn set_current_live_data(mut result: preview_data::PreviewDataMap) {
    PREVIEW_STATE.with(|preview_state| {
        let mut preview_state = preview_state.borrow_mut();
        preview_state.current_live_data.append(&mut result);
    })
}

fn apply_live_preview_data() {
    let Some(instance) = component_instance() else {
        return;
    };

    let new_initial_data = preview_data::query_preview_data_properties_and_callbacks(&instance);

    let (mut previous_initial, mut previous_current) = PREVIEW_STATE.with(|preview_state| {
        let mut preview_state = preview_state.borrow_mut();
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

fn set_contents(url: &common::VersionedUrl, content: String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let old = cache.source_code.insert(
        url.url().clone(),
        SourceCodeCacheEntry { version: *url.version(), code: content.clone() },
    );

    if Some(content) == old.map(|o| o.code) {
        return;
    }

    if cache.dependencies.contains(url.url()) {
        let ui_is_visible = cache.ui_is_visible;
        let Some(current) = cache.current_component() else {
            return;
        };

        drop(cache);

        if ui_is_visible {
            load_preview(current, LoadBehavior::Reload);
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
fn property_declaration_ranges(name: slint::SharedString) -> ui::PropertyDeclaration {
    let name = name.to_string();
    PREVIEW_STATE
        .with(|preview_state| {
            let preview_state = preview_state.borrow();

            preview_state
                .property_range_declarations
                .as_ref()
                .and_then(|d| d.get(name.as_str()).cloned())
        })
        .unwrap_or_default()
}

// triggered from the UI, running in UI thread
fn add_new_component() {
    fn find_component_name() -> Option<String> {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();

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

    let preview_component = {
        let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.current_component()
    };

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

        {
            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            cache.set_current_component(PreviewComponent {
                url: preview_component.url.clone(),
                component: Some(component_name.clone()),
                style: preview_component.style.clone(),
            })
        }

        send_workspace_edit(format!("Add {component_name}"), edit, true);
    }
}

/// Find the identifier that belongs to a component of the given `name` in the `document`
fn find_component_identifiers(
    document: &syntax_nodes::Document,
    name: &str,
) -> Vec<syntax_nodes::DeclaredIdentifier> {
    let name = Some(i_slint_compiler::parser::normalize_identifier(name));

    let mut result = vec![];
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

/// Find the last component in the `document`
pub fn find_last_component_identifier(
    document: &syntax_nodes::Document,
) -> Option<syntax_nodes::DeclaredIdentifier> {
    let last_identifier = {
        let mut tmp = None;
        for el in document.ExportsList() {
            if let Some(component) = el.Component() {
                tmp = Some(component.DeclaredIdentifier());
            }
        }
        tmp
    };

    if let Some(component) = document.Component().last() {
        let identifier = component.DeclaredIdentifier();
        if identifier.text_range().start()
            > last_identifier.as_ref().map(|i| i.text_range().start()).unwrap_or_default()
        {
            return Some(identifier);
        }
    }

    last_identifier
}

// triggered from the UI, running in UI thread
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
        // Update which component to show after refresh from the editor.
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.rename_current_component(&old_url, &old_name, &new_name);

        if let Some(current) = &mut cache.current_component() {
            if current.url == old_url {
                if let Some(component) = &current.component {
                    if component == &old_name {
                        current.component = Some(new_name.clone());
                    }
                }
            }
        }

        send_workspace_edit(format!("Rename component {old_name} to {new_name}"), edit, true);
    }
}

fn evaluate_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: String,
) -> Option<lsp_types::WorkspaceEdit> {
    let element_url = Url::parse(element_url.as_ref()).ok()?;
    let element_version = if element_version < 0 { None } else { Some(element_version) };
    let element_offset = u32::try_from(element_offset).ok()?.into();
    let property_name = property_name.to_string();

    let document_cache = document_cache()?;
    let element = document_cache.element_at_offset(&element_url, element_offset)?;

    let edit = if property_value.is_empty() {
        properties::remove_binding(element_url, element_version, &element, &property_name).ok()
    } else {
        properties::set_binding(
            element_url,
            element_version,
            &element,
            &property_name,
            property_value,
        )
    }?;

    drop_location::workspace_edit_compiles(&document_cache, &edit).then_some(edit)
}

// triggered from the UI, running in UI thread
fn test_code_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: slint::SharedString,
) -> bool {
    test_binding(
        element_url,
        element_version,
        element_offset,
        property_name,
        property_value.to_string(),
    )
}

// Backend function called by `test_*_binding`
fn test_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: String,
) -> bool {
    evaluate_binding(element_url, element_version, element_offset, property_name, property_value)
        .is_some()
}

fn set_code_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: slint::SharedString,
) {
    set_binding(
        element_url,
        element_version,
        element_offset,
        property_name,
        property_value.to_string(),
    )
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

    set_binding(
        element_url,
        element_version,
        element_offset,
        property_name,
        format!("#{value:08x}"),
    )
}

/// Internal function called by all the `set_*_binding` functions
fn set_binding(
    element_url: slint::SharedString,
    element_version: i32,
    element_offset: i32,
    property_name: slint::SharedString,
    property_value: String,
) {
    if let Some(edit) = evaluate_binding(
        element_url,
        element_version,
        element_offset,
        property_name,
        property_value,
    ) {
        send_workspace_edit("Edit property".to_string(), edit, false);
    }
}

// triggered from the UI, running in UI thread
fn show_component(name: slint::SharedString, url: slint::SharedString) {
    let name = name.to_string();
    let Ok(url) = Url::parse(url.as_ref()) else {
        return;
    };

    let Ok(file) = url.to_file_path() else {
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

    let start =
        util::text_size_to_lsp_position(&identifier.source_file, identifier.text_range().start());
    ask_editor_to_show_document(&file.to_string_lossy(), lsp_types::Range::new(start, start), false)
}

// triggered from the UI, running in UI thread
fn show_document_offset_range(url: slint::SharedString, start: i32, end: i32, take_focus: bool) {
    fn internal(
        url: slint::SharedString,
        start: i32,
        end: i32,
    ) -> Option<(PathBuf, lsp_types::Position, lsp_types::Position)> {
        let url = Url::parse(url.as_ref()).ok()?;
        let file = url.to_file_path().ok()?;

        let start = u32::try_from(start).ok()?;
        let end = u32::try_from(end).ok()?;

        let document_cache = document_cache()?;
        let document = document_cache.get_document(&url)?;
        let document = document.node.as_ref()?;

        let start = util::text_size_to_lsp_position(&document.source_file, start.into());
        let end = util::text_size_to_lsp_position(&document.source_file, end.into());

        Some((file, start, end))
    }

    if let Some((f, s, e)) = internal(url, start, end) {
        ask_editor_to_show_document(&f.to_string_lossy(), lsp_types::Range::new(s, e), take_focus);
    }
}

// triggered from the UI, running in UI thread
fn show_preview_for(name: slint::SharedString, url: slint::SharedString) {
    let name = name.to_string();
    let Ok(url) = Url::parse(url.as_ref()) else {
        return;
    };

    let current = PreviewComponent { url, component: Some(name), style: String::new() };

    load_preview(current, LoadBehavior::Load);
}

// triggered from the UI, running in UI thread
fn can_drop_component(component_index: i32, x: f32, y: f32, on_drop_area: bool) -> bool {
    if !on_drop_area {
        set_drop_mark(&None);
        return false;
    }

    let Some(document_cache) = document_cache() else {
        return false;
    };

    let position = LogicalPoint::new(x, y);

    PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow();

        if let Some(component) = preview_state.known_components.get(component_index as usize) {
            drop_location::can_drop_at(&document_cache, position, component)
        } else {
            false
        }
    })
}

// triggered from the UI, running in UI thread
fn drop_component(component_index: i32, x: f32, y: f32) {
    let Some(document_cache) = document_cache() else {
        return;
    };

    let position = LogicalPoint::new(x, y);

    let drop_result = PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow();

        let component = preview_state.known_components.get(component_index as usize)?;

        drop_location::drop_at(&document_cache, position, component)
            .map(|(e, d)| (e, d, component.name.clone()))
    });

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

fn placeholder_node_text(selected: &common::ElementRcNode) -> String {
    let Some(parent) = selected.parent() else {
        return Default::default();
    };

    if parent.layout_kind() != ui::LayoutKind::None && parent.children().len() == 1 {
        return format!("Rectangle {{ /* {} */ }}", common::NODE_IGNORE_COMMENT);
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
    let Some(cache_entry) = cache.source_code.get(&url) else {
        return;
    };

    let Some(selected_node) = selected.as_element_node() else {
        return;
    };

    let range = selected_node.with_decorated_node(|n| util::node_to_lsp_range(&n));

    // Insert a placeholder node into layouts if those end up empty:
    let new_text = placeholder_node_text(&selected_node);

    let edit = common::create_workspace_edit(
        url,
        cache_entry.version,
        vec![lsp_types::TextEdit { range, new_text }],
    );

    send_workspace_edit("Delete element".to_string(), edit, true);
}

// triggered from the UI, running in UI thread
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
        LogicalRect::new(LogicalPoint::new(x, y), LogicalSize::new(width, height)),
    ) else {
        return;
    };

    send_workspace_edit(label, edit, true);
}

fn resize_selected_element_impl(
    element_node: &ElementRcNode,
    instance_index: usize,
    rect: LogicalRect,
) -> Option<(lsp_types::WorkspaceEdit, String)> {
    let component_instance = component_instance()?;

    // They all have the same size anyway:
    let (path, offset) = element_node.path_and_offset();
    let geometry = element_node.geometries(&component_instance).get(instance_index).cloned()?;

    let position = rect.origin;
    let root_element = element_selection::root_element(&component_instance);

    let parent = search_for_parent_element(&root_element, &element_node.element)
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
            p.push(common::PropertyChange::new(
                "x",
                format!("{}px", (position.x - parent.x).round()),
            ));
            op = "Moving";
        }
        if geometry.origin.y != position.y && position.y.is_finite() {
            p.push(common::PropertyChange::new(
                "y",
                format!("{}px", (position.y - parent.y).round()),
            ));
            op = "Moving";
        }
        if geometry.size.width != rect.size.width && rect.size.width.is_finite() {
            p.push(common::PropertyChange::new("width", format!("{}px", rect.size.width.round())));
            op = "Resizing";
        }
        if geometry.size.height != rect.size.height && rect.size.height.is_finite() {
            p.push(common::PropertyChange::new(
                "height",
                format!("{}px", rect.size.height.round()),
            ));
            op = "Resizing";
        }
        (p, op)
    };

    if properties.is_empty() {
        return None;
    }

    let url = Url::from_file_path(&path).ok()?;
    let document_cache = document_cache()?;

    let version = document_cache.document_version(&url);

    properties::update_element_properties(
        &document_cache,
        common::VersionedPosition::new(common::VersionedUrl::new(url, version), offset),
        properties,
    )
    .map(|edit| (edit, format!("{op} element")))
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

    drop_location::can_move_to(
        &document_cache,
        position,
        mouse_position,
        selected_element_node,
        selected.instance_index,
    )
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
        selected.instance_index,
        position,
        mouse_position,
    ) {
        element_selection::select_element_at_source_code_position(
            drop_data.path,
            drop_data.selection_offset,
            None,
            SelectionNotification::AfterUpdate,
        );

        send_workspace_edit("Move element".to_string(), edit, false);
    } else {
        element_selection::reselect_element();
    }
}

fn test_workspace_edit(edit: &lsp_types::WorkspaceEdit, test_edit: bool) -> bool {
    if test_edit {
        let Some(document_cache) = document_cache() else {
            return false;
        };
        drop_location::workspace_edit_compiles(&document_cache, edit)
    } else {
        true
    }
}

fn send_workspace_edit(label: String, edit: lsp_types::WorkspaceEdit, test_edit: bool) -> bool {
    if !test_workspace_edit(&edit, test_edit) {
        return false;
    }

    let workspace_edit_sent = PREVIEW_STATE.with(|preview_state| {
        let mut ps = preview_state.borrow_mut();
        let result = ps.workspace_edit_sent;
        ps.workspace_edit_sent = true;
        result
    });

    if !workspace_edit_sent {
        send_message_to_lsp(PreviewToLspMessage::SendWorkspaceEdit { label: Some(label), edit });
        return true;
    }
    false
}

fn change_style() {
    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    let ui_is_visible = cache.ui_is_visible;
    let Some(current) = cache.current_component() else {
        return;
    };

    drop(cache);

    if ui_is_visible {
        load_preview(current, LoadBehavior::Reload);
    }
}

fn start_parsing() {
    set_status_text("Updating Preview...");
    PREVIEW_STATE.with(|preview_state| {
        let preview_state = preview_state.borrow_mut();

        if let Some(ui) = &preview_state.ui {
            ui::set_diagnostics(ui, &[]);
        }
    });
}

fn extract_resources(
    dependencies: &HashSet<Url>,
    component_instance: &ComponentInstance,
) -> HashSet<Url> {
    let tl = component_instance.definition().type_loader();

    let mut result: HashSet<Url> = Default::default();

    for d in dependencies {
        let Ok(path) = d.to_file_path() else {
            continue;
        };
        let Some(doc) = tl.get_document(&path) else {
            continue;
        };

        result.extend(
            doc.embedded_file_resources
                .borrow()
                .keys()
                .filter_map(|fp| Url::from_file_path(fp).ok()),
        );
    }

    result
}

fn finish_parsing(preview_url: &Url, previewed_component: Option<String>, success: bool) {
    set_status_text("");

    if !success {
        // No need to update everything...
        return;
    }

    let (previewed_url, component, source_code) = {
        let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        let pc = cache.current_component();
        (
            pc.as_ref().map(|pc| pc.url.clone()),
            pc.as_ref().and_then(|pc| pc.component.clone()),
            cache.source_code.clone(),
        )
    };

    if let Some(document_cache) = document_cache() {
        let mut document_cache = document_cache.snapshot().unwrap();

        for (url, cache_entry) in &source_code {
            let mut diag = diagnostics::BuildDiagnostics::default();
            if document_cache.get_document(url).is_none() {
                poll_once(document_cache.load_url(
                    url,
                    cache_entry.version,
                    cache_entry.code.clone(),
                    &mut diag,
                ));
            }
        }

        let uses_widgets = document_cache.uses_widgets(preview_url);

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

        let index = if let Some(component) = component {
            components
                .iter()
                .position(|ci| {
                    ci.name == component
                        && ci.defined_at.as_ref().map(|da| da.url()) == previewed_url.as_ref()
                })
                .unwrap_or(usize::MAX)
        } else {
            usize::MAX
        };

        apply_live_preview_data();

        PREVIEW_STATE.with(|preview_state| {
            let mut preview_state = preview_state.borrow_mut();
            preview_state.known_components = components;

            preview_state.document_cache.borrow_mut().replace(Some(Rc::new(document_cache)));

            let preview_data = preview_state
                .component_instance()
                .map(|component_instance| {
                    preview_data::query_preview_data_properties_and_callbacks(&component_instance)
                })
                .unwrap_or_default();

            if let Some(ui) = &preview_state.ui {
                ui::ui_set_uses_widgets(ui, uses_widgets);
                ui::ui_set_known_components(ui, &preview_state.known_components, index);
                ui::ui_set_preview_data(ui, preview_data, previewed_component);
            }
        });
    }

    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some(component_instance) = component_instance() {
        cache.resources = extract_resources(&cache.dependencies, &component_instance);
    } else {
        cache.resources.clear();
    }
}

fn config_changed(config: PreviewConfig) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

    if cache.config != config {
        cache.config = config.clone();

        let current = cache.current_component();
        let ui_is_visible = cache.ui_is_visible;
        let hide_ui = cache.config.hide_ui;

        drop(cache);

        if ui_is_visible {
            if let Some(hide_ui) = hide_ui {
                set_show_preview_ui(!hide_ui);
            }
            if let Some(current) = current {
                load_preview(current, LoadBehavior::Reload);
            }
        }
    }
}

/// If the file is in the cache, returns it.
///
/// If the file is not known, the return an empty string marked as "from disk". This is fine:
/// The LSP side will load the file and inform us about it soon.
///
/// In any way, register it as a dependency
fn get_url_from_cache(url: &Url) -> (SourceFileVersion, String) {
    let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    cache.dependencies.insert(url.to_owned());

    cache.source_code.get(url).map(|r| (r.version, r.code.clone())).unwrap_or_default().clone()
}

fn get_path_from_cache(path: &Path) -> std::io::Result<(SourceFileVersion, String)> {
    let url = Url::from_file_path(path).map_err(|()| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "Failed to convert path to URL")
    })?;
    Ok(get_url_from_cache(&url))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadBehavior {
    /// We reload the preview, most likely because a file has changed
    Reload,
    /// Load the preview and make the window visible if it wasn't already.
    Load,
    /// We show the preview because the user asked for it. The UI should become visible and focused if it wasn't already
    BringWindowToFront,
}

pub fn reload_preview() {
    let pc = {
        let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        cache.current_previewed_component.clone()
    };

    let Some(pc) = pc else {
        return;
    };

    load_preview(pc, LoadBehavior::Load);
}

async fn reload_timer_function() {
    let (selected, notify_editor) = PREVIEW_STATE.with(|preview_state| {
        let mut preview_state = preview_state.borrow_mut();
        let notify_editor = preview_state.notify_editor_about_selection_after_update;
        preview_state.notify_editor_about_selection_after_update = false;
        (preview_state.selected.take(), notify_editor)
    });

    loop {
        let (preview_component, config, behavior) = {
            let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
            let Some(behavior) = cache.current_load_behavior.take() else { return };

            let Some(preview_component) = cache.current_component() else {
                return;
            };
            cache.clear_style_of_component();

            assert_eq!(cache.loading_state, PreviewFutureState::PreLoading);

            if !cache.ui_is_visible && behavior == LoadBehavior::Reload {
                cache.loading_state = PreviewFutureState::Pending;
                return;
            }
            cache.loading_state = PreviewFutureState::Loading;
            cache.dependencies.clear();
            (preview_component, cache.config.clone(), behavior)
        };
        let style = if preview_component.style.is_empty() {
            get_current_style()
        } else {
            set_current_style(preview_component.style.clone());
            preview_component.style.clone()
        };

        match reload_preview_impl(preview_component, behavior, style, config).await {
            Ok(()) => {}
            Err(e) => {
                CONTENT_CACHE.get_or_init(Default::default).lock().unwrap().loading_state =
                    PreviewFutureState::Pending;
                send_platform_error_notification(&e.to_string());
                return;
            }
        }

        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
        match cache.loading_state {
            PreviewFutureState::Loading => {
                cache.loading_state = PreviewFutureState::Pending;
                break;
            }
            PreviewFutureState::NeedsReload => {
                cache.loading_state = PreviewFutureState::PreLoading;
                continue;
            }
            PreviewFutureState::Pending | PreviewFutureState::PreLoading => unreachable!(),
        };
    }

    if let Some(se) = selected {
        element_selection::select_element_at_source_code_position(
            se.path.clone(),
            se.offset,
            None,
            SelectionNotification::Never,
        );

        if notify_editor {
            if let Some(component_instance) = component_instance() {
                if let Some((element, debug_index)) = component_instance
                    .element_node_at_source_code_position(&se.path, se.offset.into())
                    .first()
                {
                    let Some(element_node) = ElementRcNode::new(element.clone(), *debug_index)
                    else {
                        return;
                    };
                    let (path, pos) = element_node.with_element_node(|node| {
                        let sf = &node.source_file;
                        (sf.path().to_owned(), util::text_size_to_lsp_position(sf, se.offset))
                    });
                    ask_editor_to_show_document(
                        &path.to_string_lossy(),
                        lsp_types::Range::new(pos, pos),
                        false,
                    );
                }
            }
        }
    }
}

pub fn load_preview(preview_component: PreviewComponent, behavior: LoadBehavior) {
    {
        let mut cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

        match behavior {
            LoadBehavior::Reload => {
                if !cache.ui_is_visible {
                    return;
                }
            }
            LoadBehavior::Load | LoadBehavior::BringWindowToFront => {
                cache.set_current_component(preview_component)
            }
        }

        cache.current_load_behavior = Some(behavior);

        match cache.loading_state {
            PreviewFutureState::Pending => {}
            PreviewFutureState::Loading => {
                cache.loading_state = PreviewFutureState::NeedsReload;
                return;
            }
            PreviewFutureState::NeedsReload | PreviewFutureState::PreLoading => {
                return;
            }
        }
        cache.loading_state = PreviewFutureState::PreLoading;
    };

    if let Err(e) = run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            preview_state
                .borrow_mut()
                .preview_loading_delay_timer
                .get_or_insert_with(|| {
                    let timer = slint::Timer::default();
                    timer.start(
                        slint::TimerMode::SingleShot,
                        core::time::Duration::from_millis(50),
                        || {
                            let _ = slint::spawn_local(reload_timer_function());
                        },
                    );
                    timer
                })
                .restart();
        });
    }) {
        send_platform_error_notification(&e);
    }
}

async fn parse_source(
    include_paths: Vec<PathBuf>,
    library_paths: HashMap<String, PathBuf>,
    path: PathBuf,
    version: common::SourceFileVersion,
    source_code: String,
    style: String,
    component: Option<String>,
    file_loader_fallback: impl Fn(
            String,
        ) -> core::pin::Pin<
            Box<
                dyn core::future::Future<
                    Output = Option<std::io::Result<(common::SourceFileVersion, String)>>,
                >,
            >,
        > + 'static,
) -> (
    Vec<diagnostics::Diagnostic>,
    Option<ComponentDefinition>,
    common::document_cache::OpenImportFallback,
    Rc<RefCell<common::document_cache::SourceFileVersionMap>>,
) {
    let mut builder = slint_interpreter::Compiler::default();

    let cc = builder.compiler_configuration(i_slint_core::InternalToken);
    cc.components_to_generate = if let Some(name) = component {
        i_slint_compiler::ComponentSelection::Named(name)
    } else {
        i_slint_compiler::ComponentSelection::LastExported
    };
    #[cfg(target_arch = "wasm32")]
    {
        cc.resource_url_mapper = resource_url_mapper();
    }
    cc.embed_resources = EmbedResourcesKind::ListAllResources;

    if !style.is_empty() {
        cc.style = Some(style);
    }
    cc.include_paths = include_paths;
    cc.library_paths = library_paths;

    let (open_file_fallback, source_file_versions) =
        common::document_cache::document_cache_parts_setup(
            cc,
            Some(Rc::new(file_loader_fallback)),
            common::document_cache::SourceFileVersionMap::from([(path.clone(), version)]),
        );

    let result = builder.build_from_source(source_code, path).await;

    let compiled = result.components().next();
    (result.diagnostics().collect(), compiled, open_file_fallback, source_file_versions)
}

// Must be inside the thread running the slint event loop
async fn reload_preview_impl(
    component: PreviewComponent,
    behavior: LoadBehavior,
    style: String,
    config: PreviewConfig,
) -> Result<(), PlatformError> {
    start_parsing();

    if let Some(component_instance) = component_instance() {
        let live_preview_data =
            preview_data::query_preview_data_properties_and_callbacks(&component_instance);
        set_current_live_data(live_preview_data);
    }

    let path = component.url.to_file_path().unwrap_or(PathBuf::from(&component.url.to_string()));
    let (version, source) = get_url_from_cache(&component.url);

    let (diagnostics, compiled, open_import_fallback, source_file_versions) = parse_source(
        config.include_paths,
        config.library_paths,
        path,
        version,
        source,
        style,
        component.component.clone(),
        move |path| {
            let path = path.to_owned();
            Box::pin(async move {
                let path = PathBuf::from(&path);
                // Always return Some to stop the compiler from trying to load itself...
                // All loading is done by the LSP for us!
                Some(get_path_from_cache(&path))
            })
        },
    )
    .await;

    let success = compiled.is_some();

    let loaded_component_name = compiled.as_ref().map(|c| c.name().to_string());

    {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow_mut();

            if let Some(ui) = &preview_state.ui {
                ui::set_diagnostics(ui, &diagnostics);
            }
        });
        let diags = convert_diagnostics(&diagnostics, &source_file_versions.borrow());
        notify_diagnostics(diags);
    }

    update_preview_area(compiled, behavior, open_import_fallback, source_file_versions)?;

    finish_parsing(&component.url, loaded_component_name, success);
    Ok(())
}

/// Sends a notification back to the editor when the preview fails to load because of a slint::PlatformError.
fn send_platform_error_notification(platform_error_str: &str) {
    let message = format!("Error displaying the Slint preview window: {platform_error_str}");
    // Also output the message in the console in case the user missed the notification in the editor
    eprintln!("{message}");
    send_message_to_lsp(PreviewToLspMessage::SendShowMessage {
        message: lsp_types::ShowMessageParams { typ: lsp_types::MessageType::ERROR, message },
    })
}

/// This sets up the preview area to show the ComponentInstance
///
/// This must be run in the UI thread.
fn set_preview_factory(
    ui: &ui::PreviewUi,
    compiled: ComponentDefinition,
    callback: Box<dyn Fn(ComponentInstance)>,
    behavior: LoadBehavior,
) {
    // Ensure that any popups are closed as they are related to the old factory
    i_slint_core::window::WindowInner::from_pub(ui.window()).close_all_popups();

    let factory = slint::ComponentFactory::new(move |ctx: FactoryContext| {
        let instance = compiled.create_embedded(ctx).unwrap();

        callback(instance.clone_strong());

        Some(instance)
    });

    let api = ui.global::<ui::Api>();
    api.set_preview_area(factory);
    api.set_resize_to_preferred_size(behavior != LoadBehavior::Reload);
}

/// Highlight the element pointed at the offset in the path.
/// When path is None, remove the highlight.
pub fn highlight(url: Option<Url>, offset: TextSize) {
    let Some(path) = url.as_ref().and_then(|u| Url::to_file_path(u).ok()) else {
        return;
    };

    let selected = selected_element();

    if let Some(selected) = &selected {
        if selected.path == path && selected.offset == offset {
            return;
        }
    }

    let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
    if url.as_ref().is_none_or(|url| cache.dependencies.contains(url)) {
        let _ = run_in_ui_thread(move || async move {
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
        });
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
    file_versions: &common::document_cache::SourceFileVersionMap,
) -> HashMap<Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)> {
    let mut result: HashMap<Url, (SourceFileVersion, Vec<lsp_types::Diagnostic>)> =
        Default::default();

    fn path_to_url(path: &Path) -> Url {
        Url::from_file_path(path).ok().unwrap_or_else(|| Url::parse("file:/unknown").unwrap())
    }

    // Pre-fill version info and an empty diagnostics to reset the state for the url
    for (path, version) in file_versions.iter() {
        result.insert(path_to_url(path), (*version, Vec::new()));
    }

    {
        let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();

        // Fill in actual diagnostics now
        for d in diagnostics {
            if d.source_file().is_none_or(|f| !i_slint_compiler::pathutils::is_absolute(f)) {
                continue;
            }
            let uri = path_to_url(d.source_file().unwrap());
            let new_version = cache.source_code.get(&uri).and_then(|e| e.version);
            if let Some(data) = result.get_mut(&uri) {
                if data.0.is_some() && new_version.is_some() && data.0 != new_version {
                    continue;
                }
                data.1.push(crate::util::to_lsp_diag(d));
            }
        }
    }

    result
}

fn reset_selections(ui: &ui::PreviewUi) {
    let model = Rc::new(slint::VecModel::from(Vec::new()));
    let api = ui.global::<ui::Api>();
    api.set_selections(slint::ModelRc::from(model));
}

fn set_selections(
    ui: Option<&ui::PreviewUi>,
    main_index: usize,
    layout_kind: ui::LayoutKind,
    is_interactive: bool,
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
            is_interactive,
            is_moveable,
            is_resizable,
        })
        .collect::<Vec<_>>();
    let model = Rc::new(slint::VecModel::from(values));
    let api = ui.global::<ui::Api>();
    api.set_selections(slint::ModelRc::from(model));
}

fn set_drop_mark(mark: &Option<drop_location::DropMark>) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow();

        let Some(ui) = &preview_state.ui else {
            return;
        };

        let api = ui.global::<ui::Api>();
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
    selection: Option<element_selection::ElementSelection>,
    positions: &[i_slint_core::lengths::LogicalRect],
    editor_notification: SelectionNotification,
) {
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
    PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();

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

        set_selections(
            preview_state.ui.as_ref(),
            selection.as_ref().map(|s| s.instance_index).unwrap_or_default(),
            layout_kind,
            is_interactive,
            true,
            !is_in_layout && !is_layout,
            positions,
        );

        if let Some(ui) = &preview_state.ui {
            if let Some(document_cache) = document_cache_from(&preview_state) {
                if let Some((uri, version, selection)) = selection
                    .clone()
                    .or_else(|| {
                        let current = {
                            let cache = CONTENT_CACHE.get_or_init(Default::default).lock().unwrap();
                            cache.current_component()
                        }?;

                        let document = document_cache.get_document(&current.url)?;
                        let document = document.node.as_ref()?;

                        let identifier = if let Some(name) = &current.component {
                            find_component_identifiers(document, name).last().cloned()
                        } else {
                            find_last_component_identifier(document)
                        }?;

                        let path = identifier.source_file.path().to_path_buf();
                        let offset = identifier.text_range().start();

                        Some(ElementSelection { path, offset, instance_index: 0 })
                    })
                    .as_ref()
                    .and_then(|selection| {
                        let url = Url::from_file_path(&selection.path).ok()?;
                        let version = document_cache.document_version(&url);
                        Some((
                            url.clone(),
                            version,
                            document_cache.element_at_offset(&url, selection.offset)?,
                        ))
                    })
                {
                    let in_layout = match parent_layout_kind {
                        ui::LayoutKind::None => properties::LayoutKind::None,
                        ui::LayoutKind::Horizontal => properties::LayoutKind::HorizontalBox,
                        ui::LayoutKind::Vertical => properties::LayoutKind::VerticalBox,
                        ui::LayoutKind::Grid => properties::LayoutKind::GridLayout,
                    };
                    preview_state.property_range_declarations = Some(ui::ui_set_properties(
                        ui,
                        &document_cache,
                        properties::query_properties(&uri, version, &selection, in_layout).ok(),
                    ));
                }
            }
        }

        preview_state.selected = selection;
        preview_state.notify_editor_about_selection_after_update =
            notify_editor_about_selection_after_update;
    });

    if editor_notification == SelectionNotification::Now {
        if let Some(element_node) = element_node {
            let (path, pos) = element_node.with_element_node(|node| {
                let sf = &node.source_file;
                (
                    sf.path().to_owned(),
                    util::text_size_to_lsp_position(sf, node.text_range().start()),
                )
            });
            ask_editor_to_show_document(
                &path.to_string_lossy(),
                lsp_types::Range::new(pos, pos),
                false,
            );
        }
    }
}

fn selected_element() -> Option<ElementSelection> {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow();
        preview_state.selected.clone()
    })
}

fn component_instance() -> Option<ComponentInstance> {
    PREVIEW_STATE.with(move |preview_state| preview_state.borrow().component_instance())
}

/// This is a *read-only* snapshot of the raw type loader, use this when you
/// need to know the exact state the compiled resources were in.
fn document_cache() -> Option<Rc<common::DocumentCache>> {
    PREVIEW_STATE.with(move |preview_state| document_cache_from(&preview_state.borrow()))
}

/// This is a *read-only* snapshot of the raw type loader, use this when you
/// need to know the exact state the compiled resources were in.
fn document_cache_from(preview_state: &PreviewState) -> Option<Rc<common::DocumentCache>> {
    preview_state.document_cache.borrow().as_ref().map(|dc| dc.clone())
}

fn set_show_preview_ui(show_preview_ui: bool) {
    let _ = run_in_ui_thread(move || async move {
        PREVIEW_STATE.with(|preview_state| {
            let preview_state = preview_state.borrow();
            if let Some(ui) = &preview_state.ui {
                let api = ui.global::<ui::Api>();
                api.set_show_preview_ui(show_preview_ui)
            }
        })
    });
}

fn set_current_style(style: String) {
    PREVIEW_STATE.with(move |preview_state| {
        let preview_state = preview_state.borrow_mut();
        if let Some(ui) = &preview_state.ui {
            let api = ui.global::<ui::Api>();
            api.set_current_style(style.into())
        }
    });
}

fn get_current_style() -> String {
    PREVIEW_STATE.with(|preview_state| -> String {
        let preview_state = preview_state.borrow();
        if let Some(ui) = &preview_state.ui {
            let api = ui.global::<ui::Api>();
            api.get_current_style().as_str().to_string()
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
                let api = ui.global::<ui::Api>();
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
    open_import_fallback: common::document_cache::OpenImportFallback,
    source_file_versions: Rc<RefCell<common::document_cache::SourceFileVersionMap>>,
) -> Result<(), PlatformError> {
    PREVIEW_STATE.with(move |preview_state| {
        let mut preview_state = preview_state.borrow_mut();
        preview_state.workspace_edit_sent = false;

        #[cfg(not(target_arch = "wasm32"))]
        native::open_ui_impl(&mut preview_state)?;

        let ui = preview_state.ui.as_ref().unwrap();
        let shared_handle = preview_state.handle.clone();
        let shared_document_cache = preview_state.document_cache.clone();

        if let Some(compiled) = compiled {
            let api = ui.global::<ui::Api>();
            api.set_focus_previewed_element(behavior == LoadBehavior::BringWindowToFront);

            set_preview_factory(
                ui,
                compiled,
                Box::new(move |instance| {
                    if let Some(rtl) = instance.definition().raw_type_loader() {
                        shared_document_cache.replace(Some(Rc::new(
                            common::DocumentCache::new_from_raw_parts(
                                rtl,
                                open_import_fallback.clone(),
                                source_file_versions.clone(),
                            ),
                        )));
                    }

                    shared_handle.replace(Some(instance));
                }),
                behavior,
            );
            reset_selections(ui);
        }

        ui.show().and_then(|_| {
            if matches!(behavior, LoadBehavior::BringWindowToFront) {
                let window_inner = i_slint_core::window::WindowInner::from_pub(ui.window());
                if let Some(window_adapter_internal) =
                    window_inner.window_adapter().internal(i_slint_core::InternalToken)
                {
                    window_adapter_internal.bring_to_front()?;
                }
            }

            Ok(())
        })
    })?;

    element_selection::reselect_element();
    Ok(())
}

pub fn lsp_to_preview_message(message: crate::common::LspToPreviewMessage) {
    use crate::common::LspToPreviewMessage as M;
    match message {
        M::InvalidateContents { url } => invalidate_contents(&url),
        M::ForgetFile { url } => delete_document(&url),
        M::SetContents { url, contents } => {
            set_contents(&url, contents);
        }
        M::SetConfiguration { config } => {
            config_changed(config);
        }
        M::ShowPreview(pc) => {
            load_preview(pc, LoadBehavior::BringWindowToFront);
        }
        M::HighlightFromEditor { url, offset } => {
            highlight(url, offset.into());
        }
    }
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
        let (diagnostics, component_definition, _, _) = spin_on::spin_on(super::parse_source(
            vec![],
            std::collections::HashMap::new(),
            path,
            Some(24),
            source_code.to_string(),
            style.to_string(),
            None,
            move |path| {
                let code = code.clone();
                let path = PathBuf::from(&path);

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
