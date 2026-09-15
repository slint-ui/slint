// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Ported from Flutter's `BouncingScrollSimulation` (and the `FrictionSimulation`/
//! `SpringSimulation` it composes), in scroll_simulation.dart and physics/*.dart, which are:
//! Copyright 2014 The Flutter Authors. All rights reserved.
//!
//! Use of the original source is governed by a BSD-style license
//!
//! Original: <https://github.com/flutter/flutter/blob/d6bed8ff6135cdd414f14edc3063f761d47ca846/packages/flutter/lib/src/widgets/scroll_simulation.dart>
//!
//! An implementation of scroll physics that matches iOS: the position decelerates
//! under friction, and if that would carry it past `limit_value`, a spring takes
//! over and pulls it back to the boundary (the "rubber band" overscroll effect).
//! Unlike `android::AndroidFlick`, this simulation allows the position to
//! overshoot `limit_value` before settling, hence `overshoot_allowed() == true`.
//!
//! The spring phase reuses `spring::SpringRegime` rather than re-deriving the
//! mass/spring/damper ODE Flutter's own `SpringSimulation` solves.

use core::time::Duration;
use std::println;

use crate::animations::Instant;
use crate::animations::simulations::spring::SpringRegime;
use crate::animations::simulations::{Direction, Parameter, PositionSimulation, Simulation};
#[cfg(not(feature = "std"))]
use num_traits::Float;

#[cfg(test)]
use crate::animations::simulations::test_limit_property;

/// iOS's `UIScrollView.decelerationRate` (`.normal`), expressed the way a
/// friction simulation wants it: `0.998^1000 ≈ 0.135`, the fraction of
/// velocity retained after one second.
const DRAG: f32 = 0.135;

/// `BouncingScrollPhysics`'s default spring: `mass: 0.5, stiffness: 100.0`,
/// damping ratio `1.1` (slightly overdamped: pulls back to the boundary
/// without any bounce/oscillation).
const SPRING_MASS: f32 = 0.5;
const SPRING_STIFFNESS: f32 = 100.0;
const SPRING_DAMPING_RATIO: f32 = 1.1;

/// The largest velocity that can be carried over from the friction phase
/// into the spring phase.
const MAX_SPRING_TRANSFER_VELOCITY: f32 = 5000.0;

/// A velocity at or below this magnitude (logical px/s) is considered stopped.
const VELOCITY_TOLERANCE: f32 = 1.0;
/// A spring within this distance (logical px) of `limit_value` is considered settled.
const DISTANCE_TOLERANCE: f32 = 0.5;

fn spring_natural_frequency() -> f32 {
    f32::sqrt(SPRING_STIFFNESS / SPRING_MASS)
}

/// Input parameters for the `IOsFlick` simulation.
#[derive(Debug, Clone)]
pub struct IOsFlickParameters {
    pub initial_velocity: f32,
}

impl IOsFlickParameters {
    pub fn new(initial_velocity: f32) -> Self {
        Self { initial_velocity }
    }
    pub fn new_with_distance(distance: f32, _duration: Duration) -> Self {
        let drag_log = f32::ln(DRAG);
        // finalX - x0 = -v0 / drag_log  =>  v0 = -distance * drag_log
        Self { initial_velocity: -distance * drag_log }
    }
}

impl Parameter for IOsFlickParameters {
    type Output = IOsFlick;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        IOsFlick::new(start_value, limit_value, self)
    }
}

#[derive(Debug)]
pub struct IOsFlick {
    /// If the limit is not reached, it is also fine. Exceeding the limit is
    /// expected too (that's the overscroll this simulation is for); the
    /// spring phase pulls the position back to it.
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    data: IOsFlickParameters,
    direction: Direction,
    start_value: f32,
    start_time: Instant,
    /// Elapsed time (since `start_time`) at which the friction curve would
    /// cross `limit_value`; `f32::INFINITY` if it never does.
    spring_time: f32,
    /// The spring that pulls the position back to `limit_value` once
    /// `spring_time` has elapsed. Always built, mirroring the original
    /// (Dart always constructs `_springSimulation` too), but only evaluated
    /// once `spring_time` is finite and elapsed.
    spring: SpringRegime,
    /// The absolute position this simulation's own formula last produced.
    /// `current` may be changed by other code between calls (e.g. bounds
    /// clamping), so steps apply a delta relative to this rather than
    /// assigning absolute positions; see `android::AndroidFlick` for why.
    traveled: f32,
}

