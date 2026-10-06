// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Combines the Android-style (hard-clamped) and the bouncing (rubber-band
//! overscroll) flick animation behind one type

use alloc::boxed::Box;
use core::pin::Pin;
use core::time::Duration;

use crate::Property;
use crate::animations::Instant;
use crate::animations::simulations::android::{AndroidFlick, AndroidFlickParameters};
use crate::animations::simulations::bounce::{BounceFlick, BounceFlickParameters};
use crate::animations::simulations::rubber_band;
use crate::animations::simulations::{Parameter, PositionSimulation, Simulation};
use crate::items::AutoBool;
use crate::lengths::{LogicalPoint, LogicalRect, LogicalVector, RectLengths};
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// `FlickAnimation::carried_momentum`'s growth curve: `carried = CARRY_SCALE *
/// current_velocity.abs().powf(CARRY_EXPONENT)`. Fit by log-log least squares (R² = 0.76)
/// against 112 real same-direction repeat flicks (a rapid flick starting while the previous
/// one was still gliding), captured from a live iOS UIScrollView via XCTest.
const CARRY_SCALE: f32 = 0.3522;
const CARRY_EXPONENT: f32 = 1.1674;

pub enum FlickAnimationParameter {
    /// Cover a fixed distance in a fixed duration, e.g. wheel scrolling.
    Distance { delta: f32, duration: Duration },
    /// Start from an estimated release velocity, e.g. a touch flick.
    Velocity { velocity: f32 },
}

/// Common flick animation type to dynamically switching between simulations
pub enum FlickAnimation {
    Android(AndroidFlick),
    Bounce(BounceFlick),
}

impl Simulation for FlickAnimation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        match self {
            FlickAnimation::Android(s) => s.step(current, new_tick),
            FlickAnimation::Bounce(s) => s.step(current, new_tick),
        }
    }
}

impl PositionSimulation for FlickAnimation {
    fn remaining_distance(&self, now: Instant) -> f32 {
        match self {
            FlickAnimation::Android(s) => s.remaining_distance(now),
            FlickAnimation::Bounce(s) => s.remaining_distance(now),
        }
    }

    fn remaining_velocity(&self, now: Instant) -> f32 {
        match self {
            FlickAnimation::Android(s) => s.remaining_velocity(now),
            FlickAnimation::Bounce(s) => s.remaining_velocity(now),
        }
    }
}

/// Moves `pos` by the drag `delta` along one axis, rubber-banding the part outside
/// `min_pos..=0` (see [`rubber_band`]).
pub(super) fn rubber_band_move_axis(
    pos: f32,
    delta: f32,
    min_pos: f32,
    viewport_length: f32,
) -> f32 {
    if viewport_length <= 0. {
        return pos + delta;
    }
    let raw = if pos > 0. {
        rubber_band::uncompress(pos, viewport_length)
    } else if pos < min_pos {
        min_pos + rubber_band::uncompress(pos - min_pos, viewport_length)
    } else {
        pos
    } + delta;
    if raw > 0. {
        rubber_band::compress(raw, viewport_length)
    } else if raw < min_pos {
        min_pos + rubber_band::compress(raw - min_pos, viewport_length)
    } else {
        raw
    }
}

impl FlickAnimation {
    /// Moves `current_pos` by the drag `delta`.
    /// Axes that bounce are rubber-banded (see [`rubber_band_move_axis`]); the others are clamped.
    pub fn rubber_band_move(
        current_pos: LogicalPoint,
        delta: LogicalVector,
        flick: Pin<&crate::items::Flickable>,
        geo: &LogicalRect,
        use_bounce_x: bool,
        use_bounce_y: bool,
    ) -> LogicalPoint {
        let move_axis = |pos: f32, delta: f32, content: f32, viewport_length: f32, bounce: bool| {
            let min_pos = (viewport_length - content).min(0.);
            if bounce {
                rubber_band_move_axis(pos, delta, min_pos, viewport_length)
            } else {
                (pos + delta).clamp(min_pos, 0.)
            }
        };
        LogicalPoint::new(
            move_axis(
                current_pos.x as f32,
                delta.x as f32,
                flick.content_width().get() as f32,
                geo.width_length().get() as f32,
                use_bounce_x,
            ) as _,
            move_axis(
                current_pos.y as f32,
                delta.y as f32,
                flick.content_height().get() as f32,
                geo.height_length().get() as f32,
                use_bounce_y,
            ) as _,
        )
    }

    pub fn minimum_flick_velocity_animation() -> f32 {
        #[cfg(any(target_os = "ios", slint_ios_scroll_physics))]
        return 250.;
        #[cfg(not(any(target_os = "ios", slint_ios_scroll_physics)))]
        return 50.;
    }

