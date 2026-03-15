// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Shared introspection state and logic used by both the systest (protobuf) and
//! MCP (HTTP/JSON-RPC) transports.

use i_slint_core::item_tree::ItemTreeRc;
use i_slint_core::window::WindowAdapter;
use i_slint_core::window::WindowInner;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};

use crate::{ElementHandle, ElementRoot, LayoutKind};

/// Maximum number of element handles kept in the arena before evicting the oldest.
const ELEMENT_HANDLE_CAP: usize = 10_000;

thread_local! {
    static SHARED_STATE: RefCell<Option<Rc<IntrospectionState>>> = const { RefCell::new(None) };
    static HOOK_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Returns the shared introspection state, creating it if needed.
/// Both `systest` and `mcp_server` use this to share window/element tracking.
pub(crate) fn shared_state() -> Rc<IntrospectionState> {
    SHARED_STATE.with(|s| {
        let mut borrow = s.borrow_mut();
        if let Some(state) = borrow.as_ref() {
            return state.clone();
        }
        let state = Rc::new(IntrospectionState::new());
        *borrow = Some(state.clone());
        state
    })
}

/// Ensures the window-shown hook is installed for window tracking.
/// Safe to call multiple times — only installs once.
/// Chains with any previously installed hook.
///
/// **Init order**: Both `systest::init()` and `mcp_server::init()` call this function
/// first, then layer their own hooks on top via the same capture-and-chain pattern.
/// The current call order (from `selector/lib.rs`) is: systest first, then mcp_server.
/// This order is safe because `ensure_window_tracking` is idempotent and each subsequent
/// caller chains onto the previous hook. Changing the init order in the selector is safe
/// as long as `ensure_window_tracking()` is always called before installing additional hooks.
pub(crate) fn ensure_window_tracking() -> Result<(), i_slint_core::api::EventLoopError> {
    HOOK_INSTALLED.with(|installed| {
        if installed.get() {
            return Ok(());
        }
        installed.set(true);

        let state = shared_state();

        // Capture and chain any existing hook (e.g. from systest if it ran first)
        let previous_hook = i_slint_core::context::set_window_shown_hook(None)
            .map_err(|_| i_slint_core::api::EventLoopError::NoEventLoopProvider)?;
        let previous_hook = RefCell::new(previous_hook);

        i_slint_core::context::set_window_shown_hook(Some(Box::new(move |adapter| {
            if let Some(prev) = previous_hook.borrow_mut().as_mut() {
                prev(adapter);
            }
            state.add_window(adapter);
        })))
        .map_err(|_| i_slint_core::api::EventLoopError::NoEventLoopProvider)?;

        Ok(())
    })
}

pub(crate) struct RootWrapper<'a>(pub &'a ItemTreeRc);

impl ElementRoot for RootWrapper<'_> {
    fn item_tree(&self) -> ItemTreeRc {
        self.0.clone()
    }
}

impl super::Sealed for RootWrapper<'_> {}

/// A tracked window with its adapter and cached root element handle.
pub(crate) struct TrackedWindow {
    pub window_adapter: Weak<dyn WindowAdapter>,
    pub root_element_handle: generational_arena::Index,
}

/// Shared introspection state: window and element handle arenas.
///
/// Used by both `systest` (protobuf over TCP) and `mcp_server` (JSON-RPC over HTTP).
pub(crate) struct IntrospectionState {
    pub windows: RefCell<generational_arena::Arena<TrackedWindow>>,
    pub element_handles: RefCell<generational_arena::Arena<ElementHandle>>,
    /// Insertion-order queue for FIFO eviction. Front = oldest.
    element_handle_order: RefCell<VecDeque<generational_arena::Index>>,
}

impl IntrospectionState {
    pub fn new() -> Self {
        Self {
            windows: Default::default(),
            element_handles: Default::default(),
            element_handle_order: Default::default(),
        }
    }