impl IOsFlick {
    pub fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: IOsFlickParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        mut data: IOsFlickParameters,
        start_time: Instant,
    ) -> Self {
        let limit = limit_value.as_ref().get();
        let direction = if start_value == limit {
            if data.initial_velocity >= 0. { Direction::Increasing } else { Direction::Decreasing }
        } else if start_value < limit {
            debug_assert!(data.initial_velocity >= 0.); // Makes no sense yet that the velocity goes into the other direction
            data.initial_velocity = f32::abs(data.initial_velocity);
            Direction::Increasing
        } else {
            data.initial_velocity = -f32::abs(data.initial_velocity);
            debug_assert!(data.initial_velocity <= 0.);
            Direction::Decreasing
        };

        let drag_log = f32::ln(DRAG);
        let v0 = data.initial_velocity;
        // See FrictionSimulation.finalX: the position as time approaches infinity.
        let final_x = start_value - v0 / drag_log;

        let needs_spring = match direction {
            Direction::Increasing => final_x > limit,
            Direction::Decreasing => final_x < limit,
        };
        let spring_time = if needs_spring {
            Self::time_at_x(start_value, v0, drag_log, limit).unwrap_or(f32::INFINITY)
        } else {
            f32::INFINITY
        };

        let v_at_spring =
            if spring_time.is_finite() { v0 * f32::powf(DRAG, spring_time) } else { 0. };
        // See BouncingScrollSimulation's `maxSpringTransferVelocity` clamp: only ever
        // caps an excessively fast *positive* handoff velocity, same as upstream.
        let spring_velocity = f32::min(v_at_spring, MAX_SPRING_TRANSFER_VELOCITY);
        let spring = SpringRegime::new(
            0.,
            spring_velocity,
            spring_natural_frequency(),
            SPRING_DAMPING_RATIO,
        );

        Self {
            limit_value,
            data,
            direction,
            start_value,
            start_time,
            spring_time,
            spring,
            traveled: start_value,
        }
    }

    /// The elapsed time at which the friction curve (starting at `x0` with
    /// velocity `v0`) reaches `target`, or `None` if it never does.
    fn time_at_x(x0: f32, v0: f32, drag_log: f32, target: f32) -> Option<f32> {
        if v0 == 0. {
            return None;
        }
        // x(t) - x0 = v0 * (drag^t - 1) / drag_log  =>  solve for t.
        let ratio = 1. + (target - x0) * drag_log / v0;
        if ratio <= 0. {
            return None;
        }
        Some(f32::ln(ratio) / drag_log)
    }

    /// Position and velocity of the friction curve at elapsed time `t`.
    fn friction_at(&self, t: f32) -> (f32, f32) {
        let drag_log = f32::ln(DRAG);
        let v0 = self.data.initial_velocity;
        let position = self.start_value + v0 * f32::powf(DRAG, t) / drag_log - v0 / drag_log;
        let velocity = v0 * f32::powf(DRAG, t);
        (position, velocity)
    }

    /// Position and velocity of the overscroll spring at elapsed time `t`,
    /// measured from the moment the spring took over.
    fn spring_at(&self, t: f32) -> (f32, f32) {
        let (x_rel, velocity) = self.spring.evaluate(t);
        (self.limit_value.as_ref().get() + x_rel, velocity)
    }

    /// Position, velocity, and whether the simulation has settled, at
    /// elapsed time `t` since `start_time`.
    fn evaluate(&self, t: f32) -> (f32, f32, bool) {
        let res = if t < self.spring_time {
            let (position, velocity) = self.friction_at(t);
            (position, velocity, f32::abs(velocity) < VELOCITY_TOLERANCE)
        } else {
            let (position, velocity) = self.spring_at(t - self.spring_time);
            let limit = self.limit_value.as_ref().get();
            let done = f32::abs(position - limit) < DISTANCE_TOLERANCE
                && f32::abs(velocity) < VELOCITY_TOLERANCE;
            (position, velocity, done)
        };
        res
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let (mut position, _velocity, done) = self.evaluate(t);

        if done && t >= self.spring_time {
            // Land exactly on the boundary, matching `ScrollSpringSimulation`.
            position = self.limit_value.as_ref().get();
        }

        *current += position - self.traveled;
        self.traveled = position;
        done
    }
}

