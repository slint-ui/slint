// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::Property;
use crate::animations::Instant;
use crate::graphics::{AnimatedImage, FrameDecoder, Image, OpaqueImageVTable};
use crate::timers::{Timer, TimerMode};
use alloc::rc::Rc;
use const_field_offset::FieldOffsets;
use core::cell::{Cell, RefCell};
use core::pin::Pin;
use core::time::Duration;

/// Plays an [`AnimatedImage`] for one item.
///
/// The animation clock is read without a dependency, so that unrelated ticks don't redraw the
/// item.
#[derive(FieldOffsets)]
#[pin]
pub(super) struct ImageAnimation {
    redraw: Property<()>,
    state: RefCell<State>,
    timer: Timer,
    /// The `resumed_at` and `frame_end` that the timer was started for.
    scheduled: Cell<Option<(Instant, Duration)>>,
}

/// Positions are durations of playback since the start of the first play.
struct State {
    image: vtable::VRc<OpaqueImageVTable, AnimatedImage>,
    /// `None` once the animation finished.
    decoder: Option<FrameDecoder<'static>>,
    frame: Image,
    frame_end: Duration,
    play_duration: Option<Duration>,
    completed_plays: u32,
    paused_position: Duration,
    resumed_at: Option<Instant>,
}

impl State {
    fn new(image: vtable::VRc<OpaqueImageVTable, AnimatedImage>) -> Self {
        let mut state = Self {
            frame: image.first_frame(),
            image,
            decoder: None,
            frame_end: Duration::ZERO,
            play_duration: None,
            completed_plays: 0,
            paused_position: Duration::ZERO,
            resumed_at: None,
        };
        state.start_play(Duration::ZERO);
        state
    }

    fn finished(&self) -> bool {
        self.decoder.is_none()
    }

    fn position(&self, now: Instant) -> Duration {
        self.paused_position + self.resumed_at.map_or(Duration::ZERO, |resumed_at| now - resumed_at)
    }

    fn start_play(&mut self, play_start: Duration) {
        self.decoder = self.image.frames();
        match self.decoder.as_mut().and_then(FrameDecoder::next_frame) {
            Some((buffer, delay)) => {
                self.frame = Image::from_rgba8_premultiplied(buffer);
                self.frame_end = play_start + delay;
            }
            None => self.decoder = None,
        }
    }

    /// Moves to the next frame, skipping whole plays that end before `position`.
    fn advance(&mut self, position: Duration) {
        if let Some((buffer, delay)) = self.decoder.as_mut().and_then(FrameDecoder::next_frame) {
            self.frame = Image::from_rgba8_premultiplied(buffer);
            self.frame_end += delay;
            return;
        }
        // The first play starts at zero.
        let play_duration = *self.play_duration.get_or_insert(self.frame_end);
        self.completed_plays += 1;
        let remaining = self.image.plays().map(|plays| plays.get() - self.completed_plays);
        if remaining == Some(0) {
            self.decoder = None;
            return;
        }
        // Decode the last play, which ends on its last frame.
        let skippable = (position - self.frame_end).as_nanos() / play_duration.as_nanos();
        let skip =
            remaining.map_or(skippable, |remaining| skippable.min(u128::from(remaining) - 1));
        let skip = u32::try_from(skip).unwrap_or(u32::MAX);
        self.completed_plays = self.completed_plays.saturating_add(skip);
        self.start_play(self.frame_end + play_duration * skip);
    }

    /// Decodes up to the frame shown at `position`.
    fn seek(&mut self, position: Duration) {
        while !self.finished() && position >= self.frame_end {
            self.advance(position);
        }
    }

    fn set_running(&mut self, running: bool, now: Instant) {
        match (running, self.resumed_at) {
            (true, None) => self.resumed_at = Some(now),
            (false, Some(_)) => {
                self.paused_position = self.position(now);
                self.resumed_at = None;
            }
            _ => {}
        }
    }
}

impl ImageAnimation {
    pub(super) fn new(image: vtable::VRc<OpaqueImageVTable, AnimatedImage>) -> Pin<Rc<Self>> {
        Rc::pin(Self {
            redraw: Property::new(()),
            state: RefCell::new(State::new(image)),
            timer: Timer::default(),
            scheduled: Cell::new(None),
        })
    }

    pub(super) fn shows(&self, image: &vtable::VRc<OpaqueImageVTable, AnimatedImage>) -> bool {
        vtable::VRc::ptr_eq(&self.state.borrow().image, image)
    }

    /// Returns the frame to show now, and schedules a redraw for when it changes.
    pub(super) fn frame(self: &Pin<Rc<Self>>, running: bool) -> Image {
        let now = crate::properties::evaluate_no_tracking(crate::animations::current_tick);
        let mut state = self.state.borrow_mut();
        let position = state.position(now);
        state.seek(position);
        state.set_running(running, now);
        let schedule = state
            .resumed_at
            .filter(|_| !state.finished())
            .map(|resumed_at| (resumed_at, state.frame_end));
        let frame_end = state.frame_end;
        let frame = state.frame.clone();
        drop(state);

        match schedule {
            Some(schedule) if self.scheduled.get() != Some(schedule) || !self.timer.running() => {
                self.scheduled.set(Some(schedule));
                let weak = pin_weak::rc::PinWeak::downgrade(self.clone());
                self.timer.start(TimerMode::SingleShot, frame_end - position, move || {
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
        Self::FIELD_OFFSETS.redraw().apply_pin(self.as_ref()).get();
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::SharedImageBuffer;

    fn state(data: &[u8]) -> State {
        let image = Image::load_from_data(data, Some("png")).unwrap();
        let crate::graphics::ImageInner::AnimatedImage(animated) = image.0 else {
            panic!("expected an animation")
        };
        State::new(animated)
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
    fn pausing_keeps_the_position() {
        let mut state = state(include_bytes!("../../graphics/image/testdata/forever.png"));
        let start = Instant::from_millis(5_000);
        state.set_running(true, start);
        state.set_running(false, Instant::from_millis(5_250));
        assert_eq!(state.position(Instant::from_millis(9_000)), Duration::from_millis(250));
        state.set_running(true, Instant::from_millis(9_000));
        assert_eq!(state.position(Instant::from_millis(9_100)), Duration::from_millis(350));
    }
}
