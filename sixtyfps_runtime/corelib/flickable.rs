/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//! The implementation details behind the Flickable

use crate::animations::EasingCurve;
use crate::animations::Instant;
use crate::graphics::Point;
use crate::input::{MouseEvent, MouseEventType};
use crate::items::{Flickable, PropertyAnimation, Rectangle};
use core::cell::RefCell;
use core::pin::Pin;

#[derive(Default, Debug)]
struct FlickableDataInnter {
    /// The position in which the press was made
    pressed_pos: Point,
    pressed_time: Option<Instant>,
    pressed_viewport_pos: Point,
}

#[derive(Default, Debug)]
pub struct FlickableData {
    inner: RefCell<FlickableDataInnter>,
}

impl FlickableData {
    pub fn handle_mouse(&self, flick: Pin<&Flickable>, event: MouseEvent) {
        let mut inner = self.inner.borrow_mut();
        match event.what {
            MouseEventType::MousePressed => {
                inner.pressed_pos = event.pos;
                inner.pressed_time = Some(crate::animations::current_tick());
                inner.pressed_viewport_pos = Point::new(
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .get(),
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .get(),
                )
            }
            MouseEventType::MouseExit | MouseEventType::MouseReleased => {
                if let Some(pressed_time) = inner.pressed_time {
                    let dist = event.pos - inner.pressed_pos;
                    let speed = dist
                        / ((crate::animations::current_tick() - pressed_time).as_millis() as f32);

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
                inner.pressed_time = None
            }
            MouseEventType::MouseMoved => {
                if inner.pressed_time.is_some() {
                    let new_pos = ensure_in_bound(
                        flick,
                        inner.pressed_viewport_pos + (event.pos - inner.pressed_pos),
                    );
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.x)
                        .apply_pin(flick)
                        .set(new_pos.x);
                    (Flickable::FIELD_OFFSETS.viewport + Rectangle::FIELD_OFFSETS.y)
                        .apply_pin(flick)
                        .set(new_pos.y);
                }
            }
        }
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
