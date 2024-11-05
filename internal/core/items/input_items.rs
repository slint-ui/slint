// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    EventResult, Item, ItemConsts, ItemRc, ItemRendererRef, KeyEventArg, MouseCursor, PointerEvent,
    PointerEventArg, PointerEventButton, PointerEventKind, PointerScrollEvent,
    PointerScrollEventArg, RenderingResult, VoidArg,
};
use crate::api::LogicalPosition;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, KeyEventType, MouseEvent,
};
use crate::item_rendering::CachedRenderingData;
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalLength, LogicalPoint, LogicalRect, LogicalSize, PointLengths};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::{WindowAdapter, WindowInner};
use crate::{Callback, Coord, Property};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::Cell;
use core::pin::Pin;
use i_slint_core_macros::*;

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

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        if !self.enabled() {
            self.has_hover.set(false);
            if self.grabbed.replace(false) {
                self.pressed.set(false);
                Self::FIELD_OFFSETS.pointer_event.apply_pin(self).call(&(PointerEvent {
                    button: PointerEventButton::Other,
                    kind: PointerEventKind::Cancel,
                    modifiers: window_adapter.window().0.modifiers.get().into(),
                },));
            }
            return InputEventFilterResult::ForwardAndIgnore;
        }
        if let Some(pos) = event.position() {
            Self::FIELD_OFFSETS.mouse_x.apply_pin(self).set(pos.x_length());
            Self::FIELD_OFFSETS.mouse_y.apply_pin(self).set(pos.y_length());
        }
        let hovering = !matches!(event, MouseEvent::Exit);
        Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(hovering);
        if hovering {
            if let Some(x) = window_adapter.internal(crate::InternalToken) {
                x.set_mouse_cursor(self.mouse_cursor());
            }
        }
        InputEventFilterResult::ForwardAndInterceptGrab
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        if matches!(event, MouseEvent::Exit) {
            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false);
            if let Some(x) = window_adapter.internal(crate::InternalToken) {
                x.set_mouse_cursor(MouseCursor::Default);
            }
        }
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }

        match event {
            MouseEvent::Pressed { position, button, .. } => {
                self.grabbed.set(true);
                if button == PointerEventButton::Left {
                    Self::FIELD_OFFSETS.pressed_x.apply_pin(self).set(position.x_length());
                    Self::FIELD_OFFSETS.pressed_y.apply_pin(self).set(position.y_length());
                    Self::FIELD_OFFSETS.pressed.apply_pin(self).set(true);
                }
                Self::FIELD_OFFSETS.pointer_event.apply_pin(self).call(&(PointerEvent {
                    button,
                    kind: PointerEventKind::Down,
                    modifiers: window_adapter.window().0.modifiers.get().into(),
                },));

                InputEventResult::GrabMouse
            }
            MouseEvent::Exit => {
                Self::FIELD_OFFSETS.pressed.apply_pin(self).set(false);
                if self.grabbed.replace(false) {
                    Self::FIELD_OFFSETS.pointer_event.apply_pin(self).call(&(PointerEvent {
                        button: PointerEventButton::Other,
                        kind: PointerEventKind::Cancel,
                        modifiers: window_adapter.window().0.modifiers.get().into(),
                    },));
                }

                InputEventResult::EventAccepted
            }

            MouseEvent::Released { button, position, click_count } => {
                let geometry = self_rc.geometry();
                if button == PointerEventButton::Left
                    && LogicalRect::new(LogicalPoint::default(), geometry.size).contains(position)
                    && self.pressed()
                {
                    Self::FIELD_OFFSETS.clicked.apply_pin(self).call(&());
                    if (click_count % 2) == 1 {
                        Self::FIELD_OFFSETS.double_clicked.apply_pin(self).call(&())
                    }
                }

                self.grabbed.set(false);
                if button == PointerEventButton::Left {
                    Self::FIELD_OFFSETS.pressed.apply_pin(self).set(false);
                }
                Self::FIELD_OFFSETS.pointer_event.apply_pin(self).call(&(PointerEvent {
                    button,
                    kind: PointerEventKind::Up,
                    modifiers: window_adapter.window().0.modifiers.get().into(),
                },));

                InputEventResult::EventAccepted
            }
            MouseEvent::Moved { .. } => {
                Self::FIELD_OFFSETS.pointer_event.apply_pin(self).call(&(PointerEvent {
                    button: PointerEventButton::Other,
                    kind: PointerEventKind::Move,
                    modifiers: window_adapter.window().0.modifiers.get().into(),
                },));
                return if self.grabbed.get() {
                    Self::FIELD_OFFSETS.moved.apply_pin(self).call(&());
                    InputEventResult::GrabMouse
                } else {
                    InputEventResult::EventAccepted
                };
            }
            MouseEvent::Wheel { delta_x, delta_y, .. } => {
                let modifiers = window_adapter.window().0.modifiers.get().into();
                let r = Self::FIELD_OFFSETS
                    .scroll_event
                    .apply_pin(self)
                    .call(&(PointerScrollEvent { delta_x, delta_y, modifiers },));
                if self.grabbed.get() {
                    InputEventResult::GrabMouse
                } else {
                    match r {
                        EventResult::Reject => {
                            // We are ignoring the event, so we will be removed from the item_stack,
                            // therefore we must remove the has_hover flag as there might be a scroll under us.
                            // It will be put back later.
                            Self::FIELD_OFFSETS.has_hover.apply_pin(self).set(false);
                            InputEventResult::EventIgnored
                        }
                        EventResult::Accept => InputEventResult::EventAccepted,
                    }
                }
            }
        }
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
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
}

impl ItemConsts for TouchArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        TouchArea,
        CachedRenderingData,
    > = TouchArea::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

