// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    EventResult, FocusReasonArg, Item, ItemConsts, ItemRc, ItemRendererRef, KeyEventArg,
    MouseCursor, PointerEvent, PointerEventArg, PointerEventButton, PointerEventKind,
    PointerScrollEvent, PointerScrollEventArg, RenderingResult, VoidArg,
};
use crate::api::LogicalPosition;
use crate::input::{
    FocusEvent, FocusEventResult, FocusReason, InputEventFilterResult, InputEventResult,
    InternalKeyEvent, KeyEventResult, KeyEventType, Keys, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::items::ItemTreeVTable;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PointLengths};
use crate::properties::PropertyTracker;
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::{WindowAdapter, WindowInner};
use crate::{Callback, Coord, Property};
use alloc::{boxed::Box, rc::Rc, vec::Vec};
use const_field_offset::FieldOffsets;
use core::cell::Cell;
use core::pin::Pin;
use i_slint_core_macros::*;
use vtable::{VRcMapped, VWeakMapped};

/// The implementation of the `TouchArea` element
#[repr(C)]
#[derive(FieldOffsets, SlintElement, Default)]
#[pin]
pub struct TouchArea {
    pub enabled: Property<bool>,
    /// FIXME: We should annotate this as an "output" property.
    pub pressed: Property<bool>,
    pub has_hover: Property<bool>,
    /// FIXME: there should be just one property for the point instead of two.
    /// Could even be merged with pressed in a `Property<Option<Point>>` (of course, in the
    /// implementation item only, for the compiler it would stay separate properties)
    pub pressed_x: Property<LogicalLength>,
    pub pressed_y: Property<LogicalLength>,
    /// FIXME: should maybe be as parameter to the mouse event instead. Or at least just one property
    pub mouse_x: Property<LogicalLength>,
    pub mouse_y: Property<LogicalLength>,
    pub mouse_cursor: Property<MouseCursor>,
    pub clicked: Callback<VoidArg>,
    pub double_clicked: Callback<VoidArg>,
    pub moved: Callback<VoidArg>,
    pub pointer_event: Callback<PointerEventArg>,
    pub scroll_event: Callback<PointerScrollEventArg, EventResult>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
    /// true when we are currently grabbing the mouse
    grabbed: Cell<bool>,
}