    pub fn carried_momentum(
        new_estimated_velocity: f32,
        current_velocity: f32,
        carry_momentum: AutoBool,
    ) -> f32 {
        let cm = match carry_momentum {
            AutoBool::Auto => {
                #[cfg(any(target_os = "ios", slint_ios_scroll_physics))]
                {
                    true
                }
                // On Android this momentum carry does not exist
                #[cfg(not(any(target_os = "ios", slint_ios_scroll_physics)))]
                {
                    false
                }
            }
            AutoBool::On => true,
            AutoBool::Off => false,
        };
        let same_direction = new_estimated_velocity.signum() == current_velocity.signum();
        if current_velocity == 0. || !cm || !same_direction {
            return 0.;
        }

        current_velocity.signum()
            * f32::min(CARRY_SCALE * f32::powf(current_velocity.abs(), CARRY_EXPONENT), 40000.0)
    }

    /// Whether to bounce or not depending on the bounce variable
    /// and if `Auto` on the platform
    pub fn use_bounce(bounce: AutoBool) -> bool {
        match bounce {
            AutoBool::Auto => cfg!(any(target_os = "ios", slint_ios_scroll_physics)),
            AutoBool::On => true,
            AutoBool::Off => false,
        }
    }

    /// Builds and starts the flick animation for one axis, choosing between
    /// the Android and the bouncing physics based on `bounce` and the platform.
    pub fn create_animation(
        animation_parameter: FlickAnimationParameter,
        bounce: AutoBool,
        start_value: f32,
        limit_value: Pin<Box<Property<f32>>>,
    ) -> FlickAnimation {
        if Self::use_bounce(bounce) {
            let params = match animation_parameter {
                FlickAnimationParameter::Velocity { velocity } => {
                    BounceFlickParameters::new(velocity)
                }
                FlickAnimationParameter::Distance { delta, duration } => {
                    BounceFlickParameters::new_with_distance(delta, duration)
                }
            };
            FlickAnimation::Bounce(params.simulation(start_value, limit_value))
        } else {
            let params = match animation_parameter {
                FlickAnimationParameter::Velocity { velocity } => {
                    AndroidFlickParameters::new_with_default_friction(velocity)
                }
                FlickAnimationParameter::Distance { delta, duration } => {
                    return FlickAnimation::Android(AndroidFlick::new_with_distance(
                        start_value,
                        limit_value,
                        delta,
                        duration,
                    ));
                }
            };
            FlickAnimation::Android(params.simulation(start_value, limit_value))
        }
    }

    pub fn create_spring_animation(
        start_value: f32,
        limit_value: Pin<Box<Property<f32>>>,
        start_time: Instant,
        velocity: f32,
        drag_speed: f32,
    ) -> BounceFlick {
        BounceFlick::new_overscroll_release(
            start_value,
            limit_value,
            start_time,
            velocity,
            drag_speed,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carried_momentum_is_zero_when_off_stopped_or_reversed() {
        assert_eq!(FlickAnimation::carried_momentum(2000., 2000., AutoBool::Off), 0.);
        assert_eq!(FlickAnimation::carried_momentum(2000., 0., AutoBool::On), 0.);
        assert_eq!(FlickAnimation::carried_momentum(-2000., 2000., AutoBool::On), 0.);
    }

    /// A Flickable with no laid-out size yet, or one the virtual keyboard fully
    /// covers, has zero (or negative) viewport extent on that axis. Dividing by
    /// it must not corrupt the position with `inf`/`NaN`; there's no viewport
    /// to rubber-band against, so the delta passes through unresisted.
    #[test]
    fn non_positive_viewport_does_not_produce_nan_or_inf() {
        for viewport_length in [0., -5.] {
            for delta in [-10., -1., 1., 10.] {
                let result = rubber_band_move_axis(10., delta, -100., viewport_length);
                assert!(
                    result.is_finite(),
                    "viewport_length {viewport_length}, delta {delta}: {result}"
                );
                assert_eq!(result, 10. + delta);
            }
        }
    }

    #[test]
    fn rubber_band_paths_do_not_depend_on_move_batching() {
        for start in [0., -100.] {
            for distance in [-600., -190., 190., 600.] {
                let single = rubber_band_move_axis(start, distance, -100., 774.);
                for count in [2, 10, 100] {
                    let mut stepped = start;
                    for _ in 0..count {
                        stepped =
                            rubber_band_move_axis(stepped, distance / count as f32, -100., 774.);
                    }
                    assert!((single - stepped).abs() < 0.003, "{single} != {stepped}");
                    let returned = rubber_band_move_axis(stepped, -distance, -100., 774.);
                    assert!((returned - start).abs() < 0.003, "{start} != {returned}");
                }
            }
        }
    }
}
