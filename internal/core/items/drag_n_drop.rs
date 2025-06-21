// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    DropEvent, Item, ItemConsts, ItemRc, MouseCursor, PointerEventButton, RenderingResult,
};
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, KeyEvent,
    KeyEventResult, MouseEvent,
};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalPoint, LogicalRect, LogicalSize};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Callback, Property, SharedString};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::Cell;
use core::pin::Pin;
use i_slint_core_macros::*;

pub type DropEventArg = (DropEvent,);

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `DragArea` element
pub struct DragArea {
    pub enabled: Property<bool>,
    pub mime_type: Property<SharedString>,
    pub data: Property<SharedString>,
    pressed: Cell<bool>,
    pressed_position: Cell<LogicalPoint>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for DragArea {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _: Orientation,
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
    ) -> InputEventFilterResult {
        if !self.enabled() {
            self.cancel();
            return InputEventFilterResult::ForwardAndIgnore;
        }

        match event {
            MouseEvent::Pressed { position, button: PointerEventButton::Left, .. } => {
                self.pressed_position.set(*position);
                self.pressed.set(true);
                InputEventFilterResult::ForwardAndInterceptGrab
            }
            MouseEvent::Exit => {
                self.cancel();
                InputEventFilterResult::ForwardAndIgnore
            }
            MouseEvent::Released { button: PointerEventButton::Left, .. } => {
                self.pressed.set(false);
                InputEventFilterResult::ForwardAndIgnore
            }

            MouseEvent::Moved { position } => {
                if !self.pressed.get() {
                    InputEventFilterResult::ForwardEvent
                } else {
                    let pressed_pos = self.pressed_position.get();
                    let dx = (position.x - pressed_pos.x).abs();
                    let dy = (position.y - pressed_pos.y).abs();
                    let threshold = super::flickable::DISTANCE_THRESHOLD.get();
                    if dy > threshold || dx > threshold {
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
    ) -> InputEventResult {
        match event {
            MouseEvent::Pressed { .. } => InputEventResult::EventAccepted,
            MouseEvent::Exit => {
                self.cancel();
                InputEventResult::EventIgnored
            }
            MouseEvent::Released { .. } => {
                self.cancel();
                InputEventResult::EventIgnored
            }
            MouseEvent::Moved { position } => {
                if !self.pressed.get() || !self.enabled() {
                    return InputEventResult::EventIgnored;
                }
                let pressed_pos = self.pressed_position.get();
                let dx = (position.x - pressed_pos.x).abs();
                let dy = (position.y - pressed_pos.y).abs();
                let threshold = super::flickable::DISTANCE_THRESHOLD.get();
                let start_drag = dx > threshold || dy > threshold;
                if start_drag {
                    self.pressed.set(false);
                    InputEventResult::StartDrag
                } else {
                    InputEventResult::EventAccepted
                }
            }
            MouseEvent::Wheel { .. } => InputEventResult::EventIgnored,
            MouseEvent::DragMove(..) | MouseEvent::Drop(..) => InputEventResult::EventIgnored,
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
        _: &mut &mut dyn ItemRenderer,
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

impl ItemConsts for DragArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        DragArea,
        CachedRenderingData,
    > = DragArea::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}

impl DragArea {
    fn cancel(self: Pin<&Self>) {
        self.pressed.set(false)
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `DropArea` element
pub struct DropArea {
    pub enabled: Property<bool>,
    pub contains_drag: Property<bool>,
    pub can_drop: Callback<DropEventArg, bool>,
    pub dropped: Callback<DropEventArg>,

    pub cached_rendering_data: CachedRenderingData,
}

impl Item for DropArea {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn layout_info(
        self: Pin<&Self>,
        _: Orientation,
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
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        match event {
            MouseEvent::DragMove(event) => {
                let r = Self::FIELD_OFFSETS.can_drop.apply_pin(self).call(&(event.clone(),));
                if r {
                    self.contains_drag.set(true);
                    if let Some(window_adapter) = window_adapter.internal(crate::InternalToken) {
                        window_adapter.set_mouse_cursor(MouseCursor::Copy);
                    }
                    InputEventResult::EventAccepted
                } else {
                    self.contains_drag.set(false);
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Drop(event) => {
                self.contains_drag.set(false);
                Self::FIELD_OFFSETS.dropped.apply_pin(self).call(&(event.clone(),));
                InputEventResult::EventAccepted
            }
            MouseEvent::Exit => {
                self.contains_drag.set(false);
                InputEventResult::EventIgnored
            }
            _ => InputEventResult::EventIgnored,
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
        _: &mut &mut dyn ItemRenderer,
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

impl ItemConsts for DropArea {
    const cached_rendering_data_offset: const_field_offset::FieldOffset<
        DropArea,
        CachedRenderingData,
    > = DropArea::FIELD_OFFSETS.cached_rendering_data.as_unpinned_projection();
}