impl Item for TouchArea {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        cursor: &mut MouseCursor,
    ) -> InputEventFilterResult {
        if !self.enabled() {
            self.has_hover.set(false);
            if self.grabbed.replace(false) {
                self.pressed.set(false);
                Self::FIELD_OFFSETS.pointer_event().apply_pin(self).call(&(PointerEvent {
                    button: PointerEventButton::Other,
                    kind: PointerEventKind::Cancel,
                    modifiers: window_adapter.window().0.context().0.modifiers.get().into(),
                    touch_finger_id: 0,
                },));
            }
            return InputEventFilterResult::ForwardAndIgnore;
        }
        if matches!(event, MouseEvent::DragMove(..) | MouseEvent::Drop(..)) {
            // Someone else has the grab, don't handle hover
            return InputEventFilterResult::ForwardAndIgnore;
        }
        if let Some(pos) = event.position() {
            Self::FIELD_OFFSETS.mouse_x().apply_pin(self).set(pos.x_length());
            Self::FIELD_OFFSETS.mouse_y().apply_pin(self).set(pos.y_length());
        }
        let hovering = !matches!(event, MouseEvent::Exit);
        Self::FIELD_OFFSETS.has_hover().apply_pin(self).set(hovering);
        if hovering {
            *cursor = self.mouse_cursor();
        }
        InputEventFilterResult::ForwardAndInterceptGrab
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventResult {
        if matches!(event, MouseEvent::Exit) {
            Self::FIELD_OFFSETS.has_hover().apply_pin(self).set(false);
        }
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        match event {
            MouseEvent::Pressed { position, button, touch_finger_id, .. } => {
                self.grabbed.set(true);
                if *button == PointerEventButton::Left {
                    Self::FIELD_OFFSETS.pressed_x().apply_pin(self).set(position.x_length());
                    Self::FIELD_OFFSETS.pressed_y().apply_pin(self).set(position.y_length());
                    Self::FIELD_OFFSETS.pressed().apply_pin(self).set(true);
                }
                Self::FIELD_OFFSETS.pointer_event().apply_pin(self).call(&(PointerEvent {
                    button: *button,
                    kind: PointerEventKind::Down,
                    modifiers: window_adapter.window().0.context().0.modifiers.get().into(),
                    touch_finger_id: *touch_finger_id,
                },));

                InputEventResult::GrabMouse
            }
            MouseEvent::Exit => {
                Self::FIELD_OFFSETS.pressed().apply_pin(self).set(false);
                if self.grabbed.replace(false) {
                    Self::FIELD_OFFSETS.pointer_event().apply_pin(self).call(&(PointerEvent {
                        button: PointerEventButton::Other,
                        kind: PointerEventKind::Cancel,
                        modifiers: window_adapter.window().0.context().0.modifiers.get().into(),
                        touch_finger_id: 0,
                    },));
                }

                InputEventResult::EventAccepted
            }

            MouseEvent::Released { button, position, click_count, touch_finger_id } => {
                let geometry = self_rc.geometry();
                if *button == PointerEventButton::Left
                    && LogicalRect::new(LogicalPoint::default(), geometry.size).contains(*position)
                    && self.pressed()
                {
                    Self::FIELD_OFFSETS.clicked().apply_pin(self).call(&());
                    if (click_count % 2) == 1 {
                        Self::FIELD_OFFSETS.double_clicked().apply_pin(self).call(&())
                    }
                }

                self.grabbed.set(false);
                if *button == PointerEventButton::Left {
                    Self::FIELD_OFFSETS.pressed().apply_pin(self).set(false);
                }
                Self::FIELD_OFFSETS.pointer_event().apply_pin(self).call(&(PointerEvent {
                    button: *button,
                    kind: PointerEventKind::Up,
                    modifiers: window_adapter.window().0.context().0.modifiers.get().into(),
                    touch_finger_id: *touch_finger_id,
                },));

                InputEventResult::EventAccepted
            }
            MouseEvent::Moved { touch_finger_id, .. } => {
                Self::FIELD_OFFSETS.pointer_event().apply_pin(self).call(&(PointerEvent {
                    button: PointerEventButton::Other,
                    kind: PointerEventKind::Move,
                    modifiers: window_adapter.window().0.context().0.modifiers.get().into(),
                    touch_finger_id: *touch_finger_id,
                },));
                if self.grabbed.get() {
                    Self::FIELD_OFFSETS.moved().apply_pin(self).call(&());
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                }
            }
            MouseEvent::Wheel { delta_x, delta_y, .. } => {
                let modifiers = window_adapter.window().0.context().0.modifiers.get().into();
                let r = Self::FIELD_OFFSETS.scroll_event().apply_pin(self).call(&(
                    PointerScrollEvent { delta_x: *delta_x, delta_y: *delta_y, modifiers },
                ));
                if self.grabbed.get() {
                    InputEventResult::GrabMouse
                } else {
                    match r {
                        EventResult::Reject => {
                            // We are ignoring the event, so we will be removed from the item_stack,
                            // therefore we must remove the has_hover flag as there might be a scroll under us.
                            // It will be put back later.
                            Self::FIELD_OFFSETS.has_hover().apply_pin(self).set(false);
                            InputEventResult::EventIgnored
                        }
                        EventResult::Accept => InputEventResult::EventAccepted,
                    }
                }
            }
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. } => {
                InputEventResult::EventIgnored
            }
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => InputEventResult::EventIgnored,
        }
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        mut geometry: LogicalRect,
    ) -> LogicalRect {
        geometry.size = LogicalSize::zero();
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

impl ItemConsts for KeyBinding {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        KeyBinding,
        CachedRenderingData,
    > = KeyBinding::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct KeyBinding {
    pub keys: Property<Keys>,
    pub enabled: Property<bool>,
    pub activated: Callback<VoidArg>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for KeyBinding {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: crate::items::Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::layout::LayoutInfo {
        Default::default()
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &crate::input::MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut crate::items::MouseCursor,
    ) -> crate::input::InputEventFilterResult {
        Default::default()
    }

    fn input_event(
        self: Pin<&Self>,
        _: &crate::input::MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut crate::items::MouseCursor,
    ) -> crate::input::InputEventResult {
        Default::default()
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::KeyEventResult {
        crate::input::KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::KeyEventResult {
        Default::default()
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &crate::input::FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> crate::input::FocusEventResult {
        Default::default()
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut &mut dyn crate::item_rendering::ItemRenderer,
        _self_rc: &ItemRc,
        _size: crate::lengths::LogicalSize,
    ) -> crate::items::RenderingResult {
        Default::default()
    }

    fn bounding_rect(
        self: Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        geometry: crate::lengths::LogicalRect,
    ) -> crate::lengths::LogicalRect {
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

/// An optimized ShortcutList that is only initialized when it is
/// first accessed.
#[repr(C)]
#[derive(Default)] // results in a null pointer, which we will initialize on first access
pub struct MaybeKeyBindingList(Cell<*const KeyBindingList>);

impl MaybeKeyBindingList {
    fn ensure_init(&self) {
        // This would be a race condition in Multi-threaded code, but
        // this type isn't Sync, so this function cannot race with another thread.
        if self.0.get().is_null() {
            self.0.set(Box::leak(Box::default()));
        }
    }
}

impl Drop for MaybeKeyBindingList {
    fn drop(&mut self) {
        let ptr = self.0.replace(core::ptr::null());
        if !ptr.is_null() {
            // SAFETY: Must be a pointer returned by `Box::leak`, which is guaranteed by `ensure_init`.
            drop(unsafe { Box::from_raw(ptr as *mut KeyBindingList) });
        }
    }
}

impl MaybeKeyBindingList {
    fn deref_pin(self: Pin<&Self>) -> Pin<&KeyBindingList> {
        self.ensure_init();
        // SAFETY: Must be non-null and properly aligned, which is guaranteed by `ensure_init`.
        unsafe { Pin::new_unchecked(&*self.get_ref().0.get()) }
    }
}

#[derive(Default)]
#[pin_project::pin_project]
pub struct KeyBindingList {
    found: core::cell::RefCell<Vec<VWeakMapped<ItemTreeVTable, KeyBinding>>>,
    #[pin]
    property_tracker: PropertyTracker,
}

/// A runtime item that exposes key events
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct FocusScope {
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub focus_on_click: Property<bool>,
    pub focus_on_tab_navigation: Property<bool>,
    pub key_pressed: Callback<KeyEventArg, EventResult>,
    pub key_released: Callback<KeyEventArg, EventResult>,
    pub capture_key_pressed: Callback<KeyEventArg, EventResult>,
    pub capture_key_released: Callback<KeyEventArg, EventResult>,
    pub focus_changed_event: Callback<FocusReasonArg>,
    pub focus_gained: Callback<FocusReasonArg>,
    pub focus_lost: Callback<FocusReasonArg>,
    pub key_bindings: MaybeKeyBindingList,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl FocusScope {
    fn visit_enabled_key_bindings<R>(
        self: Pin<&Self>,
        self_rc: &ItemRc,
        mut fun: impl FnMut(&VRcMapped<ItemTreeVTable, KeyBinding>) -> Option<R>,
    ) -> Option<R> {
        let list = Self::FIELD_OFFSETS.key_bindings().apply_pin(self);
        let list = list.deref_pin();

        list.project_ref().property_tracker.evaluate_if_dirty(|| {
            let mut found = list.found.borrow_mut();
            found.clear();

            let mut next = self_rc.first_child();
            while let Some(child) = next {
                if let Some(key_binding) = ItemRc::downcast::<KeyBinding>(&child)
                    && key_binding.as_pin_ref().enabled()
                {
                    found.push(VRcMapped::downgrade(&key_binding));
                }
                next = child.next_sibling();
            }
        });

        let list = list.found.borrow();
        for key_binding in &*list {
            let Some(shortcut) = key_binding.upgrade() else {
                crate::debug_log!("Warning: Found a dropped KeyBinding!");
                continue;
            };
            if let Some(result) = fun(&shortcut) {
                return Some(result);
            }
        }

        None
    }

    /// Returns the first matching key binding and whether there are multiple matches.
    fn key_binding_for_event(
        self: Pin<&Self>,
        self_rc: &ItemRc,
        inter_key_event: &InternalKeyEvent,
    ) -> Option<(VRcMapped<ItemTreeVTable, KeyBinding>, bool)> {
        let mut first_match = None;

        let ambiguous = self.visit_enabled_key_bindings(self_rc, |key_binding| {
            let keys = key_binding.as_pin_ref().keys();
            if keys.matches(&inter_key_event.key_event) {
                match &first_match {
                    Some(key_binding) => {
                        return Some(VRcMapped::clone(key_binding));
                    }
                    None => {
                        first_match = Some(VRcMapped::clone(key_binding));
                    }
                };
            }
            None
        });

        first_match.map(|binding| (VRcMapped::clone(&binding), ambiguous.is_some()))
    }
}

impl Item for FocusScope {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventResult {
        if self.enabled()
            && self.focus_on_click()
            && matches!(event, MouseEvent::Pressed { .. })
            && !self.has_focus()
        {
            WindowInner::from_pub(window_adapter.window()).set_focus_item(
                self_rc,
                true,
                FocusReason::PointerClick,
            );
            InputEventResult::EventAccepted
        } else {
            InputEventResult::EventIgnored
        }
    }

    fn capture_key_event(
        self: Pin<&Self>,
        event: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        let r = match event.event_type {
            KeyEventType::KeyPressed => Self::FIELD_OFFSETS
                .capture_key_pressed()
                .apply_pin(self)
                .call(&(event.key_event.clone(),)),
            KeyEventType::KeyReleased => Self::FIELD_OFFSETS
                .capture_key_released()
                .apply_pin(self)
                .call(&(event.key_event.clone(),)),
            KeyEventType::UpdateComposition | KeyEventType::CommitComposition => {
                EventResult::Reject
            }
        };
        match r {
            EventResult::Accept => KeyEventResult::EventAccepted,
            EventResult::Reject => KeyEventResult::EventIgnored,
        }
    }

    fn key_event(
        self: Pin<&Self>,
        event: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> KeyEventResult {
        let r = match event.event_type {
            KeyEventType::KeyPressed => {
                if let Some((key_binding, ambiguous)) = self.key_binding_for_event(self_rc, event) {
                    if ambiguous {
                        let keys = KeyBinding::FIELD_OFFSETS
                            .keys()
                            .apply_pin(key_binding.as_pin_ref())
                            .get();
                        crate::debug_log!(
                            "Warning: Multiple matching KeyBinding elements for keys {:?}!",
                            keys
                        );
                    }
                    KeyBinding::FIELD_OFFSETS
                        .activated()
                        .apply_pin(key_binding.as_pin_ref())
                        .call(&());
                    EventResult::Accept
                } else {
                    Self::FIELD_OFFSETS
                        .key_pressed()
                        .apply_pin(self)
                        .call(&(event.key_event.clone(),))
                }
            }
            KeyEventType::KeyReleased => {
                let binding = self.key_binding_for_event(self_rc, event);
                if binding.is_some() {
                    // The binding should have already handled the key press,
                    // so we just accept the release event if it matches a binding,
                    // to ensure the events remain "symmetric" in the key-pressed and key-released callbacks.
                    // Either both events appear in the callbacks, or neither do.
                    EventResult::Accept
                } else {
                    Self::FIELD_OFFSETS
                        .key_released()
                        .apply_pin(self)
                        .call(&(event.key_event.clone(),))
                }
            }
            KeyEventType::UpdateComposition | KeyEventType::CommitComposition => {
                EventResult::Reject
            }
        };
        match r {
            EventResult::Accept => KeyEventResult::EventAccepted,
            EventResult::Reject => KeyEventResult::EventIgnored,
        }
    }

    fn focus_event(
        self: Pin<&Self>,
        event: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        if !self.enabled() {
            return FocusEventResult::FocusIgnored;
        }

        match event {
            FocusEvent::FocusIn(reason) => {
                match reason {
                    FocusReason::TabNavigation if !self.focus_on_tab_navigation() => {
                        return FocusEventResult::FocusIgnored;
                    }
                    FocusReason::PointerClick if !self.focus_on_click() => {
                        return FocusEventResult::FocusIgnored;
                    }
                    _ => (),
                };

                self.has_focus.set(true);
                Self::FIELD_OFFSETS.focus_changed_event().apply_pin(self).call(&(*reason,));
                Self::FIELD_OFFSETS.focus_gained().apply_pin(self).call(&(*reason,));
            }
            FocusEvent::FocusOut(reason) => {
                self.has_focus.set(false);
                Self::FIELD_OFFSETS.focus_changed_event().apply_pin(self).call(&(*reason,));
                Self::FIELD_OFFSETS.focus_lost().apply_pin(self).call(&(*reason,));
            }
        }
        FocusEventResult::FocusAccepted
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        mut geometry: LogicalRect,
    ) -> LogicalRect {
        geometry.size = LogicalSize::zero();
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for FocusScope {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        FocusScope,
        CachedRenderingData,
    > = FocusScope::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct SwipeGestureHandler {
    pub enabled: Property<bool>,
    pub handle_swipe_left: Property<bool>,
    pub handle_swipe_right: Property<bool>,
    pub handle_swipe_up: Property<bool>,
    pub handle_swipe_down: Property<bool>,

    pub moved: Callback<VoidArg>,
    pub swiped: Callback<VoidArg>,
    pub cancelled: Callback<VoidArg>,

    pub pressed_position: Property<LogicalPosition>,
    pub current_position: Property<LogicalPosition>,
    pub swiping: Property<bool>,

    // true when the cursor is pressed down and we haven't cancelled yet for another reason
    pressed: Cell<bool>,
    // capture_events: Cell<bool>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for SwipeGestureHandler {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventFilterResult {
        if !self.enabled() {
            if self.pressed.get() {
                self.cancel_impl();
            }
            return InputEventFilterResult::ForwardAndIgnore;
        }

        match event {
            MouseEvent::Pressed { position, button: PointerEventButton::Left, .. } => {
                Self::FIELD_OFFSETS
                    .pressed_position()
                    .apply_pin(self)
                    .set(crate::lengths::logical_position_to_api(*position));
                self.pressed.set(true);
                InputEventFilterResult::DelayForwarding(
                    super::flickable::FORWARD_DELAY.as_millis() as _
                )
            }
            MouseEvent::Exit => {
                self.cancel_impl();
                InputEventFilterResult::ForwardAndIgnore
            }
            MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                if self.swiping() {
                    InputEventFilterResult::Intercept
                } else {
                    self.pressed.set(false);
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::Moved { position, .. } => {
                if self.swiping() {
                    InputEventFilterResult::Intercept
                } else if !self.pressed.get() {
                    InputEventFilterResult::ForwardEvent
                } else if self.is_over_threshold(position) {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::ForwardAndInterceptGrab
                }
            }
            MouseEvent::Wheel { .. } => InputEventFilterResult::ForwardAndIgnore,
            // Not the left button
            MouseEvent::Pressed { .. } | MouseEvent::Released { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => {
                InputEventFilterResult::ForwardAndIgnore
            }
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventResult {
        match event {
            MouseEvent::Pressed { .. } => InputEventResult::GrabMouse,
            MouseEvent::Exit => {
                self.cancel_impl();
                InputEventResult::EventIgnored
            }
            MouseEvent::Released { position, .. } => {
                if !self.pressed.get() && !self.swiping() {
                    return InputEventResult::EventIgnored;
                }
                self.current_position.set(crate::lengths::logical_position_to_api(*position));
                self.pressed.set(false);
                if self.swiping() {
                    Self::FIELD_OFFSETS.swiping().apply_pin(self).set(false);
                    Self::FIELD_OFFSETS.swiped().apply_pin(self).call(&());
                    InputEventResult::EventAccepted
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Moved { position, .. } => {
                if !self.pressed.get() {
                    return InputEventResult::EventAccepted;
                }
                self.current_position.set(crate::lengths::logical_position_to_api(*position));
                let mut swiping = self.swiping();
                if !swiping && self.is_over_threshold(position) {
                    Self::FIELD_OFFSETS.swiping().apply_pin(self).set(true);
                    swiping = true;
                }
                Self::FIELD_OFFSETS.moved().apply_pin(self).call(&());
                if swiping { InputEventResult::GrabMouse } else { InputEventResult::EventAccepted }
            }
            MouseEvent::Wheel { .. } => InputEventResult::EventIgnored,
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. } => {
                InputEventResult::EventIgnored
            }
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => InputEventResult::EventIgnored,
        }
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _event: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        mut geometry: LogicalRect,
    ) -> LogicalRect {
        geometry.size = LogicalSize::zero();
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for SwipeGestureHandler {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

impl SwipeGestureHandler {
    pub fn cancel(self: Pin<&Self>, _: &Rc<dyn WindowAdapter>, _: &ItemRc) {
        self.cancel_impl();
    }

    fn cancel_impl(self: Pin<&Self>) {
        if !self.pressed.replace(false) {
            debug_assert!(!self.swiping());
            return;
        }
        if self.swiping() {
            Self::FIELD_OFFSETS.swiping().apply_pin(self).set(false);
            Self::FIELD_OFFSETS.cancelled().apply_pin(self).call(&());
        }
    }

    fn is_over_threshold(self: Pin<&Self>, position: &LogicalPoint) -> bool {
        let pressed_pos = self.pressed_position();
        let dx = position.x - pressed_pos.x as Coord;
        let dy = position.y - pressed_pos.y as Coord;
        let threshold = super::flickable::DISTANCE_THRESHOLD.get();
        (self.handle_swipe_down() && dy > threshold && dy > dx.abs() / 2 as Coord)
            || (self.handle_swipe_up() && dy < -threshold && dy < -dx.abs() / 2 as Coord)
            || (self.handle_swipe_left() && dx < -threshold && dx < -dy.abs() / 2 as Coord)
            || (self.handle_swipe_right() && dx > threshold && dx > dy.abs() / 2 as Coord)
    }
}

#[cfg(feature = "ffi")]
mod ffi {
    use super::*;

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_swipegesturehandler_cancel(
        s: Pin<&SwipeGestureHandler>,
        window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
        self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
        self_index: u32,
    ) {
        unsafe {
            let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
            let self_rc = ItemRc::new(self_component.clone(), self_index);
            s.cancel(window_adapter, &self_rc);
        }
    }

    /// # Safety
    /// This must be called using a non-null pointer pointing to a chunk of memory big enough to
    /// hold a MaybeKeybindingList
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_maybe_key_binding_list_init(list: *mut MaybeKeyBindingList) {
        unsafe {
            core::ptr::write(list, MaybeKeyBindingList::default());
        }
    }

    /// # Safety
    /// This must be called using a non-null pointer pointing to an initialized MaybeKeybindingList
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_maybe_key_binding_list_free(list: *mut MaybeKeyBindingList) {
        unsafe { core::ptr::drop_in_place(list) };
    }
}

/// The implementation of the `ScaleRotateGestureHandler` element.
///
/// Provides an API surface for platform-recognized pinch gesture events.
/// Receives `MouseEvent::PinchGesture` events via the normal mouse event
/// tree-walk and exposes cumulative scale, center position, and lifecycle callbacks.
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct ScaleRotateGestureHandler {
    pub enabled: Property<bool>,

    // Output properties
    pub active: Property<bool>,
    /// Cumulative scale factor relative to gesture start. Always 1.0 when the
    /// gesture starts, then updated as the gesture progresses (e.g., 2.0 means
    /// doubled, 0.5 means halved).
    pub scale: Property<f32>,
    /// Cumulative rotation in degrees relative to gesture start. Always 0.0 when
    /// the gesture starts.
    pub rotation: Property<f32>,
    pub center: Property<LogicalPosition>,

    // Callbacks
    pub started: Callback<VoidArg>,
    pub updated: Callback<VoidArg>,
    pub ended: Callback<VoidArg>,
    pub cancelled: Callback<VoidArg>,

    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for ScaleRotateGestureHandler {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {
        self.scale.set(1.0);
    }

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _cross_axis_constraint: Coord,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventFilterResult {
        match event {
            // Forward gesture events so inner handlers get first shot
            MouseEvent::PinchGesture { .. } | MouseEvent::RotationGesture { .. }
                if self.enabled() =>
            {
                InputEventFilterResult::ForwardEvent
            }
            // While a gesture is active, intercept non-gesture events to
            // prevent Flickable and other items from processing them concurrently.
            _ if self.active() => InputEventFilterResult::Intercept,
            _ => InputEventFilterResult::ForwardAndIgnore,
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursor,
    ) -> InputEventResult {
        use crate::input::TouchPhase;
        match event {
            MouseEvent::PinchGesture { delta, phase, position } => {
                if !self.enabled() {
                    if self.active() {
                        self.cancel_impl();
                    }
                    return InputEventResult::EventIgnored;
                }
                let new_scale = self.scale() * (1.0 + delta);
                Self::FIELD_OFFSETS.scale().apply_pin(self).set(new_scale);
                let center = crate::lengths::logical_position_to_api(*position);
                Self::FIELD_OFFSETS.center().apply_pin(self).set(center);
                match phase {
                    TouchPhase::Started => {
                        if !self.active() {
                            Self::FIELD_OFFSETS.active().apply_pin(self).set(true);
                            Self::FIELD_OFFSETS.started().apply_pin(self).call(&());
                        }
                        InputEventResult::GrabMouse
                    }
                    TouchPhase::Moved => {
                        if !self.active() {
                            return InputEventResult::EventIgnored;
                        }
                        Self::FIELD_OFFSETS.updated().apply_pin(self).call(&());
                        InputEventResult::GrabMouse
                    }
                    TouchPhase::Ended => self.end_impl(),
                    TouchPhase::Cancelled => {
                        self.cancel_impl();
                        InputEventResult::EventAccepted
                    }
                }
            }
            MouseEvent::RotationGesture { delta, phase, position } => {
                if !self.enabled() {
                    return InputEventResult::EventIgnored;
                }
                let center = crate::lengths::logical_position_to_api(*position);
                Self::FIELD_OFFSETS.center().apply_pin(self).set(center);
                let new_rotation = self.rotation() + delta;
                Self::FIELD_OFFSETS.rotation().apply_pin(self).set(new_rotation);
                match phase {
                    TouchPhase::Started => {
                        if !self.active() {
                            Self::FIELD_OFFSETS.active().apply_pin(self).set(true);
                            Self::FIELD_OFFSETS.started().apply_pin(self).call(&());
                        }
                        InputEventResult::GrabMouse
                    }
                    TouchPhase::Moved => {
                        if !self.active() {
                            return InputEventResult::EventIgnored;
                        }
                        Self::FIELD_OFFSETS.updated().apply_pin(self).call(&());
                        InputEventResult::GrabMouse
                    }
                    TouchPhase::Ended => self.end_impl(),
                    TouchPhase::Cancelled => {
                        self.cancel_impl();
                        InputEventResult::EventAccepted
                    }
                }
            }
            // Grab mouse during active gesture to maintain exclusivity.
            _ if self.active() => InputEventResult::GrabMouse,
            _ => InputEventResult::EventIgnored,
        }
    }

    fn capture_key_event(
        self: Pin<&Self>,
        _: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn key_event(
        self: Pin<&Self>,
        _event: &InternalKeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        KeyEventResult::EventIgnored
    }

    fn focus_event(
        self: Pin<&Self>,
        _: &FocusEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> FocusEventResult {
        FocusEventResult::FocusIgnored
    }

    fn render(
        self: Pin<&Self>,
        _backend: &mut ItemRendererRef,
        _self_rc: &ItemRc,
        _size: LogicalSize,
    ) -> RenderingResult {
        RenderingResult::ContinueRenderingChildren
    }

    fn bounding_rect(
        self: core::pin::Pin<&Self>,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        mut geometry: LogicalRect,
    ) -> LogicalRect {
        geometry.size = LogicalSize::zero();
        geometry
    }

    fn clips_children(self: core::pin::Pin<&Self>) -> bool {
        false
    }
}

impl ItemConsts for ScaleRotateGestureHandler {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

impl ScaleRotateGestureHandler {
    fn cancel_impl(self: Pin<&Self>) {
        if !self.active() {
            return;
        }
        Self::FIELD_OFFSETS.active().apply_pin(self).set(false);
        Self::FIELD_OFFSETS.cancelled().apply_pin(self).call(&());
        // Reset after the callback so handlers can read the last known values
        // to animate back smoothly, matching the pattern where `ended` leaves
        // scale/rotation at their final values.
        Self::FIELD_OFFSETS.scale().apply_pin(self).set(1.0);
        Self::FIELD_OFFSETS.rotation().apply_pin(self).set(0.0);
    }

    fn end_impl(self: Pin<&Self>) -> InputEventResult {
        if !self.active() {
            return InputEventResult::EventIgnored;
        }
        Self::FIELD_OFFSETS.ended().apply_pin(self).call(&());
        Self::FIELD_OFFSETS.active().apply_pin(self).set(false);
        Self::FIELD_OFFSETS.scale().apply_pin(self).set(1.0);
        Self::FIELD_OFFSETS.rotation().apply_pin(self).set(0.0);
        InputEventResult::EventAccepted
    }
}