    pub fn add_window(&self, adapter: &Rc<dyn WindowAdapter>) {
        let root_element_handle = {
            let window = adapter.window();
            let item_tree = WindowInner::from_pub(window).component();
            let root_wrapper = RootWrapper(&item_tree);
            self.element_to_handle(root_wrapper.root_element())
        };
        self.windows
            .borrow_mut()
            .insert(TrackedWindow { window_adapter: Rc::downgrade(adapter), root_element_handle });
    }

    pub fn window_handles(&self) -> Vec<generational_arena::Index> {
        self.windows.borrow().iter().map(|(index, _)| index).collect()
    }

    pub fn window_adapter(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<Rc<dyn WindowAdapter>, String> {
        self.windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .window_adapter
            .upgrade()
            .ok_or_else(|| "Attempting to access deleted window".to_string())
    }

    pub fn root_element_handle(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<generational_arena::Index, String> {
        Ok(self
            .windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .root_element_handle)
    }

    pub fn element_to_handle(&self, element: ElementHandle) -> generational_arena::Index {
        let mut arena = self.element_handles.borrow_mut();
        let index = arena.insert(element);
        let mut order = self.element_handle_order.borrow_mut();
        order.push_back(index);
        // Evict oldest handles when over cap, skipping root element handles.
        if arena.len() > ELEMENT_HANDLE_CAP {
            // Collect root indices upfront to avoid borrowing self.windows per iteration.
            let root_indices: std::collections::HashSet<generational_arena::Index> =
                self.windows.borrow().iter().map(|(_, w)| w.root_element_handle).collect();
            // Budget prevents infinite spinning when only root/stale entries remain.
            // Note: arena may remain above cap by at most the number of tracked windows.
            let mut budget = order.len();
            while arena.len() > ELEMENT_HANDLE_CAP && budget > 0 {
                budget -= 1;
                let Some(oldest) = order.pop_front() else { break };
                if !arena.contains(oldest) {
                    continue; // stale entry — don't re-enqueue
                }
                if root_indices.contains(&oldest) {
                    order.push_back(oldest);
                    continue;
                }
                arena.remove(oldest);
            }
        }
        index
    }

    /// Remove a set of element handles from the arena (used for scoped/ephemeral handles).
    #[allow(dead_code)]
    pub fn remove_handles(&self, handles: &[generational_arena::Index]) {
        let mut arena = self.element_handles.borrow_mut();
        for &h in handles {
            arena.remove(h);
        }
        // No need to clean up element_handle_order — stale entries are
        // harmless (remove on an already-removed index is a no-op).
    }

    pub fn element(
        &self,
        request: &str,
        index: generational_arena::Index,
    ) -> Result<ElementHandle, String> {
        let element = self
            .element_handles
            .borrow()
            .get(index)
            .ok_or_else(|| format!("Invalid element handle for {request}"))?
            .clone();
        if !element.is_valid() {
            self.element_handles.borrow_mut().remove(index);
            return Err(format!(
                "Element handle for {request} refers to element that was destroyed"
            ));
        }
        Ok(element)
    }

    pub fn find_elements_by_id(
        &self,
        window_index: generational_arena::Index,
        elements_id: &str,
    ) -> Result<Vec<ElementHandle>, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        let item_tree = WindowInner::from_pub(window).component();
        Ok(ElementHandle::find_by_element_id(&RootWrapper(&item_tree), elements_id)
            .collect::<Vec<_>>())
    }

    pub fn element_properties(&self, element: &ElementHandle) -> ElementProperties {
        let type_names_and_ids: Vec<(String, String)> = core::iter::once((
            element.type_name().unwrap_or_default().to_string(),
            element.id().unwrap_or_default().to_string(),
        ))
        .chain(
            element
                .bases()
                .into_iter()
                .flatten()
                .map(|base_type_name| (base_type_name.to_string(), "root".to_string())),
        )
        .collect();

        ElementProperties {
            type_names_and_ids,
            accessible_role: accessible_role_to_string(
                element.accessible_role().unwrap_or(i_slint_core::items::AccessibleRole::None),
            ),
            accessible_label: element.accessible_label().map(|s| s.to_string()),
            accessible_value: non_empty(element.accessible_value().unwrap_or_default().to_string()),
            accessible_description: non_empty(
                element.accessible_description().unwrap_or_default().to_string(),
            ),
            accessible_placeholder_text: non_empty(
                element.accessible_placeholder_text().unwrap_or_default().to_string(),
            ),
            accessible_checked: element.accessible_checked().unwrap_or_default(),
            accessible_checkable: element.accessible_checkable().unwrap_or_default(),
            accessible_enabled: element.accessible_enabled().unwrap_or_default(),
            accessible_read_only: element.accessible_read_only().unwrap_or_default(),
            accessible_value_minimum: element.accessible_value_minimum().unwrap_or_default(),
            accessible_value_maximum: element.accessible_value_maximum().unwrap_or_default(),
            accessible_value_step: element.accessible_value_step().unwrap_or_default(),
            size: element.size(),
            absolute_position: element.absolute_position(),
            computed_opacity: element.computed_opacity(),
            layout_kind: element.layout_kind(),
        }
    }

    pub fn query_element_descendants(
        &self,
        element: ElementHandle,
        instructions: Vec<QueryInstruction>,
        find_all: bool,
    ) -> Result<Vec<ElementHandle>, String> {
        let mut query = element.query_descendants();
        for instruction in instructions {
            match instruction {
                QueryInstruction::MatchDescendants => {
                    query = query.match_descendants();
                }
                QueryInstruction::MatchId(id) => {
                    query = query.match_id(id);
                }
                QueryInstruction::MatchTypeName(name) => {
                    query = query.match_type_name(name);
                }
                QueryInstruction::MatchTypeNameOrBase(name) => {
                    query = query.match_inherits(name);
                }
                QueryInstruction::MatchAccessibleRole(role) => {
                    query = query.match_accessible_role(role);
                }
            }
        }
        Ok(if find_all { query.find_all() } else { query.find_first().into_iter().collect() })
    }

    /// Take a screenshot of a window, encoding it in the given MIME type.
    /// Pass an empty string or "image/png" for PNG (the default).
    pub fn take_snapshot(
        &self,
        window_index: generational_arena::Index,
        image_mime_type: &str,
    ) -> Result<Vec<u8>, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        let buffer =
            window.take_snapshot().map_err(|e| format!("Error grabbing window screenshot: {e}"))?;
        let format = if image_mime_type.is_empty() {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::from_mime_type(image_mime_type).ok_or_else(|| {
                format!(
                    "Unsupported image format {image_mime_type} requested for window snapshotting"
                )
            })?
        };
        let mut encoded: Vec<u8> = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut encoded);
        image::write_buffer_with_format(
            &mut cursor,
            buffer.as_bytes(),
            buffer.width(),
            buffer.height(),
            image::ExtendedColorType::Rgba8,
            format,
        )
        .map_err(|e| format!("error encoding {image_mime_type} image after screenshot: {e}"))?;
        Ok(encoded)
    }

    pub fn dispatch_window_event(
        &self,
        window_index: generational_arena::Index,
        event: i_slint_core::platform::WindowEvent,
    ) -> Result<(), String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        window.dispatch_event(event);
        Ok(())
    }

    pub fn window_properties(
        &self,
        window_index: generational_arena::Index,
    ) -> Result<WindowProperties, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        Ok(WindowProperties {
            is_fullscreen: window.is_fullscreen(),
            is_maximized: window.is_maximized(),
            is_minimized: window.is_minimized(),
            size: window.size(),
            position: window.position(),
            root_element_handle: self.root_element_handle(window_index)?,
        })
    }

    pub fn invoke_element_accessibility_action(
        &self,
        element: &ElementHandle,
        action: AccessibilityAction,
    ) -> Result<(), String> {
        match action {
            AccessibilityAction::Default => element.invoke_accessible_default_action(),
            AccessibilityAction::Increment => element.invoke_accessible_increment_action(),
            AccessibilityAction::Decrement => element.invoke_accessible_decrement_action(),
            AccessibilityAction::Expand => element.invoke_accessible_expand_action(),
        }
        Ok(())
    }
}

