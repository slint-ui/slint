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
//! The scroll physics of a Flickable that bounces: the position decelerates under friction, and
//! once it's past `limit_value`, a spring pulls it back to the boundary.
//! A release past the boundary keeps its outward motion briefly, then the spring takes over.
//! The constants are fitted to UIKit's `UIScrollView`.

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

/// The largest speed that can be carried over from the friction phase
/// into the spring phase.
const MAX_SPRING_TRANSFER_VELOCITY: f32 = 5000.0;

/// A velocity at or below this magnitude (logical px/s) is considered stopped.
const VELOCITY_TOLERANCE: f32 = 1.0;
/// UIKit ends a deceleration where the content is once its speed falls below this, in logical
/// px/s, measured on an iPhone 13 Pro Max with iOS 27.
const DECELERATION_STOP_VELOCITY: f32 = 10.0;
/// A spring within this distance (logical px) of `limit_value` is considered settled.
const DISTANCE_TOLERANCE: f32 = 0.5;

// The spring constants are fitted to UIKit returns after a held pull past the top edge and after
// flings into it, captured on an iPhone 13 Pro Max with iOS 27.
/// The natural frequency of the critically damped spring past the limit.
const SPRING_FREQUENCY: f32 = 10.67037;
/// How long after a release past the limit the return starts.
const RETURN_DELAY: Duration = Duration::from_millis(7);
/// The initial return speed relative to the overscroll, for a small overscroll.
const RETURN_RATE_MIN: f32 = 5.470122;
/// How much the initial return rate rises for a large overscroll.
const RETURN_RATE_RISE: f32 = 4.825567;
/// The overscroll at which the initial return rate has risen halfway.
const RETURN_RATE_HALF_DISTANCE: f32 = 165.8656;

// Fitted to UIKit releases while still dragging outward, at 130–2030 pointer points per second
// and 45–230 points past the edge.
/// Up to this pointer speed, a release keeps the full initial return rate.
const RETURN_RATE_FADE_START: f32 = 400.;
/// The pointer speed at which the initial return rate has faded out, for a large overscroll.
const RETURN_RATE_FADE_END_MAX: f32 = 3400.;
/// The overscroll at which the fade's end has risen halfway to [`RETURN_RATE_FADE_END_MAX`].
const RETURN_RATE_FADE_HALF_DISTANCE: f32 = 70.;

/// The share of the initial return rate that a release keeps, `distance` past the edge.
fn initial_return_rate_share(drag_speed: f32, distance: f32) -> f32 {
    let squared = distance * distance;
    let fade_length = (RETURN_RATE_FADE_END_MAX - RETURN_RATE_FADE_START) * squared
        / (RETURN_RATE_FADE_HALF_DISTANCE * RETURN_RATE_FADE_HALF_DISTANCE + squared);
    (1. - (drag_speed.abs() - RETURN_RATE_FADE_START) / fade_length.max(f32::EPSILON)).clamp(0., 1.)
}

fn initial_return_rate(distance: f32) -> f32 {
    let squared = distance * distance;
    RETURN_RATE_MIN
        + RETURN_RATE_RISE * squared
            / (RETURN_RATE_HALF_DISTANCE * RETURN_RATE_HALF_DISTANCE + squared)
}

/// Input parameters for the `BounceFlick` simulation.
#[derive(Debug, Clone)]
pub struct BounceFlickParameters {
    pub initial_velocity: f32,
}