impl Simulation for IOsFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for IOsFlick {
    fn remaining_distance(&self, time_elapsed: Duration) -> f32 {
        let (position, _velocity, _done) = self.evaluate(time_elapsed.as_secs_f32());
        position - self.start_value
    }

    fn remaining_velocity(&self, time_elapsed: Duration) -> f32 {
        let (_position, velocity, _done) = self.evaluate(time_elapsed.as_secs_f32());
        velocity
    }

    fn overshoot_allowed(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::assert_approx_eq;

    #[test]
    fn starts_at_initial_position_and_velocity() {
        let time = Instant::default();
        let mut simulation = IOsFlick::new_internal(
            10.,
            test_limit_property(2000.),
            IOsFlickParameters::new(500.),
            time,
        );
        let mut current = 10.;
        simulation.step(&mut current, time);
        assert_approx_eq!(current, 10.);
    }

    /// A gentle fling that settles under friction alone, well short of the limit.
    #[test]
    fn gentle_fling_never_reaches_limit() {
        let time = Instant::default();
        let mut simulation = IOsFlick::new_internal(
            10.,
            test_limit_property(2000.),
            IOsFlickParameters::new(500.),
            time,
        );
        let mut current = 10.;
        let finished = simulation.step(&mut current, time + Duration::from_secs(30));
        assert_eq!(finished, true);
        assert!(current < 2000.);
        assert!(current > 10.);
        assert!(simulation.spring_time.is_infinite());
    }

    /// A hard fling toward a nearby limit overshoots, then springs back exactly to it.
    #[test]
    fn hard_fling_overshoots_then_springs_back() {
        let time = Instant::default();
        let mut simulation = IOsFlick::new_internal(
            10.,
            test_limit_property(20.),
            IOsFlickParameters::new(5000.),
            time,
        );
        assert!(simulation.spring_time.is_finite());

        let mut current = 10.;
        // Shortly after the fling starts, it should already be past the limit
        // (that's the whole point of the rubber-band effect).
        let finished = simulation.step(&mut current, time + Duration::from_millis(50));
        assert_eq!(finished, false);
        assert!(current > 20.);

        // Long after, the spring must have pulled it back exactly to the limit.
        let finished = simulation.step(&mut current, time + Duration::from_secs(10));
        assert_eq!(finished, true);
        assert_approx_eq!(current, 20.);
    }

    #[test]
    fn decreasing_direction_mirrors_increasing() {
        let time = Instant::default();
        let mut simulation = IOsFlick::new_internal(
            20.,
            test_limit_property(10.),
            IOsFlickParameters::new(-5000.),
            time,
        );
        assert!(simulation.spring_time.is_finite());

        let mut current = 20.;
        let finished = simulation.step(&mut current, time + Duration::from_millis(50));
        assert_eq!(finished, false);
        assert!(current < 10.);

        let finished = simulation.step(&mut current, time + Duration::from_secs(10));
        assert_eq!(finished, true);
        assert_approx_eq!(current, 10.);
    }

    #[test]
    fn overshoot_is_allowed() {
        let time = Instant::default();
        let simulation = IOsFlick::new_internal(
            10.,
            test_limit_property(20.),
            IOsFlickParameters::new(500.),
            time,
        );
        assert!(simulation.overshoot_allowed());
    }

    #[test]
    fn zero_init_velocity() {
        const START_VALUE: f32 = 10.;

        let parameters = IOsFlickParameters::new(0.);
        let time = Instant::default();
        let mut simulation =
            IOsFlick::new_internal(START_VALUE, test_limit_property(100.), parameters, time);

        let mut current = START_VALUE;
        assert_eq!(
            simulation.step(&mut current, time),
            true,
            "There is no velocity. So the simulation is must be finish"
        );
        assert_eq!(current, START_VALUE);
    }
}
