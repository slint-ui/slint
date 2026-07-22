// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::{
    BuiltInMouseCursor, DragAction, DragActionArg, DropEvent, Item, ItemConsts, ItemRc,
    PointerEventButton, RenderingResult,
};
use crate::Coord;
use crate::cursor::MouseCursorInner;
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

/// The set of actions a drag source permits, captured when the drag starts.
#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct AllowedDragActions {
    pub copy: bool,
    pub move_: bool,
    pub link: bool,
}

impl AllowedDragActions {
    /// True if at least one action is permitted.
    pub fn any(self) -> bool {
        self.copy || self.move_ || self.link
    }
}

/// Filter step of the "left press, then drag past the threshold" gesture shared by
/// `DragArea` and `WindowMoveArea`: tracks the press in the item's `pressed`/
/// `pressed_position` cells and intercepts the children's events once the threshold is
/// crossed. The caller handles its disabled/precondition case before calling this.
pub(super) fn press_drag_filter(
    pressed: &Cell<bool>,
    pressed_position: &Cell<LogicalPoint>,
    event: &MouseEvent,
) -> InputEventFilterResult {
    match event {
        MouseEvent::Pressed { position, button: PointerEventButton::Left, .. } => {
            pressed_position.set(*position);
            pressed.set(true);
            InputEventFilterResult::ForwardAndInterceptGrab
        }
        MouseEvent::Exit => {
            pressed.set(false);
            InputEventFilterResult::ForwardAndIgnore
        }
        MouseEvent::Released { button: PointerEventButton::Left, .. } => {
            pressed.set(false);
            InputEventFilterResult::ForwardAndIgnore
        }
        MouseEvent::Moved { position, .. } => {
            if !pressed.get() {
                InputEventFilterResult::ForwardEvent
            } else if exceeds_drag_threshold(pressed_position.get(), *position) {
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
        MouseEvent::DragMove { .. } | MouseEvent::Drop { .. } => {
            InputEventFilterResult::ForwardAndIgnore
        }
    }
}

/// True once the pointer has moved far enough from the press position to count as a
/// drag rather than a click.
pub(super) fn exceeds_drag_threshold(
    pressed_position: LogicalPoint,
    position: LogicalPoint,
) -> bool {
    let dx = (position.x - pressed_position.x).abs();
    let dy = (position.y - pressed_position.y).abs();
    let threshold = super::flickable::DISTANCE_THRESHOLD.get();
    dx > threshold || dy > threshold
}

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
        _: &mut MouseCursorInner,
    ) -> InputEventFilterResult {
        if !self.enabled() || !self.allowed_actions().any() || self.data().is_empty() {
            self.cancel();
            return InputEventFilterResult::ForwardAndIgnore;
        }
        press_drag_filter(&self.pressed, &self.pressed_position, event)
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        _window_adapter: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        _: &mut MouseCursorInner,
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
                if !self.pressed.get()
                    || !self.enabled()
                    || !self.allowed_actions().any()
                    || self.data().is_empty()
                {
                    return InputEventResult::EventIgnored;
                }
                let start_drag = exceeds_drag_threshold(self.pressed_position.get(), *position);
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
            MouseEvent::DragMove { .. } | MouseEvent::Drop { .. } => InputEventResult::EventIgnored,
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

    pub(crate) fn allowed_actions(self: Pin<&Self>) -> AllowedDragActions {
        AllowedDragActions {
            copy: self.allow_copy(),
            move_: self.allow_move(),
            link: self.allow_link(),
        }
    }

    /// Build the initial DropEvent for a drag starting on this DragArea, together with the
    /// source's allowed actions. `proposed_action` is seeded from the default action (the first
    /// allowed of move, copy, link).
    pub(crate) fn initial_drop_event(self: Pin<&Self>) -> (DropEvent, AllowedDragActions) {
        let allowed = self.allowed_actions();
        let event = DropEvent {
            data: self.data(),
            position: Default::default(),
            proposed_action: compute_proposed_action(KeyboardModifiers::default(), allowed),
        };
        (event, allowed)
    }

    /// Clear the `dragging` flag and fire the `drag-finished` callback with the final negotiated
    /// action. Shared by the in-window drag path and the native backends.
    pub(crate) fn finish_drag(self: Pin<&Self>, action: DragAction) {
        self.dragging.set(false);
        Self::FIELD_OFFSETS.drag_finished().apply_pin(self).call(&(action,));
    }
}

#[repr(C)]
#[derive(FieldOffsets, Default, SlintElement)]
#[pin]
/// The implementation of the `DropArea` element
pub struct DropArea {
    pub enabled: Property<bool>,
    pub has_drag: Property<bool>,
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
        _: &mut MouseCursorInner,
    ) -> InputEventFilterResult {
        InputEventFilterResult::ForwardEvent
    }

    fn input_event(
        self: Pin<&Self>,
        event: &MouseEvent,
        _: &Rc<dyn WindowAdapter>,
        _self_rc: &ItemRc,
        cursor: &mut MouseCursorInner,
    ) -> InputEventResult {
        if !self.enabled() {
            return InputEventResult::EventIgnored;
        }
        match event {
            MouseEvent::DragMove { event, allowed } => {
                let raw = Self::FIELD_OFFSETS.can_drop().apply_pin(self).call(&(event.clone(),));
                let chosen = clamp_action_to_allowed(raw, *allowed);
                self.current_action.set(chosen);
                if chosen != DragAction::None {
                    self.has_drag.set(true);
                    *cursor = MouseCursorInner::BuiltIn(cursor_for_action(chosen));
                    InputEventResult::EventAccepted
                } else {
                    self.has_drag.set(false);
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::Drop { event, allowed } => {
                self.has_drag.set(false);
                let returned =
                    Self::FIELD_OFFSETS.dropped().apply_pin(self).call(&(event.clone(),));
                // The target's `dropped` return value is the final action reported back to
                // the source. Clamp against the source's allowed set and stash on
                // `current_action` so the post-dispatch step in `window.rs` can read it.
                self.current_action.set(clamp_action_to_allowed(returned, *allowed));
                InputEventResult::EventAccepted
            }
            MouseEvent::Exit => {
                self.has_drag.set(false);
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
/// the first allowed of move/copy/link.
pub fn compute_proposed_action(
    modifiers: KeyboardModifiers,
    allowed_actions: AllowedDragActions,
) -> DragAction {
    let allowed = |a| match a {
        DragAction::Copy => allowed_actions.copy,
        DragAction::Move => allowed_actions.move_,
        DragAction::Link => allowed_actions.link,
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
    for fallback in [DragAction::Move, DragAction::Copy, DragAction::Link] {
        if allowed(fallback) {
            return fallback;
        }
    }
    DragAction::None
}

/// Clamp a `can-drop` return value against the source's allowed actions.
/// A concrete action the source did not allow becomes `None`.
pub(crate) fn clamp_action_to_allowed(
    action: DragAction,
    allowed: AllowedDragActions,
) -> DragAction {
    match action {
        DragAction::None => DragAction::None,
        DragAction::Copy if allowed.copy => DragAction::Copy,
        DragAction::Move if allowed.move_ => DragAction::Move,
        DragAction::Link if allowed.link => DragAction::Link,
        _ => DragAction::None,
    }
}

/// The cursor shown while a DropArea is hovering an accepted drag.
pub(crate) fn cursor_for_action(action: DragAction) -> BuiltInMouseCursor {
    match action {
        DragAction::Move => BuiltInMouseCursor::Move,
        DragAction::Copy => BuiltInMouseCursor::Copy,
        DragAction::Link => BuiltInMouseCursor::Alias,
        DragAction::None => BuiltInMouseCursor::NoDrop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modifiers(control: bool, shift: bool) -> KeyboardModifiers {
        KeyboardModifiers { control, shift, alt: false, meta: false }
    }

    const ALL: AllowedDragActions = AllowedDragActions { copy: true, move_: true, link: true };
    const COPY_ONLY: AllowedDragActions =
        AllowedDragActions { copy: true, move_: false, link: false };
    const MOVE_ONLY: AllowedDragActions =
        AllowedDragActions { copy: false, move_: true, link: false };
    const LINK_ONLY: AllowedDragActions =
        AllowedDragActions { copy: false, move_: false, link: true };
    const COPY_AND_MOVE: AllowedDragActions =
        AllowedDragActions { copy: true, move_: true, link: false };

    #[test]
    fn compute_proposed_action_modifier_table() {
        // All actions allowed.
        let a = |m| compute_proposed_action(m, ALL);
        assert_eq!(a(modifiers(false, false)), DragAction::Move);
        assert_eq!(a(modifiers(true, false)), DragAction::Copy);
        assert_eq!(a(modifiers(false, true)), DragAction::Move);
        assert_eq!(a(modifiers(true, true)), DragAction::Link);
    }

    #[test]
    fn compute_proposed_action_falls_back_when_modifier_action_not_allowed() {
        // Source only allows move; user holds Ctrl (asking for copy).
        assert_eq!(compute_proposed_action(modifiers(true, false), MOVE_ONLY), DragAction::Move);
        // User holds Ctrl+Shift asking for link, only copy allowed.
        assert_eq!(compute_proposed_action(modifiers(true, true), COPY_ONLY), DragAction::Copy);
    }

    #[test]
    fn compute_proposed_action_default_is_first_allowed() {
        // No modifiers: the first allowed of move, copy, link wins.
        assert_eq!(
            compute_proposed_action(modifiers(false, false), COPY_AND_MOVE),
            DragAction::Move
        );
        assert_eq!(compute_proposed_action(modifiers(false, false), COPY_ONLY), DragAction::Copy);
        assert_eq!(compute_proposed_action(modifiers(false, false), LINK_ONLY), DragAction::Link);
        // Nothing allowed at all.
        assert_eq!(
            compute_proposed_action(modifiers(false, false), AllowedDragActions::default()),
            DragAction::None
        );
    }
}