impl BounceFlickParameters {
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

impl Parameter for BounceFlickParameters {
    type Output = BounceFlick;
    fn simulation(
        self,
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    ) -> Self::Output {
        BounceFlick::new(start_value, limit_value, self)
    }
}

/// How the position moves until the spring takes over.
#[derive(Debug, Clone, Copy)]
enum Approach {
    /// Decelerates under [`DRAG`], from `start_value` with `velocity` at `since` seconds.
    Friction { start_value: f32, velocity: f32, since: f32 },
    /// Keeps `velocity` from `start_value`, during the delay after a release past the limit.
    Coast { start_value: f32, velocity: f32 },
}

#[derive(Debug)]
pub struct BounceFlick {
    /// Limit property to determine when to bounce back
    limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
    limit: f32,
    direction: Direction,
    start_time: Instant,
    approach: Approach,
    /// Seconds after `start_time` at which the spring takes over, or infinity if it never does.
    spring_time: f32,
    /// The spring that pulls the position, relative to `limit`, back to it.
    spring: SpringRegime,
    last_position: f32,
    last_time: f32,
}

impl BounceFlick {
    pub fn new(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: BounceFlickParameters,
    ) -> Self {
        Self::new_internal(start_value, limit_value, data, crate::animations::current_tick())
    }

    fn new_internal(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        data: BounceFlickParameters,
        start_time: Instant,
    ) -> Self {
        let limit = limit_value.as_ref().get();
        let mut velocity = data.initial_velocity;
        let direction = if start_value == limit {
            if velocity >= 0. { Direction::Increasing } else { Direction::Decreasing }
        } else if start_value < limit {
            debug_assert!(velocity >= 0.); // Makes no sense yet that the velocity goes into the other direction
            velocity = f32::abs(velocity);
            Direction::Increasing
        } else {
            velocity = -f32::abs(velocity);
            debug_assert!(velocity <= 0.);
            Direction::Decreasing
        };

        let (spring_time, spring) =
            Self::spring_parameters(start_value, velocity, limit, &direction);

        Self {
            limit_value,
            limit,
            direction,
            start_time,
            approach: Approach::Friction { start_value, velocity, since: 0. },
            spring_time,
            spring,
            last_position: start_value,
            last_time: 0.,
        }
    }

    /// Springs from `start_value`, past `limit_value`, back to it, like UIKit after a pull is
    /// released past the edge.
    /// `velocity` is the content's velocity at the release; only motion away from
    /// `limit_value` carries over.
    /// A faster `drag_speed`, the pointer's speed at the release, weakens that motion's
    /// initial pull back.
    /// The content keeps that velocity until [`RETURN_DELAY`] after `start_time`, then the
    /// return starts.
    pub fn new_overscroll_release(
        start_value: f32,
        limit_value: core::pin::Pin<alloc::boxed::Box<crate::Property<f32>>>,
        start_time: Instant,
        velocity: f32,
        drag_speed: f32,
    ) -> Self {
        let limit = limit_value.as_ref().get();
        let overscroll = start_value - limit;
        let velocity = if velocity * overscroll > 0. { velocity } else { 0. };
        let delay = RETURN_DELAY.as_secs_f32();
        let onset_overscroll = overscroll + velocity * delay;
        let return_rate_share =
            if velocity == 0. { 1. } else { initial_return_rate_share(drag_speed, overscroll) };
        let onset_velocity =
            velocity - return_rate_share * onset_overscroll * initial_return_rate(onset_overscroll);
        Self {
            limit_value,
            limit,
            direction: if overscroll >= 0. { Direction::Increasing } else { Direction::Decreasing },
            start_time,
            approach: Approach::Coast { start_value, velocity },
            spring_time: delay,
            spring: SpringRegime::new(onset_overscroll, onset_velocity, SPRING_FREQUENCY, 1.),
            last_position: start_value,
            last_time: 0.,
        }
    }

    /// When the friction curve from `start_value` with `velocity` reaches `limit`, and the spring
    /// that takes over there. Past the limit already, the spring takes over at once.
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
            return (0., SpringRegime::new(start_value - limit, velocity, SPRING_FREQUENCY, 1.));
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
        let spring_velocity =
            v_at_spring.clamp(-MAX_SPRING_TRANSFER_VELOCITY, MAX_SPRING_TRANSFER_VELOCITY);
        (spring_time, SpringRegime::new(0., spring_velocity, SPRING_FREQUENCY, 1.))
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

