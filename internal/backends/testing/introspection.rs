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

#[allow(non_snake_case, unused_imports, non_camel_case_types, clippy::all)]
pub(crate) mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
    include!(concat!(env!("OUT_DIR"), "/proto.serde.rs"));
}

/// Maximum number of element handles kept in the arena before evicting the oldest.
const ELEMENT_HANDLE_CAP: usize = 10_000;

thread_local! {
    static SHARED_STATE: RefCell<Option<Rc<IntrospectionState>>> = const { RefCell::new(None) };
    static HOOK_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// Returns the shared introspection state, creating it if needed.
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
pub(crate) fn ensure_window_tracking() -> Result<(), i_slint_core::api::EventLoopError> {
    HOOK_INSTALLED.with(|installed| {
        if installed.get() {
            return Ok(());
        }
        installed.set(true);

        let state = shared_state();

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
pub(crate) struct IntrospectionState {
    pub windows: RefCell<generational_arena::Arena<TrackedWindow>>,
    pub element_handles: RefCell<generational_arena::Arena<ElementHandle>>,
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
        if arena.len() > ELEMENT_HANDLE_CAP {
            let root_indices: std::collections::HashSet<generational_arena::Index> =
                self.windows.borrow().iter().map(|(_, w)| w.root_element_handle).collect();
            let mut budget = order.len();
            while arena.len() > ELEMENT_HANDLE_CAP && budget > 0 {
                budget -= 1;
                let Some(oldest) = order.pop_front() else { break };
                if !arena.contains(oldest) {
                    continue;
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
}

// ============================================================================
// Shared proto ↔ core conversion functions
// ============================================================================

pub(crate) fn element_properties(element: &ElementHandle) -> proto::ElementPropertiesResponse {
    let type_names_and_ids = core::iter::once(proto::ElementTypeNameAndId {
        type_name: element.type_name().unwrap_or_default().into(),
        id: element.id().unwrap_or_default().into(),
    })
    .chain(element.bases().into_iter().flatten().map(|base_type_name| {
        proto::ElementTypeNameAndId { type_name: base_type_name.into(), id: "root".into() }
    }))
    .collect();

    proto::ElementPropertiesResponse {
        type_names_and_ids,
        accessible_label: element.accessible_label().map_or(Default::default(), |s| s.to_string()),
        accessible_value: element.accessible_value().unwrap_or_default().to_string(),
        accessible_value_maximum: element.accessible_value_maximum().unwrap_or_default(),
        accessible_value_minimum: element.accessible_value_minimum().unwrap_or_default(),
        accessible_value_step: element.accessible_value_step().unwrap_or_default(),
        accessible_description: element.accessible_description().unwrap_or_default().to_string(),
        accessible_checked: element.accessible_checked().unwrap_or_default(),
        accessible_checkable: element.accessible_checkable().unwrap_or_default(),
        size: {
            let sz = element.size();
            Some(proto::LogicalSize { width: sz.width, height: sz.height })
        },
        absolute_position: {
            let pos = element.absolute_position();
            Some(proto::LogicalPosition { x: pos.x, y: pos.y })
        },
        accessible_role: convert_to_proto_accessible_role(
            element.accessible_role().unwrap_or(i_slint_core::items::AccessibleRole::None),
        )
        .unwrap_or_default()
        .into(),
        computed_opacity: element.computed_opacity(),
        accessible_placeholder_text: element
            .accessible_placeholder_text()
            .unwrap_or_default()
            .to_string(),
        accessible_enabled: element.accessible_enabled().unwrap_or_default(),
        accessible_read_only: element.accessible_read_only().unwrap_or_default(),
        layout_kind: match element.layout_kind() {
            Some(LayoutKind::HorizontalLayout) => proto::LayoutKind::HorizontalLayout.into(),
            Some(LayoutKind::VerticalLayout) => proto::LayoutKind::VerticalLayout.into(),
            Some(LayoutKind::GridLayout) => proto::LayoutKind::GridLayout.into(),
            Some(LayoutKind::FlexboxLayout) => proto::LayoutKind::FlexboxLayout.into(),
            None => proto::LayoutKind::NotALayout.into(),
        },
    }
}

pub(crate) fn query_element_descendants(
    element: ElementHandle,
    query_stack: Vec<proto::ElementQueryInstruction>,
    find_all: bool,
) -> Result<Vec<ElementHandle>, String> {
    use proto::element_query_instruction::Instruction;
    let mut query = element.query_descendants();
    for instruction in query_stack {
        match instruction
            .instruction
            .ok_or_else(|| "empty element query instruction".to_string())?
        {
            Instruction::MatchDescendants(_) => {
                query = query.match_descendants();
            }
            Instruction::MatchElementId(id) => query = query.match_id(id),
            Instruction::MatchElementTypeName(type_name) => {
                query = query.match_type_name(type_name)
            }
            Instruction::MatchElementTypeNameOrBase(type_name_or_base) => {
                query = query.match_inherits(type_name_or_base)
            }
            Instruction::MatchElementAccessibleRole(role_i32) => {
                let role = proto::AccessibleRole::try_from(role_i32)
                    .map_err(|_| format!("invalid AccessibleRole value: {role_i32}"))?;
                query = query.match_accessible_role(
                    convert_from_proto_accessible_role(role)
                        .ok_or_else(|| "Unknown accessibility role".to_string())?,
                )
            }
        }
    }
    Ok(if find_all { query.find_all() } else { query.find_first().into_iter().collect() })
}

pub(crate) fn invoke_element_accessibility_action(
    element: &ElementHandle,
    action: proto::ElementAccessibilityAction,
) {
    match action {
        proto::ElementAccessibilityAction::Default => element.invoke_accessible_default_action(),
        proto::ElementAccessibilityAction::Increment => {
            element.invoke_accessible_increment_action()
        }
        proto::ElementAccessibilityAction::Decrement => {
            element.invoke_accessible_decrement_action()
        }
        proto::ElementAccessibilityAction::Expand => element.invoke_accessible_expand_action(),
    }
}

pub(crate) fn convert_to_proto_accessible_role(
    role: i_slint_core::items::AccessibleRole,
) -> Option<proto::AccessibleRole> {
    Some(match role {
        i_slint_core::items::AccessibleRole::None => proto::AccessibleRole::Unknown,
        i_slint_core::items::AccessibleRole::Button => proto::AccessibleRole::Button,
        i_slint_core::items::AccessibleRole::Checkbox => proto::AccessibleRole::Checkbox,
        i_slint_core::items::AccessibleRole::Combobox => proto::AccessibleRole::Combobox,
        i_slint_core::items::AccessibleRole::Groupbox => proto::AccessibleRole::Groupbox,
        i_slint_core::items::AccessibleRole::List => proto::AccessibleRole::List,
        i_slint_core::items::AccessibleRole::Slider => proto::AccessibleRole::Slider,
        i_slint_core::items::AccessibleRole::Spinbox => proto::AccessibleRole::Spinbox,
        i_slint_core::items::AccessibleRole::Tab => proto::AccessibleRole::Tab,
        i_slint_core::items::AccessibleRole::TabList => proto::AccessibleRole::TabList,
        i_slint_core::items::AccessibleRole::Text => proto::AccessibleRole::Text,
        i_slint_core::items::AccessibleRole::Table => proto::AccessibleRole::Table,
        i_slint_core::items::AccessibleRole::Tree => proto::AccessibleRole::Tree,
        i_slint_core::items::AccessibleRole::ProgressIndicator => {
            proto::AccessibleRole::ProgressIndicator
        }
        i_slint_core::items::AccessibleRole::TextInput => proto::AccessibleRole::TextInput,
        i_slint_core::items::AccessibleRole::Switch => proto::AccessibleRole::Switch,
        i_slint_core::items::AccessibleRole::ListItem => proto::AccessibleRole::ListItem,
        i_slint_core::items::AccessibleRole::TabPanel => proto::AccessibleRole::TabPanel,
        i_slint_core::items::AccessibleRole::Image => proto::AccessibleRole::Image,
        i_slint_core::items::AccessibleRole::RadioButton => proto::AccessibleRole::RadioButton,
        _ => return None,
    })
}

pub(crate) fn convert_from_proto_accessible_role(
    role: proto::AccessibleRole,
) -> Option<i_slint_core::items::AccessibleRole> {
    Some(match role {
        proto::AccessibleRole::Unknown => i_slint_core::items::AccessibleRole::None,
        proto::AccessibleRole::Button => i_slint_core::items::AccessibleRole::Button,
        proto::AccessibleRole::Checkbox => i_slint_core::items::AccessibleRole::Checkbox,
        proto::AccessibleRole::Combobox => i_slint_core::items::AccessibleRole::Combobox,
        proto::AccessibleRole::Groupbox => i_slint_core::items::AccessibleRole::Groupbox,
        proto::AccessibleRole::List => i_slint_core::items::AccessibleRole::List,
        proto::AccessibleRole::Slider => i_slint_core::items::AccessibleRole::Slider,
        proto::AccessibleRole::Spinbox => i_slint_core::items::AccessibleRole::Spinbox,
        proto::AccessibleRole::Tab => i_slint_core::items::AccessibleRole::Tab,
        proto::AccessibleRole::TabList => i_slint_core::items::AccessibleRole::TabList,
        proto::AccessibleRole::Text => i_slint_core::items::AccessibleRole::Text,
        proto::AccessibleRole::Table => i_slint_core::items::AccessibleRole::Table,
        proto::AccessibleRole::Tree => i_slint_core::items::AccessibleRole::Tree,
        proto::AccessibleRole::ProgressIndicator => {
            i_slint_core::items::AccessibleRole::ProgressIndicator
        }
        proto::AccessibleRole::TextInput => i_slint_core::items::AccessibleRole::TextInput,
        proto::AccessibleRole::Switch => i_slint_core::items::AccessibleRole::Switch,
        proto::AccessibleRole::ListItem => i_slint_core::items::AccessibleRole::ListItem,
        proto::AccessibleRole::TabPanel => i_slint_core::items::AccessibleRole::TabPanel,
        proto::AccessibleRole::Image => i_slint_core::items::AccessibleRole::Image,
        proto::AccessibleRole::RadioButton => i_slint_core::items::AccessibleRole::RadioButton,
    })
}

pub(crate) fn convert_pointer_event_button(
    button: proto::PointerEventButton,
) -> i_slint_core::platform::PointerEventButton {
    match button {
        proto::PointerEventButton::Left => i_slint_core::platform::PointerEventButton::Left,
        proto::PointerEventButton::Right => i_slint_core::platform::PointerEventButton::Right,
        proto::PointerEventButton::Middle => i_slint_core::platform::PointerEventButton::Middle,
    }
}

// ============================================================================
// Index ↔ parts conversion
// ============================================================================

pub(crate) fn index_to_handle(index: generational_arena::Index) -> proto::Handle {
    let (idx, generation) = index.into_raw_parts();
    proto::Handle { index: idx as u64, generation }
}

pub(crate) fn handle_to_index(handle: proto::Handle) -> generational_arena::Index {
    generational_arena::Index::from_raw_parts(handle.index as usize, handle.generation)
}
