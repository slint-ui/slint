// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    DragAction, DragActionArg, DropEvent, Item, ItemConsts, ItemRc, MouseCursor,
    PointerEventButton, RenderingResult,
};
use crate::Coord;
use crate::data_transfer::DataTransfer;
use crate::graphics::Image;
use crate::input::{
    FocusEvent, FocusEventResult, InputEventFilterResult, InputEventResult, InternalKeyEvent,
    KeyEventResult, KeyboardModifiers, MouseEvent,
};
use crate::item_rendering::{CachedRenderingData, ItemRenderer};
use crate::layout::{LayoutInfo, Orientation};
use crate::lengths::{LogicalPoint, LogicalRect, LogicalSize};
#[cfg(feature = "rtti")]
use crate::rtti::*;
use crate::window::WindowAdapter;
use crate::{Callback, Property};
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
    pub data: Property<DataTransfer>,
    pub drag_image: Property<Image>,
    pub drag_image_offset_x: Property<i32>,
    pub drag_image_offset_y: Property<i32>,
    pub allow_copy: Property<bool>,
    pub allow_move: Property<bool>,
    pub allow_link: Property<bool>,
    pub preferred_action: Property<DragAction>,
    pub dragging: Property<bool>,
    pub drag_finished: Callback<DragActionArg, ()>,
    pressed: Cell<bool>,
    pressed_position: Cell<LogicalPoint>,
    pub cached_rendering_data: CachedRenderingData,
}