fn non_empty(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

// ============================================================================
// Transport-independent types
// ============================================================================

pub(crate) struct WindowProperties {
    pub is_fullscreen: bool,
    pub is_maximized: bool,
    pub is_minimized: bool,
    pub size: i_slint_core::api::PhysicalSize,
    pub position: i_slint_core::api::PhysicalPosition,
    pub root_element_handle: generational_arena::Index,
}

pub(crate) struct ElementProperties {
    pub type_names_and_ids: Vec<(String, String)>,
    pub accessible_role: &'static str,
    pub accessible_label: Option<String>,
    pub accessible_value: Option<String>,
    pub accessible_description: Option<String>,
    pub accessible_placeholder_text: Option<String>,
    pub accessible_checked: bool,
    pub accessible_checkable: bool,
    pub accessible_enabled: bool,
    pub accessible_read_only: bool,
    pub accessible_value_minimum: f32,
    pub accessible_value_maximum: f32,
    pub accessible_value_step: f32,
    pub size: i_slint_core::api::LogicalSize,
    pub absolute_position: i_slint_core::api::LogicalPosition,
    pub computed_opacity: f32,
    pub layout_kind: Option<LayoutKind>,
}

pub(crate) enum QueryInstruction {
    MatchDescendants,
    MatchId(String),
    MatchTypeName(String),
    MatchTypeNameOrBase(String),
    MatchAccessibleRole(i_slint_core::items::AccessibleRole),
}

pub(crate) enum AccessibilityAction {
    Default,
    Increment,
    Decrement,
    Expand,
}

pub(crate) fn accessible_role_to_string(role: i_slint_core::items::AccessibleRole) -> &'static str {
    match role {
        i_slint_core::items::AccessibleRole::None => "unknown",
        i_slint_core::items::AccessibleRole::Button => "button",
        i_slint_core::items::AccessibleRole::Checkbox => "checkbox",
        i_slint_core::items::AccessibleRole::Combobox => "combobox",
        i_slint_core::items::AccessibleRole::List => "list",
        i_slint_core::items::AccessibleRole::Slider => "slider",
        i_slint_core::items::AccessibleRole::Spinbox => "spinbox",
        i_slint_core::items::AccessibleRole::Tab => "tab",
        i_slint_core::items::AccessibleRole::TabList => "tab-list",
        i_slint_core::items::AccessibleRole::Text => "text",
        i_slint_core::items::AccessibleRole::Table => "table",
        i_slint_core::items::AccessibleRole::Tree => "tree",
        i_slint_core::items::AccessibleRole::ProgressIndicator => "progress-indicator",
        i_slint_core::items::AccessibleRole::TextInput => "text-input",
        i_slint_core::items::AccessibleRole::Switch => "switch",
        i_slint_core::items::AccessibleRole::ListItem => "list-item",
        i_slint_core::items::AccessibleRole::TabPanel => "tab-panel",
        i_slint_core::items::AccessibleRole::Groupbox => "groupbox",
        i_slint_core::items::AccessibleRole::Image => "image",
        i_slint_core::items::AccessibleRole::RadioButton => "radio-button",
        _ => "unknown",
    }
}

