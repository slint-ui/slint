// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore slotmap's
//! Shared introspection state and logic used by both the systest (protobuf) and
//! MCP (HTTP/JSON-RPC) transports.

use i_slint_core::item_tree::ItemTreeRc;
use i_slint_core::window::WindowAdapter;
use i_slint_core::window::WindowInner;
use slotmap::{Key, KeyData, SlotMap};
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::{Rc, Weak};

use crate::{ElementHandle, ElementRoot, LayoutKind};

slotmap::new_key_type! {
    pub(crate) struct ArenaIndex;
}

#[allow(dead_code, non_snake_case, unused_imports, non_camel_case_types, clippy::all)]
pub(crate) mod proto;

/// Maximum number of element handles kept in the arena before evicting the oldest.
const ELEMENT_HANDLE_CAP: usize = 10_000;
const EVENT_LOG_CAP: usize = 1024;

fn bump(counter: &Cell<u64>) {
    counter.set(counter.get().saturating_add(1));
}

thread_local! {
    static SHARED_STATE: RefCell<Option<Rc<IntrospectionState>>> = const { RefCell::new(None) };
    static WINDOW_TRACKING_HOOK_INSTALLED: Cell<bool> = const { Cell::new(false) };
    static EVENT_TRACKING_HOOK_INSTALLED: Cell<bool> = const { Cell::new(false) };
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
    WINDOW_TRACKING_HOOK_INSTALLED.with(|installed| {
        if installed.get() {
            return ensure_event_tracking();
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

        ensure_event_tracking()
    })
}

fn ensure_event_tracking() -> Result<(), i_slint_core::api::EventLoopError> {
    EVENT_TRACKING_HOOK_INSTALLED.with(|installed| {
        if installed.get() {
            return Ok(());
        }
        installed.set(true);

        let state = shared_state();
        let previous_hook = i_slint_core::context::set_window_event_hook(None)
            .map_err(|_| i_slint_core::api::EventLoopError::NoEventLoopProvider)?;

        i_slint_core::context::set_window_event_hook(Some(Box::new(
            move |adapter, event, result| {
                if let Some(prev) = previous_hook.as_ref() {
                    prev(adapter, event, result.clone());
                }
                state.record_window_event(adapter, event, result);
            },
        )))
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
    pub root_element_handle: ArenaIndex,
}

/// Shared introspection state: window and element handle arenas.
pub(crate) struct IntrospectionState {
    pub windows: RefCell<SlotMap<ArenaIndex, TrackedWindow>>,
    pub element_handles: RefCell<SlotMap<ArenaIndex, ElementHandle>>,
    element_handle_order: RefCell<VecDeque<ArenaIndex>>,
    event_log: RefCell<VecDeque<proto::RecordedEvent>>,
    next_event_sequence: Cell<u64>,
    dropped_event_count: Cell<u64>,
    unknown_event_count: Cell<u64>,
    unknown_event_warned: Cell<bool>,
    recording_enabled: Cell<bool>,
}

impl IntrospectionState {
    pub fn new() -> Self {
        Self {
            windows: Default::default(),
            element_handles: Default::default(),
            element_handle_order: Default::default(),
            event_log: Default::default(),
            next_event_sequence: Default::default(),
            dropped_event_count: Default::default(),
            unknown_event_count: Default::default(),
            unknown_event_warned: Cell::new(false),
            recording_enabled: Cell::new(false),
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

    pub fn window_handles(&self) -> Vec<ArenaIndex> {
        self.windows.borrow().iter().map(|(index, _)| index).collect()
    }

    fn window_handle_for_adapter(&self, adapter: &Rc<dyn WindowAdapter>) -> Option<ArenaIndex> {
        self.windows.borrow().iter().find_map(|(index, tracked)| {
            tracked
                .window_adapter
                .upgrade()
                .filter(|tracked_adapter| Rc::ptr_eq(tracked_adapter, adapter))
                .map(|_| index)
        })
    }

    pub fn window_adapter(
        &self,
        window_index: ArenaIndex,
    ) -> Result<Rc<dyn WindowAdapter>, String> {
        self.windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .window_adapter
            .upgrade()
            .ok_or_else(|| "Attempting to access deleted window".to_string())
    }

    pub fn root_element_handle(&self, window_index: ArenaIndex) -> Result<ArenaIndex, String> {
        Ok(self
            .windows
            .borrow()
            .get(window_index)
            .ok_or_else(|| "Invalid window handle".to_string())?
            .root_element_handle)
    }

    pub fn element_to_handle(&self, element: ElementHandle) -> ArenaIndex {
        let mut arena = self.element_handles.borrow_mut();
        let index = arena.insert(element);
        let mut order = self.element_handle_order.borrow_mut();
        order.push_back(index);
        if arena.len() > ELEMENT_HANDLE_CAP {
            let root_indices: HashSet<ArenaIndex> =
                self.windows.borrow().iter().map(|(_, w)| w.root_element_handle).collect();
            let mut budget = order.len();
            while arena.len() > ELEMENT_HANDLE_CAP && budget > 0 {
                budget -= 1;
                let Some(oldest) = order.pop_front() else { break };
                if !arena.contains_key(oldest) {
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

    pub fn element(&self, request: &str, index: ArenaIndex) -> Result<ElementHandle, String> {
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
        window_index: ArenaIndex,
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
        window_index: ArenaIndex,
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
        window_index: ArenaIndex,
        event: i_slint_core::platform::WindowEvent,
    ) -> Result<(), String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        window.dispatch_event(event);
        Ok(())
    }

    pub fn record_window_event(
        &self,
        adapter: &Rc<dyn WindowAdapter>,
        event: &i_slint_core::platform::WindowEvent,
        result: i_slint_core::api::WindowEventDispatchResult,
    ) {
        if !self.recording_enabled.get() {
            return;
        }

        let proto_event = match convert_window_event_to_proto(event) {
            Ok(e) => e,
            Err(UnknownEventVariant) => {
                // Log once per process; the counter carries the magnitude.
                if !self.unknown_event_warned.replace(true) {
                    eprintln!(
                        "MCP/systest event recorder: unknown WindowEvent variant {event:?} — \
                         conversion code is out of sync with i_slint_core::platform. \
                         Further occurrences are silent; see unknown_event_count."
                    );
                }
                bump(&self.unknown_event_count);
                return;
            }
        };

        let sequence = self.next_event_sequence.get();
        self.next_event_sequence.set(sequence.saturating_add(1));

        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
            .unwrap_or_default();

        self.push_recorded_event(proto::RecordedEvent {
            sequence,
            timestamp_ms,
            window_handle: self.window_handle_for_adapter(adapter).map(index_to_handle),
            event: Some(proto_event),
            result: convert_event_dispatch_result(result).into(),
        });
    }

    /// Push a recorded event, evicting the oldest entries until the log is
    /// strictly below `EVENT_LOG_CAP`. Each evicted entry bumps
    /// `dropped_event_count`. This is the single source of truth for the
    /// eviction policy; tests should drive eviction through this method.
    fn push_recorded_event(&self, event: proto::RecordedEvent) {
        let mut log = self.event_log.borrow_mut();
        while log.len() >= EVENT_LOG_CAP {
            log.pop_front();
            bump(&self.dropped_event_count);
        }
        log.push_back(event);
    }

    #[cfg(feature = "system-testing")]
    pub fn query_event_log(
        &self,
        window_index: Option<ArenaIndex>,
        since_sequence: u64,
        max_events: u64,
        clear_after_read: bool,
    ) -> proto::EventLogResponse {
        let max_events = if max_events == 0 { 200 } else { max_events.min(1000) } as usize;
        let events: Vec<_> = self
            .event_log
            .borrow()
            .iter()
            .filter(|event| event.sequence >= since_sequence)
            .filter(|event| {
                window_index.is_none_or(|window_index| {
                    event.window_handle.as_ref().is_some_and(|handle| {
                        handle_to_index(*handle)
                            .is_ok_and(|event_window| event_window == window_index)
                    })
                })
            })
            .take(max_events)
            .cloned()
            .collect();
        let next_sequence = events
            .last()
            .map(|event| event.sequence.saturating_add(1))
            .unwrap_or_else(|| self.next_event_sequence.get());
        let returned_sequences: HashSet<u64> = events.iter().map(|event| event.sequence).collect();
        let response = proto::EventLogResponse {
            events,
            // Pass the next unread sequence number directly so callers can use
            // it as sinceSequence on the next poll without arithmetic.
            next_sequence,
            dropped_count: self.dropped_event_count.get(),
            unknown_event_count: self.unknown_event_count.get(),
        };
        if clear_after_read {
            self.event_log
                .borrow_mut()
                .retain(|event| !returned_sequences.contains(&event.sequence));
        }
        response
    }

    pub fn clear_event_log(&self) {
        self.event_log.borrow_mut().clear();
        self.dropped_event_count.set(0);
        self.unknown_event_count.set(0);
    }

    pub fn start_recording(&self) {
        self.clear_event_log();
        self.recording_enabled.set(true);
    }

    pub fn stop_recording(&self) -> proto::StopEventRecordingResponse {
        self.recording_enabled.set(false);
        let events: Vec<_> = self.event_log.borrow_mut().drain(..).collect();
        let dropped_count = self.dropped_event_count.replace(0);
        let unknown_event_count = self.unknown_event_count.replace(0);
        proto::StopEventRecordingResponse { events, dropped_count, unknown_event_count }
    }

    pub fn window_properties(
        &self,
        window_index: ArenaIndex,
    ) -> Result<proto::WindowPropertiesResponse, String> {
        let adapter = self.window_adapter(window_index)?;
        let window = adapter.window();
        Ok(proto::WindowPropertiesResponse {
            is_fullscreen: window.is_fullscreen(),
            is_maximized: window.is_maximized(),
            is_minimized: window.is_minimized(),
            size: Some(proto::PhysicalSize {
                width: window.size().width,
                height: window.size().height,
            }),
            position: Some(proto::PhysicalPosition {
                x: window.position().x,
                y: window.position().y,
            }),
            root_element_handle: Some(index_to_handle(self.root_element_handle(window_index)?)),
            scale_factor: window.scale_factor(),
        })
    }

    pub fn take_snapshot_response(
        &self,
        window_index: ArenaIndex,
        image_mime_type: &str,
    ) -> Result<proto::TakeSnapshotResponse, String> {
        let window_contents_as_encoded_image = self.take_snapshot(window_index, image_mime_type)?;
        Ok(proto::TakeSnapshotResponse { window_contents_as_encoded_image })
    }
}

/// Returned when a [`i_slint_core::platform::WindowEvent`] or
/// [`i_slint_core::platform::PointerEventButton`] variant has no proto mapping —
/// indicates the conversion code is out of date with the core enums.
#[derive(Debug)]
pub(crate) struct UnknownEventVariant;

pub(crate) fn convert_window_event_to_proto(
    event: &i_slint_core::platform::WindowEvent,
) -> Result<proto::WindowEvent, UnknownEventVariant> {
    use i_slint_core::platform::WindowEvent;
    use proto::window_event::Event;

    let event = match event {
        WindowEvent::PointerPressed { position, button } => {
            Event::PointerPressed(proto::PointerPressEvent {
                position: Some(proto::LogicalPosition { x: position.x, y: position.y }),
                button: convert_pointer_event_button_to_proto(*button)?.into(),
            })
        }
        WindowEvent::PointerReleased { position, button } => {
            Event::PointerReleased(proto::PointerReleaseEvent {
                position: Some(proto::LogicalPosition { x: position.x, y: position.y }),
                button: convert_pointer_event_button_to_proto(*button)?.into(),
            })
        }
        WindowEvent::PointerMoved { position } => Event::PointerMoved(proto::PointerMoveEvent {
            position: Some(proto::LogicalPosition { x: position.x, y: position.y }),
        }),
        WindowEvent::PointerScrolled { position, delta_x, delta_y } => {
            Event::PointerScrolled(proto::PointerScrolledEvent {
                position: Some(proto::LogicalPosition { x: position.x, y: position.y }),
                delta_x: *delta_x,
                delta_y: *delta_y,
            })
        }
        WindowEvent::PointerExited => Event::PointerExited(proto::PointerExitedEvent {}),
        WindowEvent::KeyPressed { text } => {
            Event::KeyPressed(proto::KeyPressedEvent { text: text.to_string() })
        }
        WindowEvent::KeyPressRepeated { text } => {
            Event::KeyPressRepeated(proto::KeyPressRepeatedEvent { text: text.to_string() })
        }
        WindowEvent::KeyReleased { text } => {
            Event::KeyReleased(proto::KeyReleasedEvent { text: text.to_string() })
        }
        WindowEvent::ScaleFactorChanged { scale_factor } => {
            Event::ScaleFactorChanged(proto::ScaleFactorChangedEvent {
                scale_factor: *scale_factor,
            })
        }
        WindowEvent::Resized { size } => Event::Resized(proto::ResizedEvent {
            size: Some(proto::LogicalSize { width: size.width, height: size.height }),
        }),
        WindowEvent::CloseRequested => Event::CloseRequested(proto::CloseRequestedEvent {}),
        WindowEvent::WindowActiveChanged(active) => {
            Event::WindowActiveChanged(proto::WindowActiveChangedEvent { active: *active })
        }
        // All current variants are covered above. This arm exists only because
        // WindowEvent is #[non_exhaustive]; future variants are reported as a bug
        // via record_window_event's unknown_event_count.
        #[allow(unreachable_patterns)]
        _ => return Err(UnknownEventVariant),
    };

    Ok(proto::WindowEvent { event: Some(event) })
}

fn convert_pointer_event_button_to_proto(
    button: i_slint_core::platform::PointerEventButton,
) -> Result<proto::PointerEventButton, UnknownEventVariant> {
    Ok(match button {
        i_slint_core::platform::PointerEventButton::Left => proto::PointerEventButton::Left,
        i_slint_core::platform::PointerEventButton::Right => proto::PointerEventButton::Right,
        i_slint_core::platform::PointerEventButton::Middle => proto::PointerEventButton::Middle,
        i_slint_core::platform::PointerEventButton::Back => proto::PointerEventButton::Back,
        i_slint_core::platform::PointerEventButton::Forward => proto::PointerEventButton::Forward,
        i_slint_core::platform::PointerEventButton::Other => proto::PointerEventButton::Other,
        // PointerEventButton is #[non_exhaustive]; future buttons surface as a bug
        // via record_window_event's unknown_event_count.
        #[allow(unreachable_patterns)]
        _ => return Err(UnknownEventVariant),
    })
}

fn convert_event_dispatch_result(
    result: i_slint_core::api::WindowEventDispatchResult,
) -> proto::RecordedEventResult {
    match result {
        i_slint_core::api::WindowEventDispatchResult::Accepted => {
            proto::RecordedEventResult::Accepted
        }
        i_slint_core::api::WindowEventDispatchResult::Rejected => {
            proto::RecordedEventResult::Rejected
        }
        i_slint_core::api::WindowEventDispatchResult::Ignored => {
            proto::RecordedEventResult::Ignored
        }
        _ => unreachable!(),
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
        accessible_label: element.accessible_label().map(|s| s.to_string()).unwrap_or_default(),
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
        i_slint_core::items::AccessibleRole::RadioGroup => proto::AccessibleRole::RadioGroup,
        i_slint_core::items::AccessibleRole::Banner => proto::AccessibleRole::Banner,
        i_slint_core::items::AccessibleRole::Complementary => proto::AccessibleRole::Complementary,
        i_slint_core::items::AccessibleRole::ContentInfo => proto::AccessibleRole::ContentInfo,
        i_slint_core::items::AccessibleRole::Form => proto::AccessibleRole::Form,
        i_slint_core::items::AccessibleRole::Main => proto::AccessibleRole::Main,
        i_slint_core::items::AccessibleRole::Navigation => proto::AccessibleRole::Navigation,
        i_slint_core::items::AccessibleRole::Region => proto::AccessibleRole::Region,
        i_slint_core::items::AccessibleRole::Search => proto::AccessibleRole::Search,
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
        proto::AccessibleRole::RadioGroup => i_slint_core::items::AccessibleRole::RadioGroup,
        proto::AccessibleRole::Banner => i_slint_core::items::AccessibleRole::Banner,
        proto::AccessibleRole::Complementary => i_slint_core::items::AccessibleRole::Complementary,
        proto::AccessibleRole::ContentInfo => i_slint_core::items::AccessibleRole::ContentInfo,
        proto::AccessibleRole::Form => i_slint_core::items::AccessibleRole::Form,
        proto::AccessibleRole::Main => i_slint_core::items::AccessibleRole::Main,
        proto::AccessibleRole::Navigation => i_slint_core::items::AccessibleRole::Navigation,
        proto::AccessibleRole::Region => i_slint_core::items::AccessibleRole::Region,
        proto::AccessibleRole::Search => i_slint_core::items::AccessibleRole::Search,
    })
}

pub(crate) fn convert_pointer_event_button(
    button: proto::PointerEventButton,
) -> i_slint_core::platform::PointerEventButton {
    match button {
        proto::PointerEventButton::Left => i_slint_core::platform::PointerEventButton::Left,
        proto::PointerEventButton::Right => i_slint_core::platform::PointerEventButton::Right,
        proto::PointerEventButton::Middle => i_slint_core::platform::PointerEventButton::Middle,
        proto::PointerEventButton::Back => i_slint_core::platform::PointerEventButton::Back,
        proto::PointerEventButton::Forward => i_slint_core::platform::PointerEventButton::Forward,
        proto::PointerEventButton::Other => i_slint_core::platform::PointerEventButton::Other,
    }
}

// ============================================================================
// Index ↔ handle conversion
// ============================================================================

pub(crate) fn index_to_handle(index: ArenaIndex) -> proto::Handle {
    let ffi = index.data().as_ffi();
    proto::Handle { index: ffi & 0xffff_ffff, generation: ffi >> 32 }
}

pub(crate) fn handle_to_index(handle: proto::Handle) -> Result<ArenaIndex, String> {
    if handle.index > u64::from(u32::MAX) || handle.generation > u64::from(u32::MAX) {
        return Err("Invalid handle".to_string());
    }

    let ffi = (handle.generation << 32) | handle.index;
    let index: ArenaIndex = KeyData::from_ffi(ffi).into();

    // Reject malformed handles instead of accepting slotmap's normalization.
    if index.data().as_ffi() != ffi {
        return Err("Invalid handle".to_string());
    }

    Ok(index)
}

// ============================================================================
// Shared dispatch functions used by both transports
// ============================================================================

pub(crate) mod dispatch {
    use super::{
        ArenaIndex, IntrospectionState, convert_pointer_event_button, index_to_handle,
        invoke_element_accessibility_action, proto,
    };

    pub(crate) fn list_windows(state: &IntrospectionState) -> proto::WindowListResponse {
        proto::WindowListResponse {
            window_handles: state.window_handles().into_iter().map(index_to_handle).collect(),
        }
    }

    pub(crate) fn window_properties(
        state: &IntrospectionState,
        window: ArenaIndex,
    ) -> Result<proto::WindowPropertiesResponse, String> {
        state.window_properties(window)
    }

    pub(crate) fn find_elements_by_id(
        state: &IntrospectionState,
        window: ArenaIndex,
        elements_id: &str,
    ) -> Result<proto::ElementsResponse, String> {
        let elements = state.find_elements_by_id(window, elements_id)?;
        Ok(proto::ElementsResponse {
            element_handles: elements
                .into_iter()
                .map(|e| index_to_handle(state.element_to_handle(e)))
                .collect(),
        })
    }

    pub(crate) fn element_properties(
        state: &IntrospectionState,
        element: ArenaIndex,
    ) -> Result<proto::ElementPropertiesResponse, String> {
        let element = state.element("element_properties", element)?;
        Ok(super::element_properties(&element))
    }

    pub(crate) fn query_element_descendants(
        state: &IntrospectionState,
        element: ArenaIndex,
        query_stack: Vec<proto::ElementQueryInstruction>,
        find_all: bool,
    ) -> Result<proto::ElementQueryResponse, String> {
        let element = state.element("query_element_descendants", element)?;
        let results = super::query_element_descendants(element, query_stack, find_all)?;
        Ok(proto::ElementQueryResponse {
            element_handles: results
                .into_iter()
                .map(|e| index_to_handle(state.element_to_handle(e)))
                .collect(),
        })
    }

    pub(crate) fn take_snapshot(
        state: &IntrospectionState,
        window: ArenaIndex,
        image_mime_type: &str,
    ) -> Result<proto::TakeSnapshotResponse, String> {
        state.take_snapshot_response(window, image_mime_type)
    }

    #[cfg(feature = "system-testing")]
    pub(crate) fn event_log(
        state: &IntrospectionState,
        window: Option<ArenaIndex>,
        since_sequence: u64,
        max_events: u64,
        clear_after_read: bool,
    ) -> proto::EventLogResponse {
        state.query_event_log(window, since_sequence, max_events, clear_after_read)
    }

    #[cfg(feature = "system-testing")]
    pub(crate) fn clear_event_log(state: &IntrospectionState) -> proto::ClearEventLogResponse {
        state.clear_event_log();
        proto::ClearEventLogResponse {}
    }

    pub(crate) fn start_event_recording(
        state: &IntrospectionState,
    ) -> proto::StartEventRecordingResponse {
        state.start_recording();
        proto::StartEventRecordingResponse {}
    }

    pub(crate) fn stop_event_recording(
        state: &IntrospectionState,
    ) -> proto::StopEventRecordingResponse {
        state.stop_recording()
    }

    pub(crate) fn invoke_accessibility_action(
        state: &IntrospectionState,
        element: ArenaIndex,
        action: proto::ElementAccessibilityAction,
    ) -> Result<(), String> {
        let element = state.element("invoke_accessibility_action", element)?;
        invoke_element_accessibility_action(&element, action);
        Ok(())
    }

    pub(crate) fn set_accessible_value(
        state: &IntrospectionState,
        element: ArenaIndex,
        value: String,
    ) -> Result<(), String> {
        let element = state.element("set_accessible_value", element)?;
        element.set_accessible_value(value);
        Ok(())
    }

    pub(crate) async fn click(
        state: &IntrospectionState,
        element: ArenaIndex,
        action: proto::ClickAction,
        button: proto::PointerEventButton,
    ) -> Result<(), String> {
        let element = state.element("click", element)?;
        let button = convert_pointer_event_button(button);
        match action {
            proto::ClickAction::SingleClick => element.single_click(button).await,
            proto::ClickAction::DoubleClick => element.double_click(button).await,
        }
        Ok(())
    }

    pub(crate) async fn drag(
        state: &IntrospectionState,
        element: ArenaIndex,
        target: proto::LogicalPosition,
        button: proto::PointerEventButton,
    ) -> Result<(), String> {
        let element = state.element("drag", element)?;
        let button = convert_pointer_event_button(button);
        let target = i_slint_core::api::LogicalPosition::new(target.x, target.y);
        element.drag(target, button).await;
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn test_dispatch_element_properties_stale_handle() {
    let state = IntrospectionState::new();
    let err = dispatch::element_properties(&state, ArenaIndex::default()).unwrap_err();
    assert!(err.contains("Invalid element handle"), "got: {err}");
}

#[test]
fn test_dispatch_find_elements_by_id_stale_window() {
    let state = IntrospectionState::new();
    let err = dispatch::find_elements_by_id(&state, ArenaIndex::default(), "foo").unwrap_err();
    assert!(err.contains("Invalid window handle"), "got: {err}");
}

#[test]
fn test_dispatch_click_double_click_stale_handle() {
    futures_lite::future::block_on(async {
        let state = IntrospectionState::new();
        let err = dispatch::click(
            &state,
            ArenaIndex::default(),
            proto::ClickAction::DoubleClick,
            proto::PointerEventButton::Left,
        )
        .await
        .unwrap_err();
        assert!(err.contains("Invalid element handle"), "got: {err}");
    });
}

#[test]
fn test_handle_to_index_rejects_noncanonical_generation() {
    assert!(handle_to_index(proto::Handle { index: 42, generation: 6 }).is_err());
}

#[test]
fn test_handle_to_index_rejects_out_of_range_parts() {
    assert!(
        handle_to_index(proto::Handle { index: u64::from(u32::MAX) + 1, generation: 7 }).is_err()
    );
    assert!(
        handle_to_index(proto::Handle { index: 42, generation: u64::from(u32::MAX) + 1 }).is_err()
    );
}

#[test]
fn test_event_log_filters_since_sequence_and_window() {
    let state = IntrospectionState::new();
    let mut window_indices = SlotMap::with_key();
    let first_window = window_indices.insert(());
    let second_window = window_indices.insert(());

    state.next_event_sequence.set(3);
    state.event_log.borrow_mut().extend([
        proto::RecordedEvent {
            sequence: 0,
            window_handle: Some(index_to_handle(first_window)),
            result: proto::RecordedEventResult::Accepted.into(),
            ..Default::default()
        },
        proto::RecordedEvent {
            sequence: 1,
            window_handle: Some(index_to_handle(second_window)),
            result: proto::RecordedEventResult::Accepted.into(),
            ..Default::default()
        },
        proto::RecordedEvent {
            sequence: 2,
            window_handle: Some(index_to_handle(first_window)),
            result: proto::RecordedEventResult::Ignored.into(),
            ..Default::default()
        },
    ]);

    let response = state.query_event_log(Some(first_window), 1, 10, true);
    assert_eq!(response.events.len(), 1);
    assert_eq!(response.events[0].sequence, 2);
    assert_eq!(response.next_sequence, 3);
    // clear_after_read removes only returned events.
    let remaining = state.query_event_log(None, 0, 10, false);
    assert_eq!(remaining.events.iter().map(|event| event.sequence).collect::<Vec<_>>(), vec![0, 1]);
    assert_eq!(remaining.next_sequence, 2);
}

#[test]
fn test_event_log_eviction_at_cap() {
    let state = IntrospectionState::new();

    // Push EVENT_LOG_CAP + 10 events through the real eviction path.
    for seq in 0..(EVENT_LOG_CAP + 10) as u64 {
        state.push_recorded_event(proto::RecordedEvent {
            sequence: seq,
            result: proto::RecordedEventResult::Accepted.into(),
            ..Default::default()
        });
        state.next_event_sequence.set(seq + 1);
    }

    assert_eq!(state.event_log.borrow().len(), EVENT_LOG_CAP);
    assert_eq!(state.dropped_event_count.get(), 10);

    // The oldest retained event should have sequence 10 (the first 10 were evicted).
    let response = state.query_event_log(None, 0, 1, false);
    assert_eq!(response.events[0].sequence, 10);
    assert_eq!(response.dropped_count, 10);
    assert_eq!(response.next_sequence, 11);

    // After clear, dropped count and log reset, but the sequence cursor remains monotonic.
    state.clear_event_log();
    assert!(state.event_log.borrow().is_empty());
    assert_eq!(state.dropped_event_count.get(), 0);
    assert_eq!(state.next_event_sequence.get(), (EVENT_LOG_CAP + 10) as u64);
    assert_eq!(state.query_event_log(None, 0, 1, false).next_sequence, (EVENT_LOG_CAP + 10) as u64);
}

#[test]
fn test_event_log_pagination_cursor_advances_to_returned_page() {
    let state = IntrospectionState::new();
    for seq in 0..3 {
        state.event_log.borrow_mut().push_back(proto::RecordedEvent {
            sequence: seq,
            result: proto::RecordedEventResult::Accepted.into(),
            ..Default::default()
        });
    }
    state.next_event_sequence.set(3);

    let first_page = state.query_event_log(None, 0, 1, false);
    assert_eq!(first_page.events[0].sequence, 0);
    assert_eq!(first_page.next_sequence, 1);

    let second_page = state.query_event_log(None, first_page.next_sequence, 1, false);
    assert_eq!(second_page.events[0].sequence, 1);
    assert_eq!(second_page.next_sequence, 2);
}

#[test]
fn test_event_log_clear_keeps_sequence_monotonic() {
    let state = IntrospectionState::new();
    state.next_event_sequence.set(42);
    state.event_log.borrow_mut().push_back(proto::RecordedEvent {
        sequence: 41,
        result: proto::RecordedEventResult::Accepted.into(),
        ..Default::default()
    });

    state.clear_event_log();
    assert_eq!(state.next_event_sequence.get(), 42);
    assert_eq!(state.query_event_log(None, 42, 10, false).next_sequence, 42);
}

#[test]
fn test_pointer_event_button_mapping_preserves_extended_buttons() {
    assert_eq!(
        convert_pointer_event_button_to_proto(i_slint_core::platform::PointerEventButton::Back)
            .unwrap(),
        proto::PointerEventButton::Back
    );
    assert_eq!(
        convert_pointer_event_button_to_proto(i_slint_core::platform::PointerEventButton::Forward)
            .unwrap(),
        proto::PointerEventButton::Forward
    );
    assert_eq!(
        convert_pointer_event_button_to_proto(i_slint_core::platform::PointerEventButton::Other)
            .unwrap(),
        proto::PointerEventButton::Other
    );
    assert_eq!(
        convert_pointer_event_button(proto::PointerEventButton::Back),
        i_slint_core::platform::PointerEventButton::Back
    );
    assert_eq!(
        convert_pointer_event_button(proto::PointerEventButton::Forward),
        i_slint_core::platform::PointerEventButton::Forward
    );
    assert_eq!(
        convert_pointer_event_button(proto::PointerEventButton::Other),
        i_slint_core::platform::PointerEventButton::Other
    );
}

#[test]
fn test_accessibility_role_mapping_complete() {
    macro_rules! test_accessibility_enum_mapping_inner {
        (AccessibleRole, $($Value:ident,)*) => {
            $(assert!(convert_to_proto_accessible_role(i_slint_core::items::AccessibleRole::$Value).is_some());)*
        };
        ($_:ident, $($Value:ident,)*) => {};
    }

    macro_rules! test_accessibility_enum_mapping {
        ($( $(#[doc = $enum_doc:literal])* $(#[non_exhaustive])? $vis:vis enum $Name:ident { $( $(#[doc = $value_doc:literal])* $Value:ident,)* })*) => {
            $(
                test_accessibility_enum_mapping_inner!($Name, $($Value,)*);
            )*
        };
    }
    i_slint_common::for_each_enums!(test_accessibility_enum_mapping);
}

// `WindowEventDispatchResult` honesty for pointer events: verify that
// `Window::dispatch_event_with_result` reports `Accepted` only when an item consumed the
// event, and `Ignored` otherwise. Tests install the window-event hook directly
// since that's the consumer the public contract is for.

#[cfg(test)]
mod dispatch_result_tests {
    use i_slint_core::api::LogicalPosition;
    use i_slint_core::api::WindowEventDispatchResult;
    use i_slint_core::items::PointerEventButton;
    use i_slint_core::platform::WindowEvent;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Install a recording hook; the returned guard restores whatever hook was
    /// installed before (possibly `None`) on drop.
    fn capture_hook() -> (HookGuard, Rc<RefCell<Vec<(WindowEvent, WindowEventDispatchResult)>>>) {
        let captured = Rc::new(RefCell::new(Vec::new()));
        let captured_in_hook = captured.clone();
        let previous = i_slint_core::context::set_window_event_hook(Some(Box::new(
            move |_adapter, event, result| {
                captured_in_hook.borrow_mut().push((event.clone(), result));
            },
        )))
        .expect("install hook");
        (HookGuard { previous: Some(previous) }, captured)
    }

    struct HookGuard {
        previous: Option<Option<i_slint_core::context::WindowEventHook>>,
    }

    impl Drop for HookGuard {
        fn drop(&mut self) {
            if let Some(prev) = self.previous.take() {
                let _ = i_slint_core::context::set_window_event_hook(prev);
            }
        }
    }

    /// Dispatch `event` to `window` with a recording hook installed, then assert that
    /// exactly one hook invocation occurred with the `expected` dispatch result.
    fn assert_single_dispatch(
        window: &i_slint_core::api::Window,
        event: WindowEvent,
        expected: WindowEventDispatchResult,
    ) {
        let (_guard, captured) = capture_hook();
        window.dispatch_event(event);
        let captured = captured.borrow();
        assert_eq!(captured.len(), 1);
        assert_eq!(captured[0].1, expected);
    }

    #[test]
    fn pointer_pressed_over_touch_area_is_accepted() {
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                TouchArea { width: 100%; height: 100%; }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerPressed {
                position: LogicalPosition::new(50.0, 50.0),
                button: PointerEventButton::Left,
            },
            WindowEventDispatchResult::Accepted,
        );
    }

    #[test]
    fn pointer_pressed_with_no_handler_is_ignored() {
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                Rectangle { background: #abc; }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerPressed {
                position: LogicalPosition::new(50.0, 50.0),
                button: PointerEventButton::Left,
            },
            WindowEventDispatchResult::Ignored,
        );
    }

    #[test]
    fn pointer_scrolled_over_flickable_is_accepted() {
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                Flickable {
                    width: 100%; height: 100%;
                    content-width: 400px;
                    content-height: 400px;
                    Rectangle { background: #abc; }
                }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerScrolled {
                position: LogicalPosition::new(100.0, 100.0),
                delta_x: 0.0,
                delta_y: -30.0,
            },
            WindowEventDispatchResult::Accepted,
        );
    }

    #[test]
    fn pointer_pressed_inside_flickable_is_accepted() {
        // Flickable installs `DelayForwarding` on press to disambiguate click from flick;
        // the hit-test visitor returns `abort` for the delayed item, so the press dispatch
        // is Accepted even though no child has yet received the event.
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                Flickable {
                    content-width: 400px;
                    content-height: 400px;
                    Rectangle { background: #abc; }
                }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerPressed {
                position: LogicalPosition::new(100.0, 100.0),
                button: PointerEventButton::Left,
            },
            WindowEventDispatchResult::Accepted,
        );
    }

    #[test]
    fn pointer_exited_is_always_accepted() {
        // Teardown event — Accepted unconditionally even when no item is under the cursor.
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                Rectangle { background: #abc; }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerExited,
            WindowEventDispatchResult::Accepted,
        );
    }

    #[test]
    fn pointer_scrolled_over_empty_area_is_ignored() {
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                Rectangle { background: #abc; }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerScrolled {
                position: LogicalPosition::new(100.0, 100.0),
                delta_x: 0.0,
                delta_y: -30.0,
            },
            WindowEventDispatchResult::Ignored,
        );
    }

    #[test]
    fn pointer_moved_over_empty_area_is_ignored() {
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                Rectangle { background: #abc; }
            }
        }
        let app = App::new().unwrap();
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerMoved { position: LogicalPosition::new(50.0, 50.0) },
            WindowEventDispatchResult::Ignored,
        );
    }

    #[test]
    fn pointer_moved_while_touch_area_is_pressed_is_accepted() {
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                TouchArea { width: 100%; height: 100%; }
            }
        }
        let app = App::new().unwrap();
        // Press first so the TouchArea grabs the mouse; without the grab a Moved over a
        // TouchArea is hover-only and falls through.
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: LogicalPosition::new(50.0, 50.0),
            button: PointerEventButton::Left,
        });
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerMoved { position: LogicalPosition::new(60.0, 60.0) },
            WindowEventDispatchResult::Accepted,
        );
    }

    #[test]
    fn pointer_released_outside_grabbed_touch_area_is_accepted() {
        // The TouchArea grabbed the mouse on press, so the release reaches the grab handler
        // even though it lands outside the item's geometry — Accepted via the grab path.
        crate::init_no_event_loop();
        slint::slint! {
            export component App inherits Window {
                width: 200px;
                height: 200px;
                TouchArea { width: 100%; height: 100%; }
            }
        }
        let app = App::new().unwrap();
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: LogicalPosition::new(20.0, 20.0),
            button: PointerEventButton::Left,
        });
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: LogicalPosition::new(250.0, 250.0),
        });
        assert_single_dispatch(
            app.window(),
            WindowEvent::PointerReleased {
                position: LogicalPosition::new(250.0, 250.0),
                button: PointerEventButton::Left,
            },
            WindowEventDispatchResult::Accepted,
        );
    }

    #[test]
    fn pointer_released_at_end_of_drag_is_accepted_if_droparea_accepted() {
        // Release is rewritten internally to `Drop`; with a permissive DropArea the
        // public PointerReleased dispatch reports Accepted.
        crate::init_no_event_loop();
        slint::slint! {
            export global Api {
                pure callback make-data() -> data-transfer;
            }
            export component App inherits Window {
                width: 200px;
                height: 200px;
                VerticalLayout {
                    DragArea {
                        data: Api.make-data();
                        allow-copy: true;
                        Rectangle { background: #abc; }
                    }
                    DropArea {
                        can-drop(_) => { DragAction.copy }
                        Rectangle { background: #cba; }
                    }
                }
            }
        }
        let app = App::new().unwrap();
        app.global::<Api>().on_make_data(|| slint::SharedString::from("payload").into());
        let (_guard, captured) = capture_hook();
        crate::search_api::mock_drag_window(
            app.window(),
            LogicalPosition::new(100.0, 50.0),
            LogicalPosition::new(100.0, 150.0),
            PointerEventButton::Left,
        );
        let release = captured
            .borrow()
            .iter()
            .rev()
            .find(|(e, _)| matches!(e, WindowEvent::PointerReleased { .. }))
            .map(|(_, r)| r.clone())
            .expect("PointerReleased recorded");
        assert_eq!(release, WindowEventDispatchResult::Accepted);
    }

    #[test]
    fn pointer_released_at_end_of_drag_is_ignored_if_no_droparea_accepted() {
        // Release is rewritten internally to `Exit` (no DropArea accepted the prior
        // DragMove); the public PointerReleased reports Ignored.
        crate::init_no_event_loop();
        slint::slint! {
            export global Api {
                pure callback make-data() -> data-transfer;
            }
            export component App inherits Window {
                width: 200px;
                height: 200px;
                DragArea {
                    data: Api.make-data();
                    allow-copy: true;
                    Rectangle { background: #abc; }
                }
            }
        }
        let app = App::new().unwrap();
        app.global::<Api>().on_make_data(|| slint::SharedString::from("payload").into());
        let (_guard, captured) = capture_hook();
        crate::search_api::mock_drag_window(
            app.window(),
            LogicalPosition::new(50.0, 100.0),
            LogicalPosition::new(150.0, 100.0),
            PointerEventButton::Left,
        );
        let release = captured
            .borrow()
            .iter()
            .rev()
            .find(|(e, _)| matches!(e, WindowEvent::PointerReleased { .. }))
            .map(|(_, r)| r.clone())
            .expect("PointerReleased recorded");
        assert_eq!(release, WindowEventDispatchResult::Ignored);
    }
}
