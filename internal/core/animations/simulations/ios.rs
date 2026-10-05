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
//!
//! The spring phase reuses `spring::SpringRegime` rather than re-deriving the
//! mass/spring/damper ODE Flutter's own `SpringSimulation` solves.

use core::time::Duration;

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
/// UIKit ends a deceleration where the content is once its speed falls below this, in logical
/// px/s, measured on an iPhone 13 Pro Max with iOS 27.
const DECELERATION_STOP_VELOCITY: f32 = 10.0;
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
        // finalX - x0 = (v0 - v_stop) / -drag_log  =>  v0 = -distance * drag_log + v_stop
        let stop_velocity =
            if distance == 0. { 0. } else { distance.signum() * DECELERATION_STOP_VELOCITY };
        Self { initial_velocity: -distance * drag_log + stop_velocity }
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
    /// Limit property to determine when to bounce back
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    limit: f32,
    direction: Direction,
    data: IOsFlickParameters,
    start_value: f32,
    start_time: Instant,
    friction_time: f32,
    spring_time: f32,
    /// The spring that pulls the position back to `limit_value` once
    /// `spring_time` has elapsed.
    spring: SpringRegime,
    last_position: f32,
    last_time: f32,
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

        let (spring_time, spring) =
            Self::spring_parameters(start_value, data.initial_velocity, limit, &direction);

        Self {
            limit_value,
            limit,
            direction,
            data,
            start_value,
            start_time,
            friction_time: 0.,
            spring_time,
            spring,
            last_position: start_value,
            last_time: 0.,
        }
    }

    fn spring_parameters(
        start_value: f32,
        velocity: f32,
        limit: f32,
        direction: &Direction,
    ) -> (f32, SpringRegime) {
        let outside_bounds = match direction {
            Direction::Increasing => start_value > limit,
            Direction::Decreasing => start_value < limit,
        };
        if outside_bounds {
            return (
                0.,
                SpringRegime::new(
                    start_value - limit,
                    velocity,
                    spring_natural_frequency(),
                    SPRING_DAMPING_RATIO,
                ),
            );
        }

        let final_x = Self::friction_end(start_value, velocity);
        let needs_spring = match direction {
            Direction::Increasing => final_x > limit,
            Direction::Decreasing => final_x < limit,
        };
        let spring_time = if needs_spring {
            Self::time_at_x(start_value, velocity, f32::ln(DRAG), limit).unwrap_or(f32::INFINITY)
        } else {
            f32::INFINITY
        };

        let v_at_spring =
            if spring_time.is_finite() { velocity * f32::powf(DRAG, spring_time) } else { 0. };
        // See BouncingScrollSimulation's `maxSpringTransferVelocity` clamp: only ever
        // caps an excessively fast *positive* handoff velocity, same as upstream.
        let spring_velocity = f32::min(v_at_spring, MAX_SPRING_TRANSFER_VELOCITY);
        let spring = SpringRegime::new(
            0.,
            spring_velocity,
            spring_natural_frequency(),
            SPRING_DAMPING_RATIO,
        );

        (spring_time, spring)
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

    /// The elapsed time at which the friction curve's speed falls to [`DECELERATION_STOP_VELOCITY`].
    fn friction_stop_time(v0: f32) -> f32 {
        if f32::abs(v0) <= DECELERATION_STOP_VELOCITY {
            0.
        } else {
            f32::ln(DECELERATION_STOP_VELOCITY / f32::abs(v0)) / f32::ln(DRAG)
        }
    }

    /// The position the friction curve (starting at `x0` with velocity `v0`) comes to rest at.
    fn friction_end(x0: f32, v0: f32) -> f32 {
        let drag_log = f32::ln(DRAG);
        x0 + v0 * f32::powf(DRAG, Self::friction_stop_time(v0)) / drag_log - v0 / drag_log
    }

    /// The position the simulation comes to rest at: the limit if the spring takes over,
    /// the end of the friction curve otherwise.
    fn final_position(&self) -> f32 {
        if self.spring_time.is_finite() {
            self.limit
        } else {
            Self::friction_end(self.start_value, self.data.initial_velocity)
        }
    }

    /// Position and velocity of the friction curve at elapsed time `t`.
    fn friction_at(&self, t: f32) -> (f32, f32) {
        let drag_log = f32::ln(DRAG);
        let v0 = self.data.initial_velocity;
        if t >= Self::friction_stop_time(v0) {
            return (Self::friction_end(self.start_value, v0), 0.);
        }
        let position = self.start_value + v0 * f32::powf(DRAG, t) / drag_log - v0 / drag_log;
        let velocity = v0 * f32::powf(DRAG, t);
        (position, velocity)
    }

    /// Position and velocity of the overscroll spring at elapsed time `t`,
    /// measured from the moment the spring took over.
    fn spring_at(&self, t: f32) -> (f32, f32) {
        let (x_rel, velocity) = self.spring.evaluate(t);
        (self.limit + x_rel, velocity)
    }

    /// Position, velocity, and whether the simulation has settled, at
    /// elapsed time `t` since `start_time`.
    fn evaluate(&self, t: f32) -> (f32, f32, bool) {
        if t < self.spring_time {
            let (position, velocity) = self.friction_at(t - self.friction_time);
            (position, velocity, f32::abs(velocity) < VELOCITY_TOLERANCE)
        } else {
            let (position, velocity) = self.spring_at(t - self.spring_time);
            let done = f32::abs(position - self.limit) < DISTANCE_TOLERANCE
                && f32::abs(velocity) < VELOCITY_TOLERANCE;
            (position, velocity, done)
        }
    }

    fn step_internal(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        let t = new_tick.duration_since(self.start_time).as_secs_f32();
        let limit = self.limit_value.as_ref().get();
        if limit != self.limit || *current != self.last_position {
            let (_, velocity, _) = self.evaluate(self.last_time);
            let (spring_time, spring) =
                Self::spring_parameters(*current, velocity, limit, &self.direction);
            self.limit = limit;
            self.start_value = *current;
            self.data.initial_velocity = velocity;
            self.friction_time = self.last_time;
            self.spring_time = self.last_time + spring_time;
            self.spring = spring;
        }
        let (mut position, _velocity, done) = self.evaluate(t);

        if done && t >= self.spring_time {
            position = self.limit;
        }

        *current = position;
        self.last_position = position;
        self.last_time = t;
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
        self.final_position() - position
    }

    fn remaining_velocity(&self, time_elapsed: Duration) -> f32 {
        let (_position, velocity, _done) = self.evaluate(time_elapsed.as_secs_f32());
        velocity
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
    fn retargeting_preserves_position_and_velocity() {
        for sign in [-1., 1.] {
            for (limit, initial_velocity, new_limit, offset) in
                [(2000., 500., 10., 0.), (20., 5000., 10., 0.), (20., 5000., 20., 50.)]
            {
                let start = Instant::default();
                let mut simulation = IOsFlick::new_internal(
                    0.,
                    test_limit_property(sign * limit),
                    IOsFlickParameters::new(sign * initial_velocity),
                    start,
                );
                let elapsed = Duration::from_millis(50);
                let mut position = 0.;
                assert!(!simulation.step(&mut position, start + elapsed));
                let velocity = simulation.remaining_velocity(elapsed);

                simulation.limit_value.as_ref().set(sign * new_limit);
                position += sign * offset;
                let before = position;
                assert!(!simulation.step(&mut position, start + elapsed));
                assert_approx_eq!(position, before);
                assert!((simulation.remaining_velocity(elapsed) - velocity).abs() < 0.01);
                assert_approx_eq!(
                    simulation.remaining_distance(elapsed),
                    sign * new_limit - position
                );

                assert!(simulation.step(&mut position, start + Duration::from_secs(10)));
                assert_approx_eq!(position, sign * new_limit);
            }
        }
    }

    #[test]
    fn growing_bounds_cancels_the_pending_bounce() {
        for sign in [-1., 1.] {
            let start = Instant::default();
            let mut simulation = IOsFlick::new_internal(
                0.,
                test_limit_property(sign * 100.),
                IOsFlickParameters::new(sign * 500.),
                start,
            );
            let mut unbounded = IOsFlick::new_internal(
                0.,
                test_limit_property(sign * 1000.),
                IOsFlickParameters::new(sign * 500.),
                start,
            );
            let mut position = 0.;
            let mut expected = 0.;
            assert!(!simulation.step(&mut position, start + Duration::from_millis(50)));
            simulation.limit_value.as_ref().set(sign * 1000.);
            for millis in [50, 100, 300, 1000, 10_000] {
                let tick = start + Duration::from_millis(millis);
                let finished = simulation.step(&mut position, tick);
                assert_eq!(finished, unbounded.step(&mut expected, tick));
                assert_approx_eq!(position, expected);
            }
            assert!(sign * position > 100.);
        }
    }

    /// The distance left shrinks to zero, and together with the distance moved so far it is
    /// always the whole distance of the friction curve.
    #[test]
    fn remaining_distance_is_the_distance_left_to_rest() {
        let time = Instant::default();
        let simulation = IOsFlick::new_internal(
            10.,
            test_limit_property(2000.),
            IOsFlickParameters::new(500.),
            time,
        );
        let total = -(500. - DECELERATION_STOP_VELOCITY) / f32::ln(DRAG);
        assert!((simulation.remaining_distance(Duration::ZERO) - total).abs() < 1e-2);

        let mut previous = total;
        for millis in [50, 300, 1000, 1900] {
            let elapsed = Duration::from_millis(millis);
            let remaining = simulation.remaining_distance(elapsed);
            let (position, _, _) = simulation.evaluate(elapsed.as_secs_f32());
            assert!(remaining > 0. && remaining < previous, "{millis}ms: {remaining}");
            assert!((position - 10. + remaining - total).abs() < 1e-2, "{millis}ms");
            previous = remaining;
        }
        assert!(simulation.remaining_distance(Duration::from_secs(60)).abs() < 1e-2);
    }

    /// With the spring, the position comes to rest at the limit, so the distance left is measured
    /// to it. Beyond the limit that is the way back.
    #[test]
    fn remaining_distance_with_the_spring_is_measured_to_the_limit() {
        let time = Instant::default();
        let increasing = IOsFlick::new_internal(
            10.,
            test_limit_property(20.),
            IOsFlickParameters::new(5000.),
            time,
        );
        assert!((increasing.remaining_distance(Duration::ZERO) - 10.).abs() < 1e-2);
        assert!(increasing.remaining_distance(Duration::from_millis(50)) < 0.);
        assert!(increasing.remaining_distance(Duration::from_secs(10)).abs() < 1e-2);

        let decreasing = IOsFlick::new_internal(
            20.,
            test_limit_property(10.),
            IOsFlickParameters::new(-5000.),
            time,
        );
        assert!((decreasing.remaining_distance(Duration::ZERO) + 10.).abs() < 1e-2);
        assert!(decreasing.remaining_distance(Duration::from_millis(50)) > 0.);
        assert!(decreasing.remaining_distance(Duration::from_secs(10)).abs() < 1e-2);
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
            "There is no velocity. So the simulation must be finish"
        );
        assert_eq!(current, START_VALUE);
    }

    /// UIKit stops a 656 pt/s fling about 2.1 s after release, 5 points before its friction
    /// curve would come to rest.
    #[test]
    fn stops_where_the_speed_falls_to_the_stop_velocity() {
        let time = Instant::default();
        let mut simulation = IOsFlick::new_internal(
            0.,
            test_limit_property(5000.),
            IOsFlickParameters::new(656.),
            time,
        );
        let stop_time = IOsFlick::friction_stop_time(656.);
        assert!((stop_time - 2.09).abs() < 0.01, "{stop_time}");
        let (_, velocity, done) = simulation.evaluate(stop_time - 0.01);
        assert!(!done && (velocity - DECELERATION_STOP_VELOCITY).abs() < 0.5, "{velocity}");

        let mut current = 0.;
        assert!(simulation.step(&mut current, time + Duration::from_secs_f32(stop_time + 0.01)));
        let unstopped_travel = -656. / f32::ln(DRAG);
        assert!((unstopped_travel - current - 5.).abs() < 0.05, "{current}");
        let stopped = current;
        simulation.step(&mut current, time + Duration::from_secs(5));
        assert_eq!(current, stopped);
    }

    #[test]
    fn distance_parameters_still_cover_the_distance() {
        for distance in [-120., 40., 300.] {
            let params = IOsFlickParameters::new_with_distance(distance, Duration::ZERO);
            let end = IOsFlick::friction_end(10., params.initial_velocity);
            assert!((end - 10. - distance).abs() < 1e-3, "{distance}: {end}");
        }
    }
}
