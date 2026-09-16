// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Combines the Android-style (hard-clamped) and iOS-style (rubber-band
//! overscroll) flick animation behind one type, so callers such as
//! `Flickable` don't need to know which concrete animation is running.

use alloc::boxed::Box;
use core::pin::Pin;
use core::time::Duration;
use std::println;

use crate::Property;
use crate::animations::Instant;
use crate::animations::simulations::android::{AndroidFlick, AndroidFlickParameters};
use crate::animations::simulations::ios::{IOsFlick, IOsFlickParameters};
use crate::animations::simulations::scroll_spring::SpringSimulation;
use crate::animations::simulations::{Parameter, PositionSimulation, Simulation};
use crate::items::AutoBool;
use crate::lengths::{LogicalPoint, LogicalVector, RectLengths};

/// `BouncingScrollPhysics.frictionFactor`'s base factor for
/// `ScrollDecelerationRate.normal`, used on iOS.
const IOS_FRICTION_FACTOR: f32 = 0.52;
/// `BouncingScrollPhysics.frictionFactor`'s base factor for
/// `ScrollDecelerationRate.fast`, used on macOS.
const MACOS_FRICTION_FACTOR: f32 = 0.26;
const MOMENTUM_RETAIN_VELOCITY_THRESHOLD_FACTOR: f32 = 0.5;

/// Parameters to start a flick animation from, independent of which
/// concrete animation ends up running.
pub enum FlickAnimationParameter {
    /// Cover a fixed distance in a fixed duration, e.g. wheel scrolling.
    Distance { delta: f32, duration: Duration },
    /// Start from an estimated release velocity, e.g. a touch flick.
    Velocity { velocity: f32 },
}

/// Either an Android-style hard-clamped fling or an iOS-style rubber-band
/// fling. Wrapping both in one enum (rather than picking a single concrete
/// type at compile time) lets `create_animation` choose per call, based on
/// the `bounce` property and platform, since that decision can change at
/// runtime even on a single platform (`bounce: on` forces iOS-style physics
/// anywhere).
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
/// harder further overscroll gets. `base` is `0.52` on iOS and `0.26` on
/// macOS ("fast" deceleration).
fn friction_factor(overscroll_fraction: f32, base: f32) -> f32 {
    base * (1. - overscroll_fraction) * (1. - overscroll_fraction)
}

/// `BouncingScrollPhysics._applyFriction`: resists the portion of `abs_delta`
/// that lies within `extent_outside` of the edge by `gamma`, and passes the
/// rest through unresisted, since past `extent_outside` there's no more
/// "outside" left to resist.
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
    if delta == 0. || overscroll_past <= 0. {
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
    ///
    /// This runs regardless of platform: an axis only has anything to resist
    /// once it's out of range, and `ensure_in_bound` already keeps a
    /// bounce-off axis (Android's default) hard-clamped in range before this
    /// ever runs, so there's nothing left here to gate on platform.
    pub fn apply_friction(
        current_pos: LogicalPoint,
        offset: LogicalVector,
        flick: Pin<&crate::items::Flickable>,
        flick_rc: &crate::item_tree::ItemRc,
    ) -> LogicalVector {
        let geo = crate::items::Flickable::geometry_without_virtual_keyboard(flick_rc);
        let width = geo.width_length().get();
        let height = geo.height_length().get();
        let min_x = width - flick.content_width().get();
        let min_y = height - flick.content_height().get();
        LogicalVector::new(
            apply_friction_axis(current_pos.x as f32, offset.x as f32, min_x, width) as _,
            apply_friction_axis(current_pos.y as f32, offset.y as f32, min_y, height) as _,
        )
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
                #[cfg(not(target_os = "ios"))]
                {
                    false
                }
            }
            AutoBool::On => true,
            AutoBool::Off => false,
        };
        if current_velocity == 0. || !cm {
            return 0.;
        }

        // On Android this momentum carry on does not exist
        let carried_velocity = current_velocity.signum()
            * f32::min(0.000816 * f32::powf(current_velocity.abs(), 1.967), 40000.0);

        let is_velocity_not_substantially_less_than_carried_momentum = new_estimated_velocity.abs()
            > carried_velocity.abs() * MOMENTUM_RETAIN_VELOCITY_THRESHOLD_FACTOR;
        let same_direction = new_estimated_velocity.signum() == current_velocity.signum();

        if is_velocity_not_substantially_less_than_carried_momentum && same_direction {
            return carried_velocity;
        } else {
            if !same_direction {
                println!("Carried momentum. same direction: FALSE");
            }
            if !is_velocity_not_substantially_less_than_carried_momentum {
                println!(
                    "Carried momentum. Velocities different: {:?} vs. {:?}",
                    new_estimated_velocity.abs(),
                    carried_velocity.abs()
                );
            }
        }
        0.
    }

    /// Whether to use the iOS-style (rubber-band overscroll) animation rather
    /// than the Android-style (hard-clamped) one: forced on by `bounce: on`,
    /// forced off by `bounce: off`, and otherwise on exactly where iOS's own
    /// scroll views bounce.
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
                    println!(
                        "New android simulation. Start value: {start_value:?}, Limit: {limit_value:?}, Velocity: {velocity:?}"
                    );
                    AndroidFlickParameters::new_with_default_friction(velocity)
                }
                FlickAnimationParameter::Distance { delta, duration } => {
                    AndroidFlickParameters::new_with_distance(delta, duration)
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