pub(crate) fn string_to_accessible_role(s: &str) -> Option<i_slint_core::items::AccessibleRole> {
    Some(match s {
        "unknown" => i_slint_core::items::AccessibleRole::None,
        "button" => i_slint_core::items::AccessibleRole::Button,
        "checkbox" => i_slint_core::items::AccessibleRole::Checkbox,
        "combobox" => i_slint_core::items::AccessibleRole::Combobox,
        "list" => i_slint_core::items::AccessibleRole::List,
        "slider" => i_slint_core::items::AccessibleRole::Slider,
        "spinbox" => i_slint_core::items::AccessibleRole::Spinbox,
        "tab" => i_slint_core::items::AccessibleRole::Tab,
        "tab-list" => i_slint_core::items::AccessibleRole::TabList,
        "text" => i_slint_core::items::AccessibleRole::Text,
        "table" => i_slint_core::items::AccessibleRole::Table,
        "tree" => i_slint_core::items::AccessibleRole::Tree,
        "progress-indicator" => i_slint_core::items::AccessibleRole::ProgressIndicator,
        "text-input" => i_slint_core::items::AccessibleRole::TextInput,
        "switch" => i_slint_core::items::AccessibleRole::Switch,
        "list-item" => i_slint_core::items::AccessibleRole::ListItem,
        "tab-panel" => i_slint_core::items::AccessibleRole::TabPanel,
        "groupbox" => i_slint_core::items::AccessibleRole::Groupbox,
        "image" => i_slint_core::items::AccessibleRole::Image,
        "radio-button" => i_slint_core::items::AccessibleRole::RadioButton,
        _ => return None,
    })
}

