// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Keeps the frame rate near `TARGET_FPS` by changing the scene's render scale, and the
//! quality while the player hasn't picked one.
//!
//! Without GPU timers the frame rate is the only measure, and vsync caps it, so it shows
//! when frames take too long but not how much time is left. The scale drops once frames are
//! missed for `SLOW_WINDOWS` windows in a row, and rises one step at a time after the frame
//! rate held the target for `SETTLE`. A step that was too slow isn't tried again for `RETRY`.
//! The quality only drops, after the scale reached `MIN_LEVEL` or while the player picked
//! one, since a change in quality changes the look and stalls a frame.

use std::time::Duration;
use web_time::Instant;

use crate::Quality;

const TARGET_FPS: f32 = 60.0;
/// The frame rate below which a window is slow: about two missed frames.
const LOW_FPS: f32 = 57.0;
/// The frame rate from which the scale may rise: no missed frames.
const STEADY_FPS: f32 = 59.0;
const WINDOW: Duration = Duration::from_millis(500);
const SLOW_WINDOWS: u32 = 2;
const SETTLE: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(20);
/// Frames that take longer are hitches, such as building a new track, and don't count
/// unless most frames take as long.
const HITCH: Duration = Duration::from_millis(100);
/// How long after a start or a change in quality windows of hitches don't count, while the
/// GPU sets up the shaders and textures.
const WARM_UP: Duration = Duration::from_secs(5);
/// The scale is `level / MAX_LEVEL`.
const MAX_LEVEL: u32 = 20;
const MIN_LEVEL: u32 = 10;

pub struct Adaptive {
    level: u32,
    quality: Quality,
    auto_scale: bool,
    started: Instant,
    /// The lowest level that was too slow, and when.
    too_slow: Option<(u32, Instant)>,
    slow_windows: u32,
    steady_since: Instant,
    last_frame: Instant,
    window: Window,
}

#[derive(Default)]
struct Window {
    frames: u32,
    time: Duration,
    hitches: u32,
    hitch_time: Duration,
    /// Whether every frame came from the animation, rather than from input while idle.
    animating: bool,
}

impl Window {
    fn mostly_hitches(&self) -> bool {
        self.hitches * 2 >= self.frames
    }

    fn fps(&self) -> f32 {
        let (frames, time) = if self.mostly_hitches() {
            (self.frames, self.time)
        } else {
            (self.frames - self.hitches, self.time - self.hitch_time)
        };
        frames as f32 / time.as_secs_f32()
    }
}

impl Adaptive {
    pub fn new(quality: Quality, auto_scale: bool) -> Self {
        let now = Instant::now();
        Self {
            level: MAX_LEVEL,
            quality,
            auto_scale,
            started: now,
            too_slow: None,
            slow_windows: 0,
            steady_since: now,
            last_frame: now,
            window: Window { animating: true, ..Default::default() },
        }
    }

    pub fn scale(&self) -> f32 {
        self.level as f32 / MAX_LEVEL as f32
    }

    /// Counts a frame. Returns a lower quality to switch to, when `auto_quality` allows one.
    pub fn frame(
        &mut self,
        quality: Quality,
        auto_quality: bool,
        auto_scale: bool,
        animating: bool,
    ) -> Option<Quality> {
        let now = Instant::now();
        let interval = now - self.last_frame;
        self.last_frame = now;
        if quality != self.quality || auto_scale != self.auto_scale {
            *self = Self::new(quality, auto_scale);
            return None;
        }
        let window = &mut self.window;
        window.frames += 1;
        window.time += interval;
        if interval > HITCH {
            window.hitches += 1;
            window.hitch_time += interval;
        }
        window.animating &= animating;
        if window.time < WINDOW {
            return None;
        }
        let window =
            std::mem::replace(&mut self.window, Window { animating, ..Default::default() });
        let warming_up = window.mostly_hitches() && now - self.started < WARM_UP;
        if !window.animating || warming_up {
            self.slow_windows = 0;
            self.steady_since = now;
            return None;
        }
        let fps = window.fps();
        if fps >= STEADY_FPS {
            self.slow_windows = 0;
            if auto_scale && now - self.steady_since >= SETTLE && self.level < MAX_LEVEL {
                self.steady_since = now;
                let retry = self
                    .too_slow
                    .is_none_or(|(level, when)| self.level + 1 < level || now - when >= RETRY);
                if retry {
                    self.level += 1;
                }
            }
            return None;
        }
        self.steady_since = now;
        if fps >= LOW_FPS {
            self.slow_windows = 0;
            return None;
        }
        self.slow_windows += 1;
        if self.slow_windows < SLOW_WINDOWS {
            return None;
        }
        self.slow_windows = 0;
        if auto_scale && self.level > MIN_LEVEL {
            self.too_slow = Some((self.level, now));
            // The scene's cost grows with its pixels, the square of the scale.
            let fit = self.level as f32 * (fps / TARGET_FPS).sqrt();
            self.level = (fit as u32).clamp(MIN_LEVEL, self.level - 1);
            return None;
        }
        let lower = match quality {
            Quality::High => Quality::Medium,
            Quality::Medium => Quality::Low,
            Quality::Low => return None,
        };
        if !auto_quality {
            return None;
        }
        *self = Self::new(lower, auto_scale);
        Some(lower)
    }
}
