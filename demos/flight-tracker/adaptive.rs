// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Keeps the frame rate near `TARGET_FPS` by changing the scene's render scale,
//! and the quality while the user hasn't picked one.
//!
//! Without GPU timers the frame rate is the only measure, and vsync caps it,
//! so it shows when frames take too long but not how much time is left.
//! The scale drops once frames are missed for `SLOW_WINDOWS` windows in a row,
//! and rises one step at a time after the frame rate held the target for `SETTLE`.
//! A step that was too slow isn't tried again for `RETRY`.
//! The quality drops after the scale reached `MIN_LEVEL`,
//! since a change in quality changes the look and stalls a frame.
//!
//! A display that refreshes less often than `TARGET_FPS` looks like frames that take too long.
//! So a drop that doesn't raise the frame rate by `GAIN` is undone,
//! and once neither the scale nor the quality raised it, that frame rate is the target.

use std::time::Duration;
use web_time::Instant;

use crate::Quality;

const TARGET_FPS: f32 = 60.0;
/// The part of the target below which a window is slow: about two missed frames.
const LOW: f32 = 57.0 / 60.0;
/// The part of the target from which the scale may rise: no missed frames.
const STEADY: f32 = 59.0 / 60.0;
/// By how much a drop has to raise the frame rate to count as a help.
const GAIN: f32 = 1.1;
const WINDOW: Duration = Duration::from_millis(500);
const SLOW_WINDOWS: u32 = 2;
const SETTLE: Duration = Duration::from_secs(2);
const RETRY: Duration = Duration::from_secs(20);
/// Frames that take longer are hitches, such as uploading the imagery,
/// and don't count unless most frames take as long.
const HITCH: Duration = Duration::from_millis(100);
/// How long after a start or a change in quality windows of hitches don't count,
/// while the GPU sets up the shaders and textures.
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
    /// The frame rate to keep: `TARGET_FPS`, or what the display turned out to reach.
    target: f32,
    /// The level from which the scale dropped, and the frame rate there.
    descent: Option<(u32, f32)>,
    /// Whether a lower scale raised the frame rate at this quality, as far as it's known.
    scale_helps: bool,
    /// The quality that this one was dropped from, and the frame rate there.
    dropped: Option<(Quality, f32)>,
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
        Self::starting(quality, auto_scale, TARGET_FPS, Instant::now())
    }

    fn starting(quality: Quality, auto_scale: bool, target: f32, now: Instant) -> Self {
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
            target,
            descent: None,
            scale_helps: true,
            dropped: None,
        }
    }

    pub fn scale(&self) -> f32 {
        self.level as f32 / MAX_LEVEL as f32
    }

    /// Counts a frame.
    /// Returns a quality to switch to, when `auto_quality` allows one.
    pub fn frame(
        &mut self,
        quality: Quality,
        auto_quality: bool,
        auto_scale: bool,
        animating: bool,
    ) -> Option<Quality> {
        self.frame_at(Instant::now(), quality, auto_quality, auto_scale, animating)
    }

    fn frame_at(
        &mut self,
        now: Instant,
        quality: Quality,
        auto_quality: bool,
        auto_scale: bool,
        animating: bool,
    ) -> Option<Quality> {
        let interval = now - self.last_frame;
        self.last_frame = now;
        if quality != self.quality || auto_scale != self.auto_scale {
            *self = Self::starting(quality, auto_scale, self.target, now);
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
        // A rate that the display reached once, it can reach again.
        self.target = self.target.max(fps.min(TARGET_FPS));
        if fps >= self.target * STEADY {
            self.slow_windows = 0;
            self.descent = None;
            self.dropped = None;
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
        if fps >= self.target * LOW {
            self.slow_windows = 0;
            return None;
        }
        self.slow_windows += 1;
        if self.slow_windows < SLOW_WINDOWS {
            return None;
        }
        self.slow_windows = 0;
        if auto_scale && self.scale_helps && self.level > MIN_LEVEL {
            self.descent.get_or_insert((self.level, fps));
            self.too_slow = Some((self.level, now));
            // The scene's cost grows with its pixels, the square of the scale.
            let fit = self.level as f32 * (fps / self.target).sqrt();
            self.level = (fit as u32).clamp(MIN_LEVEL, self.level - 1);
            return None;
        }
        if let Some((level, before)) = self.descent.take()
            && self.level * self.level * 2 <= level * level
            && fps < before * GAIN
        {
            // Half the pixels or fewer at the same frame rate: the scale isn't what limits it.
            self.scale_helps = false;
            self.level = level;
            self.too_slow = None;
        }
        let lower = match quality {
            Quality::High => Some(Quality::Medium),
            Quality::Medium => Some(Quality::Low),
            Quality::Low => None,
        };
        match (self.dropped.take(), lower.filter(|_| auto_quality)) {
            (Some((previous, before)), _) if auto_quality && fps < before * GAIN => {
                // The lower quality didn't raise the frame rate either.
                *self = Self::starting(previous, auto_scale, fps, now);
                Some(previous)
            }
            (_, Some(lower)) => {
                *self = Self::starting(lower, auto_scale, self.target, now);
                self.dropped = Some((quality, fps));
                Some(lower)
            }
            (_, None) => {
                if !self.scale_helps {
                    self.target = fps;
                }
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs the demo for `seconds`, at the frame rate that `fps` gives for a quality and a scale.
    /// Returns the quality, the scale, and the frame rate that it ends with.
    fn run(seconds: f32, fps: impl Fn(Quality, f32) -> f32) -> (Quality, f32, f32) {
        let mut now = Instant::now();
        let end = now + Duration::from_secs_f32(seconds);
        let mut quality = Quality::High;
        let mut adaptive = Adaptive::starting(quality, true, TARGET_FPS, now);
        while now < end {
            now += Duration::from_secs_f32(1.0 / fps(quality, adaptive.scale()));
            if let Some(changed) = adaptive.frame_at(now, quality, true, true, true) {
                quality = changed;
            }
        }
        (quality, adaptive.scale(), fps(quality, adaptive.scale()))
    }

    #[test]
    fn slow_display_keeps_quality_and_scale() {
        for refresh in [50.0, 30.0] {
            let (quality, scale, _) = run(60.0, |_, _| refresh);
            assert!(quality == Quality::High && scale == 1.0, "{refresh} Hz: scale {scale}");
        }
    }

    #[test]
    fn slow_gpu_lowers_scale() {
        // A GPU that renders 36 frames per second at the full scale.
        let (quality, scale, fps) = run(60.0, |_, scale| (36.0 / (scale * scale)).min(60.0));
        assert!(quality == Quality::High && scale < 1.0 && fps >= 57.0, "scale {scale}: {fps}");
    }

    #[test]
    fn costly_quality_drops_without_the_scale() {
        // The high quality misses every other frame, whatever the scale.
        let (quality, scale, _) =
            run(60.0, |quality, _| if quality == Quality::High { 30.0 } else { 60.0 });
        assert!(quality == Quality::Medium && scale == 1.0, "scale {scale}");
    }

    #[test]
    fn slow_gpu_ends_at_the_lowest() {
        // 10 frames per second at the full scale, and a third more for each lower quality.
        let (quality, scale, _) = run(120.0, |quality, scale| {
            let gain = match quality {
                Quality::High => 1.0,
                Quality::Medium => 1.33,
                Quality::Low => 1.77,
            };
            (10.0 * gain / (scale * scale)).min(60.0)
        });
        assert!(quality == Quality::Low && scale < 1.0, "scale {scale}");
    }
}