impl Item for DragArea {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _: Orientation,
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
        if !self.enabled() || !self.any_action_allowed() {
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

            MouseEvent::Moved { position, .. } => {
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
            MouseEvent::Pressed { .. } => InputEventResult::EventAccepted,
            MouseEvent::Exit => {
                self.cancel();
                InputEventResult::EventIgnored
            }
            MouseEvent::Released { .. } => {
                self.cancel();
                InputEventResult::EventIgnored
            }
            MouseEvent::Moved { position, .. } => {
                if !self.pressed.get() || !self.enabled() || !self.any_action_allowed() {
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
    > = DragArea::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

impl DragArea {
    fn cancel(self: Pin<&Self>) {
        self.pressed.set(false)
    }

    pub(crate) fn any_action_allowed(self: Pin<&Self>) -> bool {
        self.allow_copy() || self.allow_move() || self.allow_link()
    }

    /// Returns the first allowed of: preferred_action, move, copy, link. None if no action is allowed.
    pub(crate) fn effective_default_action(self: Pin<&Self>) -> DragAction {
        let preferred = self.preferred_action();
        let allowed = |a| match a {
            DragAction::None => false,
            DragAction::Copy => self.allow_copy(),
            DragAction::Move => self.allow_move(),
            DragAction::Link => self.allow_link(),
        };
        if allowed(preferred) {
            return preferred;
        }
        for fallback in [DragAction::Move, DragAction::Copy, DragAction::Link] {
            if allowed(fallback) {
                return fallback;
            }
        }
        DragAction::None
    }

    /// Build the initial DropEvent for a drag starting on this DragArea, populating
    /// the source's allowed actions and seeding `proposed_action` from the preferred default.
    pub(crate) fn initial_drop_event(self: Pin<&Self>) -> DropEvent {
        DropEvent {
            data: self.data(),
            position: Default::default(),
            allow_copy: self.allow_copy(),
            allow_move: self.allow_move(),
            allow_link: self.allow_link(),
            proposed_action: self.effective_default_action(),
        }
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `DropArea` element
pub struct DropArea {
    pub enabled: Property<bool>,
    pub contains_drag: Property<bool>,
    pub current_action: Property<DragAction>,
    pub can_drop: Callback<DropEventArg, DragAction>,
    pub dropped: Callback<DropEventArg, DragAction>,

    pub cached_rendering_data: CachedRenderingData,
}

impl Item for DropArea {
    fn init(self: Pin<&Self>, _self_rc: &ItemRc) {}

    fn deinit(self: Pin<&Self>, _window_adapter: &Rc<dyn WindowAdapter>) {}

    fn layout_info(
        self: Pin<&Self>,
        _: Orientation,
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
        _: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        cursor: &mut MouseCursor,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        match event {
            MouseEvent::DragMove(event) => {
                let raw = Self::FIELD_OFFSETS.can_drop().apply_pin(self).call(&(event.clone(),));
                let chosen = clamp_action_to_allowed(raw, event);
                self.current_action.set(chosen);
                if chosen != DragAction::None {
                    self.contains_drag.set(true);
                    *cursor = cursor_for_action(chosen);
                    InputEventResult::EventAccepted
                } else {
                    self.contains_drag.set(false);
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Drop(event) => {
                self.contains_drag.set(false);
                let returned =
                    Self::FIELD_OFFSETS.dropped().apply_pin(self).call(&(event.clone(),));
                // The target's `dropped` return value is the final action reported back to
                // the source. Clamp against the source's allowed set and stash on
                // `current_action` so the post-dispatch step in `window.rs` can read it.
                self.current_action.set(clamp_action_to_allowed(returned, event));
                InputEventResult::EventAccepted
            }
            MouseEvent::Exit => {
                self.contains_drag.set(false);
                self.current_action.set(DragAction::None);
                InputEventResult::EventIgnored
            }
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
    > = DropArea::FIELD_OFFSETS.cached_rendering_data().as_unpinned_projection();
}

/// Compute the action proposed by the user's current modifier state, clamped to the source's
/// allowed actions. Ctrl alone → copy, Shift alone → move, Ctrl+Shift → link, no modifier →
/// `preferred` with deterministic fallback to the first allowed of move/copy/link.
pub(crate) fn compute_proposed_action(
    modifiers: KeyboardModifiers,
    allow_copy: bool,
    allow_move: bool,
    allow_link: bool,
    preferred: DragAction,
) -> DragAction {
    let allowed = |a| match a {
        DragAction::Copy => allow_copy,
        DragAction::Move => allow_move,
        DragAction::Link => allow_link,
        DragAction::None => false,
    };
    let modifier_request = match (modifiers.control, modifiers.shift) {
        (true, true) => Some(DragAction::Link),
        (true, false) => Some(DragAction::Copy),
        (false, true) => Some(DragAction::Move),
        (false, false) => None,
    };
    if let Some(req) = modifier_request
        && allowed(req)
    {
        return req;
    }
    if allowed(preferred) {
        return preferred;
    }
    for fallback in [DragAction::Move, DragAction::Copy, DragAction::Link] {
        if allowed(fallback) {
            return fallback;
        }
    }
    DragAction::None
}

/// Clamp a `can-drop` return value against the source's allowed actions on the DropEvent.
/// A concrete action the source did not allow becomes `None`.
pub(crate) fn clamp_action_to_allowed(action: DragAction, event: &DropEvent) -> DragAction {
    match action {
        DragAction::None => DragAction::None,
        DragAction::Copy if event.allow_copy => DragAction::Copy,
        DragAction::Move if event.allow_move => DragAction::Move,
        DragAction::Link if event.allow_link => DragAction::Link,
        _ => DragAction::None,
    }
}

/// The cursor shown while a DropArea is hovering an accepted drag.
pub(crate) fn cursor_for_action(action: DragAction) -> MouseCursor {
    match action {
        DragAction::Move => MouseCursor::Move,
        DragAction::Copy => MouseCursor::Copy,
        DragAction::Link => MouseCursor::Alias,
        DragAction::None => MouseCursor::NoDrop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(control: bool, shift: bool) -> KeyboardModifiers {
        KeyboardModifiers { control, shift, alt: false, meta: false }
    }

    #[test]
    fn compute_proposed_action_modifier_table() {
        let preferred = DragAction::Copy;
        // All actions allowed.
        let a = |m| compute_proposed_action(m, true, true, true, preferred);
        assert_eq!(a(modifiers(false, false)), DragAction::Copy);
        assert_eq!(a(modifiers(true, false)), DragAction::Copy);
        assert_eq!(a(modifiers(false, true)), DragAction::Move);
        assert_eq!(a(modifiers(true, true)), DragAction::Link);
    }

    #[test]
    fn compute_proposed_action_falls_back_when_modifier_action_not_allowed() {
        // Source only allows move; user holds Ctrl (asking for copy).
        assert_eq!(
            compute_proposed_action(modifiers(true, false), false, true, false, DragAction::Move),
            DragAction::Move
        );
        // User holds Ctrl+Shift asking for link, only copy allowed; preferred copy.
        assert_eq!(
            compute_proposed_action(modifiers(true, true), true, false, false, DragAction::Copy),
            DragAction::Copy
        );
    }

    #[test]
    fn compute_proposed_action_preferred_clamped_to_allowed_set() {
        // Preferred is move but only copy is allowed; no modifiers — should fall back to copy.
        assert_eq!(
            compute_proposed_action(modifiers(false, false), true, false, false, DragAction::Move),
            DragAction::Copy
        );
        // Preferred None — same behavior, picks first allowed.
        assert_eq!(
            compute_proposed_action(modifiers(false, false), false, false, true, DragAction::None),
            DragAction::Link
        );
        // Nothing allowed at all.
        assert_eq!(
            compute_proposed_action(modifiers(false, false), false, false, false, DragAction::Copy),
            DragAction::None
        );
    }
}
