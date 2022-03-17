// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

//! The implementation details behind the Flickable

use core::time::Duration;

use crate::animations::EasingCurve;
use crate::animations::Instant;
use crate::graphics::Point;
use crate::input::{InputEventFilterResult, InputEventResult, MouseEvent};
use crate::items::PointerEventButton;
use crate::items::{Flickable, PropertyAnimation, Rectangle};
use core::cell::RefCell;
use core::pin::Pin;
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// The distance required before it starts flicking if there is another item intercepting the mouse.
const DISTANCE_THRESHOLD: f32 = 8.;
/// Time required before we stop caring about child event if the mouse hasn't been moved
const DURATION_THRESHOLD: Duration = Duration::from_millis(500);

#[derive(Default, Debug)]
struct FlickableDataInner {
    /// The position in which the press was made
    pressed_pos: Point,
    pressed_time: Option<Instant>,
    pressed_viewport_pos: Point,
    /// Set to true if the flickable is flicking and capturing all mouse event, not forwarding back to the children
    capture_events: bool,
}

#[derive(Default, Debug)]
pub struct FlickableData {
    inner: RefCell<FlickableDataInner>,
}

impl FlickableData {
    pub fn handle_mouse_filter(
        &self,
        flick: Pin<&Flickable>,
        event: MouseEvent,
    ) -> InputEventFilterResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::MousePressed { pos, button: PointerEventButton::left } => {
                inner.pressed_pos = pos;
                inner.pressed_time = Some(crate::animations::current_tick());
                inner.pressed_viewport_pos = Point::new(
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .get(),
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .get(),
                );
                if inner.capture_events {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::ForwardAndInterceptGrab
                }
            }
            MouseEvent::MouseExit
            | MouseEvent::MouseReleased { button: PointerEventButton::left, .. } => {
                let was_capturing = inner.capture_events;
                Self::mouse_released(&mut inner, flick, event);
                if was_capturing {
                    InputEventFilterResult::Intercept
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::MouseMoved { pos } => {
                let do_intercept = inner.capture_events
                    || inner.pressed_time.map_or(false, |pressed_time| {
                        if crate::animations::current_tick() - pressed_time > DURATION_THRESHOLD {
                            return false;
                        }
                        let can_move_horiz = (Flickable::FIELD_OFFSETS.viewport
                            + Rectangle::FIELD_OFFSETS.width)
                            .apply_pin(flick)
                            .get()
                            > flick.width();
                        let can_move_vert = (Flickable::FIELD_OFFSETS.viewport
                            + Rectangle::FIELD_OFFSETS.height)
                            .apply_pin(flick)
                            .get()
                            > flick.height();
                        let diff = pos - inner.pressed_pos;
                        (can_move_horiz && diff.x.abs() > DISTANCE_THRESHOLD)
                            || (can_move_vert && diff.y.abs() > DISTANCE_THRESHOLD)
                    });
                if do_intercept {
                    InputEventFilterResult::Intercept
                } else if inner.pressed_time.is_some() {
                    InputEventFilterResult::ForwardAndInterceptGrab
                } else {
                    InputEventFilterResult::ForwardEvent
                }
            }
            MouseEvent::MouseWheel { .. } => InputEventFilterResult::Intercept,
            // Not the left button
            MouseEvent::MousePressed { .. } | MouseEvent::MouseReleased { .. } => {
                InputEventFilterResult::ForwardAndIgnore
            }
        }
    }

    pub fn handle_mouse(&self, flick: Pin<&Flickable>, event: MouseEvent) -> InputEventResult {
        let mut inner = self.inner.borrow_mut();
        match event {
            MouseEvent::MousePressed { .. } => {
                inner.capture_events = true;
                InputEventResult::GrabMouse
            }
            MouseEvent::MouseExit | MouseEvent::MouseReleased { .. } => {
                Self::mouse_released(&mut inner, flick, event);
                InputEventResult::EventAccepted
            }
            MouseEvent::MouseMoved { pos } => {
                if inner.pressed_time.is_some() {
                    inner.capture_events = true;
                    let new_pos = ensure_in_bound(
                        flick,
                        inner.pressed_viewport_pos + (pos - inner.pressed_pos),
                    );
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .set(new_pos.x);
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .set(new_pos.y);
                    InputEventResult::GrabMouse
                } else {
                    inner.capture_events = false;
                    InputEventResult::EventIgnored
                }
            }
            MouseEvent::MouseWheel { delta, .. } => {
                let old_pos = Point::new(
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .get(),
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .get(),
                );
                let new_pos = ensure_in_bound(flick, old_pos + delta.to_vector());
                (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                    .apply_pin(flick)
                    .set(new_pos.x);
                (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                    .apply_pin(flick)
                    .set(new_pos.y);
                InputEventResult::EventAccepted
            }
        }
    }

    fn mouse_released(inner: &mut FlickableDataInner, flick: Pin<&Flickable>, event: MouseEvent) {
        if let (Some(pressed_time), Some(pos)) = (inner.pressed_time, event.pos()) {
            let dist = pos - inner.pressed_pos;
            let speed =
                dist / ((crate::animations::current_tick() - pressed_time).as_millis() as f32);

            let duration = 100;
            let final_pos = ensure_in_bound(
                flick,
                inner.pressed_viewport_pos + dist + speed * (duration as f32),
            );
            let anim = PropertyAnimation {
                duration,
                easing: EasingCurve::CubicBezier([0.0, 0.0, 0.58, 1.0]),
                ..PropertyAnimation::default()
            };
            (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                .apply_pin(flick)
                .set_animated_value(final_pos.x, anim.clone());
            (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                .apply_pin(flick)
                .set_animated_value(final_pos.y, anim);
        }
        inner.capture_events = false; // FIXME: should only be set to false once the flick animation is over
        inner.pressed_time = None;
    }
}

/// Make sure that the point is within the bounds
fn ensure_in_bound(flick: Pin<&Flickable>, p: Point) -> Point {
    let w = flick.width();
    let h = flick.height();
    let vw =
        (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.width).apply_pin(flick).get();
    let vh = (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.height)
        .apply_pin(flick)
        .get();

    let min = Point::new(w - vw, h - vh);
    let max = Point::new(0., 0.);
    p.max(min).min(max)
}
