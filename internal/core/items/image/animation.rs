// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Property;
use crate::animations::Instant;
use crate::graphics::{AnimatedImage, Frame, FrameDecoder, Image, OpaqueImageVTable};
use crate::timers::{Timer, TimerMode};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use core::time::Duration;

#[derive(FieldOffsets)]
#[pin]
pub(super) struct ImageAnimation {
    redraw: Property<()>,
    start: Instant,
    state: RefCell<State>,
    timer: Timer,
    /// The `frame_end` that the timer was started for.
    scheduled: Cell<Option<Duration>>,
}

/// Positions are durations of playback since the start of the first play.
struct State {
    image: vtable::VRc<OpaqueImageVTable, AnimatedImage>,
    decoder: Option<FrameDecoder<'static>>,
    frame: Image,
    /// The last frame `seek` skipped, shown if the animation ends on it.
    skipped: Option<Frame>,
    frame_end: Duration,
    play_duration: Option<Duration>,
    completed_plays: u32,
}

impl State {
    fn new(image: vtable::VRc<OpaqueImageVTable, AnimatedImage>) -> Self {
        let mut state = Self {
            frame: image.first_frame(),
            image,
            decoder: None,
            skipped: None,
            frame_end: Duration::ZERO,
            play_duration: None,
            completed_plays: 0,
        };
        state.start_play(Duration::ZERO, Duration::ZERO);
        state
    }

    fn finished(&self) -> bool {
        self.decoder.is_none()
    }

    fn start_play(&mut self, play_start: Duration, position: Duration) {
        self.decoder = self.image.frames();
        self.frame_end = play_start;
        if !self.next_frame(position) {
            self.finish();
        }
    }

    fn next_frame(&mut self, position: Duration) -> bool {
        let Some(frame) = self.decoder.as_mut().and_then(FrameDecoder::next_frame) else {
            return false;
        };
        self.frame_end += frame.delay();
        if position < self.frame_end {
            self.frame = Image::from_rgba8_premultiplied(frame.into_pixels());
            self.skipped = None;
        } else {
            self.skipped = Some(frame);
        }
        true
    }

    fn finish(&mut self) {
        self.decoder = None;
        if let Some(frame) = self.skipped.take() {
            self.frame = Image::from_rgba8_premultiplied(frame.into_pixels());
        }
    }

    fn advance(&mut self, position: Duration) {
        if self.next_frame(position) {
            return;
        }
        let play_duration = *self.play_duration.get_or_insert(self.frame_end);
        self.completed_plays += 1;
        let remaining = self.image.plays().map(|plays| plays.get() - self.completed_plays);
        if remaining == Some(0) {
            self.finish();
            return;
        }
        // Decode the last play, which ends on its last frame.
        let skippable = (position - self.frame_end).as_nanos() / play_duration.as_nanos();
        let skip =
            remaining.map_or(skippable, |remaining| skippable.min(u128::from(remaining) - 1));
        let skip = u32::try_from(skip).unwrap_or(u32::MAX);
        self.completed_plays = self.completed_plays.saturating_add(skip);
        self.start_play(self.frame_end + play_duration * skip, position);
    }

    fn seek(&mut self, position: Duration) {
        while !self.finished() && position >= self.frame_end {
            self.advance(position);
        }
    }
}

impl ImageAnimation {
    pub(super) fn new(image: vtable::VRc<OpaqueImageVTable, AnimatedImage>) -> Pin<Rc<Self>> {
        Rc::pin(Self {
            redraw: Property::new(()),
            start: now(),
            state: RefCell::new(State::new(image)),
            timer: Timer::default(),
            scheduled: Cell::new(None),
        })
    }

    pub(super) fn shows(&self, image: &vtable::VRc<OpaqueImageVTable, AnimatedImage>) -> bool {
        *self.state.borrow().image == **image
    }