/// Converts an arena Index to a (index, generation) pair for serialization.
pub(crate) fn index_to_parts(index: generational_arena::Index) -> (u64, u64) {
    let (idx, generation) = index.into_raw_parts();
    (idx as u64, generation)
}

/// Converts a (index, generation) pair back to an arena Index.
pub(crate) fn parts_to_index(index: u64, generation: u64) -> generational_arena::Index {
    generational_arena::Index::from_raw_parts(index as usize, generation)
}

pub(crate) fn layout_kind_to_string(lk: &crate::LayoutKind) -> &'static str {
    match lk {
        crate::LayoutKind::HorizontalLayout => "horizontal",
        crate::LayoutKind::VerticalLayout => "vertical",
        crate::LayoutKind::GridLayout => "grid",
        crate::LayoutKind::FlexBox => "flex-box",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_element() -> ElementHandle {
        ElementHandle::new_test_dummy()
    }

    #[test]
    fn test_element_to_handle_and_remove() {
        let state = IntrospectionState::new();
        let h1 = state.element_to_handle(dummy_element());
        let h2 = state.element_to_handle(dummy_element());
        assert_eq!(state.element_handles.borrow().len(), 2);

        state.remove_handles(&[h1]);
        assert_eq!(state.element_handles.borrow().len(), 1);
        assert!(state.element_handles.borrow().get(h1).is_none());
        assert!(state.element_handles.borrow().get(h2).is_some());
    }

    #[test]
    fn test_remove_handles_idempotent() {
        let state = IntrospectionState::new();
        let h = state.element_to_handle(dummy_element());
        state.remove_handles(&[h]);
        assert_eq!(state.element_handles.borrow().len(), 0);
        // Removing again is a no-op
        state.remove_handles(&[h]);
        assert_eq!(state.element_handles.borrow().len(), 0);
    }

    #[test]
    fn test_eviction_caps_arena_size() {
        let state = IntrospectionState::new();
        let mut handles = Vec::new();
        // Insert more than the cap
        for _ in 0..ELEMENT_HANDLE_CAP + 100 {
            handles.push(state.element_to_handle(dummy_element()));
        }
        // Arena should be capped
        assert!(state.element_handles.borrow().len() <= ELEMENT_HANDLE_CAP);
        // The most recent handles should still be valid
        let last = *handles.last().unwrap();
        assert!(state.element_handles.borrow().get(last).is_some());
        // The earliest handles should have been evicted
        let first = handles[0];
        assert!(state.element_handles.borrow().get(first).is_none());
    }

    #[test]
    fn test_eviction_preserves_root_element_handles() {
        let state = IntrospectionState::new();
        // Simulate a root element handle by inserting a window with a known handle
        let root_handle = state.element_to_handle(dummy_element());
        state.windows.borrow_mut().insert(TrackedWindow {
            window_adapter: Weak::<crate::testing_backend::TestingWindow>::new(),
            root_element_handle: root_handle,
        });

        // Fill past the cap
        for _ in 0..ELEMENT_HANDLE_CAP + 100 {
            state.element_to_handle(dummy_element());
        }

        // Root handle must survive eviction
        assert!(
            state.element_handles.borrow().get(root_handle).is_some(),
            "root element handle should not be evicted"
        );
    }
}
