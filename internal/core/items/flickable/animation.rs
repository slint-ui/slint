// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Combines the Android-style (hard-clamped) and iOS-style (rubber-band
//! overscroll) flick animation behind one type

use alloc::boxed::Box;
use core::pin::Pin;
use core::time::Duration;

use crate::Property;
use crate::animations::Instant;
use crate::animations::simulations::android::{AndroidFlick, AndroidFlickParameters};
use crate::animations::simulations::ios::{IOsFlick, IOsFlickParameters};
use crate::animations::simulations::scroll_spring::SpringSimulation;
use crate::animations::simulations::{Parameter, PositionSimulation, Simulation};
use crate::items::AutoBool;
use crate::lengths::{LogicalPoint, LogicalVector, RectLengths};
#[cfg(not(feature = "std"))]
use num_traits::Float;

/// `BouncingScrollPhysics.frictionFactor`'s base factor for
/// `ScrollDecelerationRate.normal`, used on iOS.
const IOS_FRICTION_FACTOR: f32 = 0.52;
/// `BouncingScrollPhysics.frictionFactor`'s base factor for
/// `ScrollDecelerationRate.fast`, used on macOS.
const MACOS_FRICTION_FACTOR: f32 = 0.26;
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
    Ios(IOsFlick),
}

impl Simulation for FlickAnimation {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        match self {
            FlickAnimation::Android(s) => s.step(current, new_tick),
            FlickAnimation::Ios(s) => s.step(current, new_tick),
        }
    }
}

impl PositionSimulation for FlickAnimation {
    fn remaining_distance(&self, time_elapsed: Duration) -> f32 {
        match self {
            FlickAnimation::Android(s) => s.remaining_distance(time_elapsed),
            FlickAnimation::Ios(s) => s.remaining_distance(time_elapsed),
        }
    }

    fn remaining_velocity(&self, time_elapsed: Duration) -> f32 {
        match self {
            FlickAnimation::Android(s) => s.remaining_velocity(time_elapsed),
            FlickAnimation::Ios(s) => s.remaining_velocity(time_elapsed),
        }
    }
}

/// `BouncingScrollPhysics.frictionFactor`: the further past the edge
/// `overscroll_fraction` (a fraction of the viewport size) already is, the
/// harder further overscroll gets.
fn friction_factor(overscroll_fraction: f32, base: f32) -> f32 {
    base * (1. - overscroll_fraction) * (1. - overscroll_fraction)
}

/// `BouncingScrollPhysics._applyFriction`: resists the portion of `abs_delta`
/// that lies within `extent_outside` of the edge by `gamma`, and passes the
/// rest through unresisted
fn apply_friction_scalar(extent_outside: f32, abs_delta: f32, gamma: f32) -> f32 {
    if extent_outside > 0. {
        let delta_to_limit = extent_outside / gamma;
        if abs_delta < delta_to_limit {
            return abs_delta * gamma;
        }
        extent_outside + (abs_delta - delta_to_limit)
    } else {
        abs_delta
    }
}

/// Rubber-bands a proposed drag `delta` along one axis, mirroring Flutter's
/// `BouncingScrollPhysics.applyPhysicsToUserOffset`
/// (`scroll_physics.dart`, Copyright 2014 The Flutter Authors, BSD-style license,
/// <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/widgets/scroll_physics.dart>):
/// once already overscrolled, further movement in the same direction gets
/// harder the further out `pos` already is, while movement back toward the
/// valid range ("easing") meets less resistance, or none at all on macOS.
///
/// `pos`/`delta`/`min_pos` are in the same units as `content_x`/`content_y`
/// (`0` is the leading edge, `min_pos` the trailing edge), not Flutter's
/// `pixels` (which increases into the content); the shape of the formula is
/// the same either way.
fn apply_friction_axis(pos: f32, delta: f32, min_pos: f32, viewport: f32) -> f32 {
    let overscroll_past_start = f32::max(pos, 0.);
    let overscroll_past_end = f32::max(min_pos - pos, 0.);
    let overscroll_past = f32::max(overscroll_past_start, overscroll_past_end);
    // no viewport fraction to compute friction from.
    if delta == 0. || overscroll_past <= 0. || viewport <= 0. {
        return delta;
    }

    let easing =
        (overscroll_past_start > 0. && delta < 0.) || (overscroll_past_end > 0. && delta > 0.);
    let fast = cfg!(target_os = "macos");
    if easing && fast {
        // macOS lets an easing drag back toward the valid range through at full speed.
        return delta;
    }

    let base = if fast { MACOS_FRICTION_FACTOR } else { IOS_FRICTION_FACTOR };
    let overscroll_fraction = if easing {
        (overscroll_past - delta.abs()) / viewport
    } else {
        overscroll_past / viewport
    };
    let gamma = friction_factor(overscroll_fraction, base);
    delta.signum() * apply_friction_scalar(overscroll_past, delta.abs(), gamma)
}