    /// Returns the frame to show now, and schedules a redraw for when it changes.
    pub(super) fn frame(self: &Pin<Rc<Self>>) -> Image {
        let position = now() - self.start;
        let mut state = self.state.borrow_mut();
        let shown_until = state.frame_end;
        state.seek(position);
        let advanced = state.frame_end != shown_until;
        let schedule = (!state.finished()).then_some(state.frame_end);
        let frame = state.frame.clone();
        drop(state);

        match schedule {
            Some(frame_end) if self.scheduled.get() != Some(frame_end) || !self.timer.running() => {
                self.scheduled.set(Some(frame_end));
                let weak = pin_weak::rc::PinWeak::downgrade(self.clone());
                let delay = (frame_end - position) * crate::animations::slow_down_factor();
                self.timer.start(TimerMode::SingleShot, delay, move || {
                    if let Some(this) = weak.upgrade() {
                        Self::FIELD_OFFSETS.redraw().apply_pin(this.as_ref()).mark_dirty();
                    }
                });
            }
            Some(_) => {}
            None => {
                self.scheduled.set(None);
                self.timer.stop();
            }
        }
        let redraw = Self::FIELD_OFFSETS.redraw().apply_pin(self.as_ref());
        // Renderers that cache the item's texture can call this outside of the cache's tracker.
        if advanced {
            redraw.mark_dirty();
        }
        redraw.get();
        frame
    }
}

fn now() -> Instant {
    // A tracked read would redraw the item on every animation tick.
    crate::properties::evaluate_no_tracking(crate::animations::current_tick)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::SharedImageBuffer;
    use crate::properties::PropertyTracker;

    fn animated(data: &[u8]) -> vtable::VRc<OpaqueImageVTable, AnimatedImage> {
        let image = Image::load_from_data(data, Some("png")).unwrap();
        let crate::graphics::ImageInner::AnimatedImage(animated) = image.0 else {
            panic!("expected an animation")
        };
        animated
    }

    fn state(data: &[u8]) -> State {
        State::new(animated(data))
    }

    fn color(state: &State) -> [u8; 3] {
        let crate::graphics::ImageInner::EmbeddedImage { buffer, .. } = &state.frame.0 else {
            unreachable!()
        };
        let SharedImageBuffer::RGBA8Premultiplied(pixels) = buffer else {
            panic!("unexpected pixel format")
        };
        let p = pixels.as_slice()[0];
        [p.r, p.g, p.b]
    }

    const WHITE: [u8; 3] = [255, 255, 255];

    #[test]
    fn seek_skips_whole_plays() {
        let mut state = state(include_bytes!("../../graphics/image/testdata/forever.png"));
        // Each play lasts one second and ends with white from 600ms.
        state.seek(Duration::from_millis(1_000));
        state.seek(Duration::from_millis(10_000_650));
        assert_eq!(state.completed_plays, 10_000);
        assert_eq!(color(&state), WHITE);
        assert_eq!(state.frame_end, Duration::from_millis(10_001_000));
    }

    #[test]
    fn seek_stops_after_the_last_play() {
        let mut state = state(include_bytes!("../../graphics/image/testdata/twice.png"));
        state.seek(Duration::from_millis(1_000));
        state.seek(Duration::from_secs(60));
        assert!(state.finished());
        assert_eq!(state.completed_plays, 2);
        assert_eq!(color(&state), WHITE);
    }

    #[test]
    fn advancing_outside_a_tracker_invalidates_it() {
        let animation = ImageAnimation::new(animated(include_bytes!(
            "../../graphics/image/testdata/twice.png"
        )));
        let tracker = alloc::boxed::Box::pin(PropertyTracker::<false>::default());
        tracker.as_ref().evaluate(|| animation.frame());
        assert!(!tracker.is_dirty());
        crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
            driver.update_animations(driver.current_tick() + Duration::from_millis(150))
        });
        animation.frame();
        assert!(tracker.is_dirty());
    }
}