    /// Position and velocity of the friction curve from `x0` with velocity `v0`, `t` seconds in.
    fn friction_at(x0: f32, v0: f32, t: f32) -> (f32, f32) {
        if t >= Self::friction_stop_time(v0) {
            return (Self::friction_end(x0, v0), 0.);
        }
        let drag_log = f32::ln(DRAG);
        let position = x0 + v0 * f32::powf(DRAG, t) / drag_log - v0 / drag_log;
        (position, v0 * f32::powf(DRAG, t))
    }

    /// The position the simulation comes to rest at: the limit if the spring takes over,
    /// the end of the friction curve otherwise.
    fn final_position(&self) -> f32 {
        match self.approach {
            Approach::Friction { start_value, velocity, .. } if self.spring_time.is_infinite() => {
                Self::friction_end(start_value, velocity)
            }
            _ => self.limit,
        }
    }

    /// Position, velocity, and whether the simulation has settled, at
    /// elapsed time `t` since `start_time`.
    fn evaluate(&self, t: f32) -> (f32, f32, bool) {
        if t < self.spring_time {
            match self.approach {
                Approach::Friction { start_value, velocity, since } => {
                    let (position, velocity) = Self::friction_at(start_value, velocity, t - since);
                    (position, velocity, f32::abs(velocity) < VELOCITY_TOLERANCE)
                }
                Approach::Coast { start_value, velocity } => {
                    (start_value + velocity * t, velocity, false)
                }
            }
        } else {
            let (x_rel, velocity) = self.spring.evaluate(t - self.spring_time);
            let position = self.limit + x_rel;
            let done =
                f32::abs(x_rel) < DISTANCE_TOLERANCE && f32::abs(velocity) < VELOCITY_TOLERANCE;
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
            self.approach =
                Approach::Friction { start_value: *current, velocity, since: self.last_time };
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

impl Simulation for BounceFlick {
    fn step(&mut self, current: &mut f32, new_tick: Instant) -> bool {
        self.step_internal(current, new_tick)
    }
}

impl PositionSimulation for BounceFlick {
    fn remaining_distance(&self, now: Instant) -> f32 {
        let (position, _velocity, _done) =
            self.evaluate(now.duration_since(self.start_time).as_secs_f32());
        self.final_position() - position
    }

    fn remaining_velocity(&self, now: Instant) -> f32 {
        let (_position, velocity, _done) =
            self.evaluate(now.duration_since(self.start_time).as_secs_f32());
        velocity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::simulations::assert_approx_eq;

    /// A hard fling toward a nearby limit overshoots, then springs back exactly to it.
    #[test]
    fn hard_fling_overshoots_then_springs_back() {
        for sign in [-1., 1.] {
            let time = Instant::default();
            let mut simulation = BounceFlick::new_internal(
                sign * 10.,
                test_limit_property(sign * 20.),
                BounceFlickParameters::new(sign * 5000.),
                time,
            );
            let mut current = sign * 10.;
            assert!(!simulation.step(&mut current, time + Duration::from_millis(50)));
            assert!(sign * current > 20., "{current}");
            assert!(simulation.step(&mut current, time + Duration::from_secs(10)));
            assert_approx_eq!(current, sign * 20.);
        }
    }

    #[test]
    fn retargeting_preserves_position_and_velocity() {
        for sign in [-1., 1.] {
            for (limit, initial_velocity, new_limit, offset) in
                [(2000., 500., 10., 0.), (20., 5000., 10., 0.), (20., 5000., 20., 50.)]
            {
                let start = Instant::default();
                let mut simulation = BounceFlick::new_internal(
                    0.,
                    test_limit_property(sign * limit),
                    BounceFlickParameters::new(sign * initial_velocity),
                    start,
                );
                let elapsed = Duration::from_millis(50);
                let mut position = 0.;
                assert!(!simulation.step(&mut position, start + elapsed));
                let velocity = simulation.remaining_velocity(start + elapsed);

                simulation.limit_value.as_ref().set(sign * new_limit);
                position += sign * offset;
                let before = position;
                assert!(!simulation.step(&mut position, start + elapsed));
                assert_approx_eq!(position, before);
                assert!((simulation.remaining_velocity(start + elapsed) - velocity).abs() < 0.01);
                assert_approx_eq!(
                    simulation.remaining_distance(start + elapsed),
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
            let mut simulation = BounceFlick::new_internal(
                0.,
                test_limit_property(sign * 100.),
                BounceFlickParameters::new(sign * 500.),
                start,
            );
            let mut unbounded = BounceFlick::new_internal(
                0.,
                test_limit_property(sign * 1000.),
                BounceFlickParameters::new(sign * 500.),
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

    /// With the spring, the position comes to rest at the limit, so the distance left is measured
    /// to it. Beyond the limit that is the way back.
    #[test]
    fn remaining_distance_with_the_spring_is_measured_to_the_limit() {
        let time = Instant::default();
        let increasing = BounceFlick::new_internal(
            10.,
            test_limit_property(20.),
            BounceFlickParameters::new(5000.),
            time,
        );
        assert!((increasing.remaining_distance(time) - 10.).abs() < 1e-2);
        assert!(increasing.remaining_distance(time + Duration::from_millis(50)) < 0.);
        assert!(increasing.remaining_distance(time + Duration::from_secs(10)).abs() < 1e-2);

        let decreasing = BounceFlick::new_internal(
            20.,
            test_limit_property(10.),
            BounceFlickParameters::new(-5000.),
            time,
        );
        assert!((decreasing.remaining_distance(time) + 10.).abs() < 1e-2);
        assert!(decreasing.remaining_distance(time + Duration::from_millis(50)) > 0.);
        assert!(decreasing.remaining_distance(time + Duration::from_secs(10)).abs() < 1e-2);
    }

    #[test]
    fn zero_init_velocity() {
        const START_VALUE: f32 = 10.;

        let parameters = BounceFlickParameters::new(0.);
        let time = Instant::default();
        let mut simulation =
            BounceFlick::new_internal(START_VALUE, test_limit_property(100.), parameters, time);

        let mut current = START_VALUE;
        assert_eq!(
            simulation.step(&mut current, time),
            true,
            "There is no velocity. So the simulation must be finish"
        );
        assert_eq!(current, START_VALUE);
    }

    #[test]
    fn stops_where_the_speed_falls_to_the_stop_velocity() {
        let time = Instant::default();
        let velocity = 656.;
        let mut simulation = BounceFlick::new_internal(
            0.,
            test_limit_property(5000.),
            BounceFlickParameters::new(velocity),
            time,
        );
        let stop_time = BounceFlick::friction_stop_time(velocity);
        let (_, before_stop, done) = simulation.evaluate(stop_time - 0.001);
        assert!(!done && (before_stop - DECELERATION_STOP_VELOCITY).abs() < 0.1, "{before_stop}");

        let mut current = 0.;
        assert!(simulation.step(&mut current, time + Duration::from_secs_f32(stop_time + 0.01)));
        let unstopped_travel = -velocity / f32::ln(DRAG);
        let shortfall = -DECELERATION_STOP_VELOCITY / f32::ln(DRAG);
        assert!((unstopped_travel - current - shortfall).abs() < 0.05, "{current}");
        let stopped = current;
        simulation.step(&mut current, time + Duration::from_secs(5));
        assert_approx_eq!(current, stopped);
    }

    fn start_time() -> Instant {
        Instant::from_millis(1_000)
    }

    #[test]
    fn retargeting_a_release_past_the_limit_preserves_position_and_velocity() {
        for sign in [-1., 1.] {
            for (new_limit, offset) in [(10., 0.), (20., 10.)] {
                let start = start_time();
                let mut simulation = BounceFlick::new_overscroll_release(
                    sign * 30.,
                    test_limit_property(sign * 20.),
                    start,
                    0.,
                    0.,
                );
                let elapsed = Duration::from_millis(50);
                let mut position = sign * 30.;
                assert!(!simulation.step(&mut position, start + elapsed));
                let velocity = simulation.remaining_velocity(start + elapsed);

                simulation.limit_value.as_ref().set(sign * new_limit);
                position += sign * offset;
                let before = position;
                assert!(!simulation.step(&mut position, start + elapsed));
                assert_approx_eq!(position, before);
                assert_approx_eq!(simulation.remaining_velocity(start + elapsed), velocity);
                assert_approx_eq!(
                    simulation.remaining_distance(start + elapsed),
                    sign * new_limit - position
                );

                assert!(simulation.step(&mut position, start + Duration::from_secs(10)));
                assert_approx_eq!(position, sign * new_limit);
            }
        }
    }

    #[test]
    fn remaining_velocity_matches_the_displayed_motion() {
        for (start, velocity) in [(-200., 0.), (-100., -600.), (20., 0.), (20., 600.), (200., 600.)]
        {
            let simulation = BounceFlick::new_overscroll_release(
                start,
                test_limit_property(0.),
                start_time(),
                velocity,
                0.,
            );
            assert_approx_eq!(simulation.remaining_distance(start_time()), -start);
            for millis in [50, 100, 200, 500] {
                let t = start_time() + Duration::from_millis(millis);
                let dt = Duration::from_millis(1);
                let measured = -(simulation.remaining_distance(t + dt)
                    - simulation.remaining_distance(t - dt))
                    / (2. * dt.as_secs_f32());
                assert!((simulation.remaining_velocity(t) - measured).abs() < 0.2, "{start}");
            }
            assert!(
                simulation.remaining_distance(start_time() + Duration::from_secs(10)).abs() < 0.001
            );
        }
    }

    #[test]
    fn ignores_release_velocity_toward_the_limit() {
        let still =
            BounceFlick::new_overscroll_release(45., test_limit_property(0.), start_time(), 0., 0.);
        let inward = BounceFlick::new_overscroll_release(
            45.,
            test_limit_property(0.),
            start_time(),
            -600.,
            0.,
        );
        for millis in [0, 5, 50, 200] {
            let t = start_time() + Duration::from_millis(millis);
            assert_approx_eq!(inward.remaining_distance(t), still.remaining_distance(t));
        }
    }

    #[test]
    fn faster_drags_pull_back_less() {
        let peak = |drag_speed: f32| {
            let start = start_time();
            let mut simulation = BounceFlick::new_overscroll_release(
                45.,
                test_limit_property(0.),
                start,
                600.,
                drag_speed,
            );
            let mut position = 45.;
            let mut peak = position;
            for millis in (1..200).step_by(1) {
                simulation.step(&mut position, start + Duration::from_millis(millis));
                peak = peak.max(position);
            }
            peak
        };
        assert!(peak(1200.) > peak(400.) + 1., "{} <= {}", peak(1200.), peak(400.));
        assert_approx_eq!(peak(0.), peak(RETURN_RATE_FADE_START));
    }

    #[test]
    fn initial_return_rate_fades_with_the_drag_speed() {
        for distance in [20., 100., 300.] {
            assert_eq!(initial_return_rate_share(0., distance), 1.);
            assert_eq!(initial_return_rate_share(RETURN_RATE_FADE_START, distance), 1.);
            assert!(initial_return_rate_share(RETURN_RATE_FADE_START + 100., distance) < 1.);
            assert_eq!(initial_return_rate_share(RETURN_RATE_FADE_END_MAX + 1., distance), 0.);
        }
        // A larger overscroll fades out at a higher drag speed.
        assert!(initial_return_rate_share(1000., 300.) > initial_return_rate_share(1000., 100.));
    }
}
