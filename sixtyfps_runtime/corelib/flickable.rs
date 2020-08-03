//! The implementation details behind the Flickable

use crate::animations::EasingCurve;
use crate::graphics::Point;
use crate::input::{MouseEvent, MouseEventType};
use crate::items::{Flickable, PropertyAnimation, Rectangle};
use core::cell::RefCell;
use core::pin::Pin;
use instant::Instant;

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
                    (Flickable::field_offsets().viewport + Rectangle::field_offsets().x)
                        .apply_pin(flick)
                        .get(),
                    (Flickable::field_offsets().viewport + Rectangle::field_offsets().y)
                        .apply_pin(flick)
                        .get(),
                )
            }
            MouseEventType::MouseReleased => {
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
                    (Flickable::field_offsets().viewport + Rectangle::field_offsets().x)
                        .apply_pin(flick)
                        .set_animated_value(final_pos.x, &anim);
                    (Flickable::field_offsets().viewport + Rectangle::field_offsets().y)
                        .apply_pin(flick)
                        .set_animated_value(final_pos.y, &anim);
                }
                inner.pressed_time = None
            }
            MouseEventType::MouseMoved => {
                if inner.pressed_time.is_some() {
                    let new_pos = ensure_in_bound(
                        flick,
                        inner.pressed_viewport_pos + (event.pos - inner.pressed_pos),
                    );
                    (Flickable::field_offsets().viewport + Rectangle::field_offsets().x)
                        .apply_pin(flick)
                        .set(new_pos.x);
                    (Flickable::field_offsets().viewport + Rectangle::field_offsets().y)
                        .apply_pin(flick)
                        .set(new_pos.y);
                }
            }
        }
    }
}

/// Make sure that the point is within the bounds
fn ensure_in_bound(flick: Pin<&Flickable>, p: Point) -> Point {
    let w = (Flickable::field_offsets().width).apply_pin(flick).get();
    let h = (Flickable::field_offsets().height).apply_pin(flick).get();
    /*let vw = (Flickable::field_offsets().viewport + Rectangle::field_offsets().width)
        .apply_pin(flick)
        .get();
    let vh = (Flickable::field_offsets().viewport + Rectangle::field_offsets().height)
        .apply_pin(flick)
        .get();*/
    let (vw, vh) = (1000., 1000.); // FIXME: should be the actual viewport

    let min = Point::new(w - vw, h - vh);
    let max = Point::new(0., 0.);
    p.max(min).min(max)
}