impl FlickAnimation {
    /// Applies overscroll drag resistance to a proposed `content_x`/`content_y`
    /// delta, per axis (see [`apply_friction_axis`]).
    pub fn apply_friction(
        current_pos: LogicalPoint,
        offset: LogicalVector,
        flick: Pin<&crate::items::Flickable>,
        flick_rc: &crate::item_tree::ItemRc,
    ) -> LogicalVector {
        let geo = crate::items::Flickable::geometry_without_virtual_keyboard(flick_rc);
        let width = geo.width_length().get() as f32;
        let height = geo.height_length().get() as f32;
        let min_x = width - flick.content_width().get() as f32;
        let min_y = height - flick.content_height().get() as f32;
        LogicalVector::new(
            apply_friction_axis(current_pos.x as f32, offset.x as f32, min_x, width) as _,
            apply_friction_axis(current_pos.y as f32, offset.y as f32, min_y, height) as _,
        )
    }

    pub fn minimum_flick_velocity_animation() -> f32 {
        #[cfg(target_os = "ios")]
        return 250.;
        #[cfg(not(target_os = "ios"))]
        return 50.;
    }

    pub fn carried_momentum(
        new_estimated_velocity: f32,
        current_velocity: f32,
        carry_momentum: AutoBool,
    ) -> f32 {
        let cm = match carry_momentum {
            AutoBool::Auto => {
                #[cfg(target_os = "ios")]
                {
                    true
                }
                // On Android this momentum carry does not exist
                #[cfg(not(target_os = "ios"))]
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

    /// Wether to bounce or not depending on the bounce variable
    /// and if `Auto` on the platform
    pub fn use_bounce(bounce: AutoBool) -> bool {
        match bounce {
            AutoBool::Auto => cfg!(target_os = "ios"),
            AutoBool::On => true,
            AutoBool::Off => false,
        }
    }

    /// Builds and starts the flick animation for one axis, choosing between
    /// the Android and iOS physics based on `bounce` and the platform.
    pub fn create_animation(
        animation_parameter: FlickAnimationParameter,
        bounce: AutoBool,
        start_value: f32,
        limit_value: Pin<Box<Property<f32>>>,
    ) -> FlickAnimation {
        if Self::use_bounce(bounce) {
            let params = match animation_parameter {
                FlickAnimationParameter::Velocity { velocity } => IOsFlickParameters::new(velocity),
                FlickAnimationParameter::Distance { delta, duration } => {
                    IOsFlickParameters::new_with_distance(delta, duration)
                }
            };
            FlickAnimation::Ios(params.simulation(start_value, limit_value))
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
    ) -> SpringSimulation {
        SpringSimulation::new_with_default_parameters(start_value, limit_value)
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

    /// The growth curve grows with the residual velocity a same-direction flick interrupts,
    /// and never depends on the new flick's own estimated velocity (unlike the old
    /// threshold-gated version, real repeated same-strength flicks keep compounding rather
    /// than alternating between a boosted and an un-boosted repeat).
    #[test]
    fn carried_momentum_grows_with_residual_velocity() {
        let mut previous = 0.;
        for current_velocity in [500., 1000., 2000., 3000., 3625.] {
            let carried = FlickAnimation::carried_momentum(1., current_velocity, AutoBool::On);
            assert!(carried > previous, "{current_velocity}: {carried} <= {previous}");
            previous = carried;
        }
    }

    /// Five real (residual velocity, required carry boost) pairs decomposed from repeated
    /// same-direction flicks measured on a live iOS UIScrollView (see
    /// /home/martin/Downloads/iosFlick, uniform-flick-events-t1.csv, releases 4-8), using the
    /// DRAG constant in `simulations::ios` to turn each release's total measured travel back
    /// into an effective launch velocity. `CARRY_SCALE`/`CARRY_EXPONENT` are fit to this data
    /// (and 107 further points from other real flick sequences) by log-log least squares.
    #[test]
    fn carried_momentum_matches_real_device_measurements() {
        for (current_velocity, measured_carry) in
            [(550.4, 628.1), (1028.3, 1570.4), (1644.5, 2826.6), (2642.8, 4397.7), (3625.8, 6282.2)]
        {
            let carried = FlickAnimation::carried_momentum(1., current_velocity, AutoBool::On);
            assert!(
                (carried - measured_carry).abs() < measured_carry * 0.35,
                "current_velocity={current_velocity}: predicted {carried}, measured {measured_carry}"
            );
        }
    }

    /// A Flickable with no laid-out size yet, or one the virtual keyboard fully
    /// covers, has zero (or negative) viewport extent on that axis. Dividing by
    /// it must not corrupt the position with `inf`/`NaN`; there's no viewport
    /// fraction to compute friction from, so the delta passes through unresisted.
    #[test]
    fn non_positive_viewport_does_not_produce_nan_or_inf() {
        for viewport in [0., -5.] {
            for delta in [-10., -1., 1., 10.] {
                let result = apply_friction_axis(10., delta, -100., viewport);
                assert!(result.is_finite(), "viewport {viewport}, delta {delta}: {result}");
                assert_eq!(result, delta);
            }
        }
    }
}