/// A runtime item that exposes key
#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
pub struct FocusScope {
    pub enabled: Property<bool>,
    pub has_focus: Property<bool>,
    pub key_pressed: Callback<KeyEventArg, EventResult>,
    pub key_released: Callback<KeyEventArg, EventResult>,
    pub focus_changed_event: Callback<VoidArg>,
    /// FIXME: remove this
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for FocusScope {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        _: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        self_rc: &ItemRc,
    ) -> InputEventResult {
        if self.enabled() && matches!(event, MouseEvent::Pressed { .. }) && !self.has_focus() {
            WindowInner::from_pub(window_adapter.window()).set_focus_item(self_rc, true);
            InputEventResult::EventAccepted
        } else {
            InputEventResult::EventIgnored
        }
    }

    fn key_event(
        self: Pin<&Self>,
        event: &KeyEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> KeyEventResult {
        let r = match event.event_type {
            KeyEventType::KeyPressed => {
                Self::FIELD_OFFSETS.key_pressed.apply_pin(self).call(&(event.clone(),))
            }
            KeyEventType::KeyReleased => {
                Self::FIELD_OFFSETS.key_released.apply_pin(self).call(&(event.clone(),))
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
            FocusEvent::FocusIn | FocusEvent::WindowReceivedFocus => {
                self.has_focus.set(true);
                Self::FIELD_OFFSETS.focus_changed_event.apply_pin(self).call(&());
            }
            FocusEvent::FocusOut | FocusEvent::WindowLostFocus => {
                self.has_focus.set(false);
                Self::FIELD_OFFSETS.focus_changed_event.apply_pin(self).call(&());
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
}

impl ItemConsts for FocusScope {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        FocusScope,
        CachedRenderingData,
    > = FocusScope::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
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

    fn layout_info(
        self: Pin<&Self>,
        _orientation: Orientation,
        _window_adapter: &Rc<dyn WindowAdapter>,
    ) -> LayoutInfo {
        LayoutInfo { stretch: 1., ..LayoutInfo::default() }
    }

    fn input_event_filter_before_children(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
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
                    .pressed_position
                    .apply_pin(self)
                    .set(crate::lengths::logical_position_to_api(position));
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
            MouseEvent::Moved { position } => {
                if self.swiping() {
                    InputEventFilterResult::Intercept
                } else if !self.pressed.get() {
                    InputEventFilterResult::ForwardEvent
                } else {
                    let pressed_pos = self.pressed_position();
                    let dx = position.x - pressed_pos.x as Coord;
                    let dy = position.y - pressed_pos.y as Coord;
                    let threshold = super::flickable::DISTANCE_THRESHOLD.get();
                    if (self.handle_swipe_down() && dy > threshold)
                        || (self.handle_swipe_up() && dy < -threshold)
                        || (self.handle_swipe_left() && dx < -threshold)
                        || (self.handle_swipe_right() && dx > threshold)
                    {
                        InputEventFilterResult::Intercept
                    } else {
                        InputEventFilterResult::ForwardAndInterceptGrab
                    }
                }
            }
            MouseEvent::Wheel { .. } => InputEventFilterResult::ForwardAndIgnore,
            // Not the left button
            MouseEvent::Pressed { .. } | MouseEvent::Released { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
        }
    }

    fn input_event(
        self: Pin<&Self>,
        event: MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
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
                self.current_position.set(crate::lengths::logical_position_to_api(position));
                self.pressed.set(false);
                if self.swiping() {
                    Self::FIELD_OFFSETS.swiping.apply_pin(self).set(false);
                    Self::FIELD_OFFSETS.swiped.apply_pin(self).call(&());
                    InputEventResult::EventAccepted
                } else {
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Moved { position } => {
                if !self.pressed.get() {
                    return InputEventResult::EventIgnored;
                }
                self.current_position.set(crate::lengths::logical_position_to_api(position));
                if !self.swiping() {
                    let pressed_pos = self.pressed_position();
                    let dx = position.x - pressed_pos.x as Coord;
                    let dy = position.y - pressed_pos.y as Coord;
                    let threshold = super::flickable::DISTANCE_THRESHOLD.get();
                    let start_swipe = (self.handle_swipe_down() && dy > threshold)
                        || (self.handle_swipe_up() && dy < -threshold)
                        || (self.handle_swipe_left() && dx < -threshold)
                        || (self.handle_swipe_right() && dx > threshold);

                    if start_swipe {
                        Self::FIELD_OFFSETS.swiping.apply_pin(self).set(true);
                    }
                }
                Self::FIELD_OFFSETS.moved.apply_pin(self).call(&());
                InputEventResult::GrabMouse
            }
            MouseEvent::Wheel { .. } => InputEventResult::EventIgnored,
        }
    }

    fn key_event(
        self: Pin<&Self>,
        _: &KeyEvent,
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
}

impl ItemConsts for SwipeGestureHandler {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<Self, CachedRenderingData> =
        Self::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
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
            Self::FIELD_OFFSETS.swiping.apply_pin(self).set(false);
            Self::FIELD_OFFSETS.cancelled.apply_pin(self).call(&());
        }
    }
}

#[cfg(feature = "ffi")]
#[no_mangle]
pub unsafe extern "C" fn slint_swipegesturehandler_cancel(
    s: Pin<&SwipeGestureHandler>,
    window_adapter: *const crate::window::ffi::WindowAdapterRcOpaque,
    self_component: &vtable::VRc<crate::item_tree::ItemTreeVTable>,
    self_index: u32,
) {
    let window_adapter = &*(window_adapter as *const Rc<dyn WindowAdapter>);
    let self_rc = ItemRc::new(self_component.clone(), self_index);
    s.cancel(window_adapter, &self_rc);
}
